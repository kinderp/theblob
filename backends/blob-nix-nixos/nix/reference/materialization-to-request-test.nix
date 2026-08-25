{ pkgs, lib, ... }:
let
  busName = "org.theblob.NixOsRoot";
  objectPath = "/org/theblob/NixOsRoot";
  interfaceName = "org.theblob.NixOsRoot1";
  previewAction = "org.theblob.nixos.preview-activation";

  rootHarnesses = pkgs.stdenv.mkDerivation {
    pname = "blob-materialization-to-request-vm";
    version = "0.1.0";
    src = lib.cleanSource ../../../..;
    nativeBuildInputs = [ pkgs.cargo pkgs.rustc ];
    buildPhase = ''
      runHook preBuild
      export HOME="$TMPDIR/home"
      export CARGO_HOME="$TMPDIR/cargo-home"
      mkdir -p "$HOME" "$CARGO_HOME"
      cargo build --offline --release \
        -p blob-nix-nixos-materialization-authority \
        --example root_materialization_authority_vm
      cargo build --offline --release \
        -p blob-nix-nixos-request-publisher \
        --example root_request_publisher_vm
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/bin"
      cp target/release/examples/root_materialization_authority_vm \
        "$out/bin/blob-materialization-authority-vm"
      cp target/release/examples/root_request_publisher_vm \
        "$out/bin/blob-root-request-publisher-vm"
      runHook postInstall
    '';
  };

  materializationFlakeFile = pkgs.writeText "blob-materialization-to-request-flake.nix" ''
    {
      outputs = { self }: {
        packages.x86_64-linux.candidate = builtins.derivation {
          name = "blob-materialization-to-request-candidate";
          system = "x86_64-linux";
          builder = "''${self.outPath}/builder";
          args = [
            "sh"
            "-c"
            "\"''${self.outPath}/builder\" mkdir -p \"$out\"; printf '%s\\n' MATERIALIZED_FOR_PUBLISHER > \"$out/blob-marker\""
          ];
        };
      };
    }
  '';

  # Pending recovery requires the exact immutable source itself to remain
  # available across reboot. Keep the test source self-contained by embedding a
  # static BusyBox builder in the same store object that carries flake.nix.
  materializationFlake = pkgs.runCommand "blob-materialization-to-request-flake" { } ''
    mkdir -p "$out"
    cp ${materializationFlakeFile} "$out/flake.nix"
    cp ${pkgs.pkgsStatic.busybox}/bin/busybox "$out/builder"
    chmod 0555 "$out/builder"
  '';

  blobPolkitActions = pkgs.writeTextDir "share/polkit-1/actions/org.theblob.nixos.policy" ''
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE policyconfig PUBLIC
      "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
      "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
    <policyconfig>
      <vendor>The Blob</vendor>
      <vendor_url>https://github.com/kinderp/theblob</vendor_url>
      <action id="${previewAction}">
        <description>Prepare an exact NixOS activation preview</description>
        <message>Authentication is required for the exact NixOS activation preview</message>
        <defaults>
          <allow_any>no</allow_any>
          <allow_inactive>auth_admin</allow_inactive>
          <allow_active>auth_admin</allow_active>
        </defaults>
      </action>
    </policyconfig>
  '';

  blobDbusPolicy = pkgs.writeTextDir "share/dbus-1/system.d/org.theblob.NixOsRoot.conf" ''
    <!DOCTYPE busconfig PUBLIC
      "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
      "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
    <busconfig>
      <policy user="root">
        <allow own="${busName}"/>
      </policy>
      <policy context="default">
        <allow send_destination="${busName}"/>
      </policy>
    </busconfig>
  '';

  python = pkgs.python3.withPackages (ps: [ ps.pygobject3 ]);

  rootService = pkgs.writeTextFile {
    name = "blob-materialization-to-request-service";
    executable = true;
    text = ''
      #!${python}/bin/python3
      import subprocess
      from gi.repository import Gio, GLib

      BUS_NAME = ${builtins.toJSON busName}
      OBJECT_PATH = ${builtins.toJSON objectPath}
      INTERFACE = ${builtins.toJSON interfaceName}
      PUBLISHER = ${builtins.toJSON "${rootHarnesses}/bin/blob-root-request-publisher-vm"}
      PKCHECK = ${builtins.toJSON "${pkgs.polkit}/bin/pkcheck"}

      INTROSPECTION = f"""
      <node>
        <interface name='{INTERFACE}'>
          <method name='Prepare'>
            <arg type='s' name='materialization_operation' direction='in'/>
            <arg type='s' name='action' direction='in'/>
            <arg type='s' name='authorization' direction='out'/>
            <arg type='s' name='evidence' direction='out'/>
          </method>
        </interface>
      </node>
      """

      def run_checked(command):
          result = subprocess.run(
              command,
              stdin=subprocess.DEVNULL,
              stdout=subprocess.PIPE,
              stderr=subprocess.PIPE,
              text=True,
              check=False,
          )
          if result.returncode != 0:
              detail = result.stderr.strip().replace("\n", " | ")
              raise RuntimeError(detail[-1800:] or "root publisher rejected")
          return result.stdout.strip()

      def published_authorization(evidence):
          for line in evidence.splitlines():
              if line.startswith("authorization="):
                  value = line.split("=", 1)[1]
                  if value.startswith("auth:published-"):
                      return value
          raise RuntimeError("publisher did not return a valid authorization id")

      def on_method_call(connection, sender, object_path, interface_name, method_name, parameters, invocation):
          if not sender or not sender.startswith(":"):
              invocation.return_dbus_error(
                  "org.theblob.Error.InvalidSender",
                  "The system bus did not provide a unique sender name",
              )
              return
          if method_name != "Prepare":
              invocation.return_dbus_error(
                  "org.theblob.Error.UnsupportedMethod",
                  "Unsupported root request method",
              )
              return

          try:
              materialization_operation, action = parameters.unpack()
              if action != "preview":
                  raise RuntimeError("only preview is exposed by this composition checkpoint")
              evidence = run_checked([
                  PUBLISHER,
                  "--sender", sender,
                  "--materialization-operation", materialization_operation,
                  "--action", action,
                  "--pkcheck", PKCHECK,
              ])
              authorization = published_authorization(evidence)
              invocation.return_value(GLib.Variant("(ss)", (authorization, evidence)))
          except Exception as error:
              invocation.return_dbus_error(
                  "org.theblob.Error.PreparationRejected",
                  str(error),
              )

      node = Gio.DBusNodeInfo.new_for_xml(INTROSPECTION)
      connection = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
      reply = connection.call_sync(
          "org.freedesktop.DBus",
          "/org/freedesktop/DBus",
          "org.freedesktop.DBus",
          "RequestName",
          GLib.Variant("(su)", (BUS_NAME, 4)),
          GLib.VariantType.new("(u)"),
          Gio.DBusCallFlags.NONE,
          -1,
          None,
      )
      if reply.unpack()[0] != 1:
          raise RuntimeError("failed to become primary owner of " + BUS_NAME)

      registration = connection.register_object(
          OBJECT_PATH,
          node.interfaces[0],
          on_method_call,
          None,
          None,
      )
      if registration == 0:
          raise RuntimeError("failed to register D-Bus object")
      GLib.MainLoop().run()
    '';
  };
in
{
  name = "blob-materialization-to-request";

  nodes.machine = { ... }: {
    nix.settings.experimental-features = [ "nix-command" "flakes" ];
    nix.settings.substituters = lib.mkForce [ ];
    # The source identity persisted in a pending intent must remain available for
    # recovery. In the VM, an explicit system closure dependency models that
    # retention requirement without retaining arbitrary derived outputs.
    system.extraDependencies = [ materializationFlake ];

    security.polkit.enable = true;
    security.polkit.extraConfig = ''
      polkit.addRule(function(action, subject) {
        if (subject.user == "alice" && action.id == "${previewAction}") {
          return "yes";
        }
      });
    '';
    services.dbus.packages = [ blobDbusPolicy ];

    users.users.alice = { isNormalUser = true; uid = 1000; };

    environment.systemPackages = [
      blobPolkitActions
      pkgs.dbus
      pkgs.nix
      pkgs.polkit
      rootHarnesses
    ];

    systemd.tmpfiles.rules = [
      "d /var/lib/theblob 0700 root root -"
      "d /var/lib/theblob/materialization-intents 0700 root root -"
      "d /var/lib/theblob/materialization-intents/pending 0700 root root -"
      "d /var/lib/theblob/materialization-intents/completed 0700 root root -"
      "d /var/lib/theblob/materialization-admissions 0700 root root -"
      "d /var/lib/theblob/prepared-activations 0700 root root -"
      "d /var/lib/theblob/prepared-activations/ready 0700 root root -"
      "d /var/lib/theblob/prepared-activations/inflight 0700 root root -"
      "d /var/lib/theblob/prepared-activations/completed 0700 root root -"
      "d /var/lib/theblob/prepared-activations/failed 0700 root root -"
    ];

    systemd.services.blob-materialization-to-request = {
      description = "The Blob materialization admission to request composition service";
      wantedBy = [ "multi-user.target" ];
      after = [ "dbus.service" "polkit.service" "systemd-tmpfiles-setup.service" ];
      requires = [ "dbus.service" ];
      wants = [ "polkit.service" ];
      restartIfChanged = false;
      serviceConfig = {
        Type = "simple";
        User = "root";
        Group = "root";
        ExecStart = "${rootService}";
        Restart = "on-failure";
        RestartSec = "100ms";
        NoNewPrivileges = true;
        PrivateTmp = true;
      };
    };

    # runNixOSTest normally puts the writable overlay of /nix/store on tmpfs,
    # which intentionally discards dynamically evaluated/built store paths at
    # reboot. Recovery must model an installed machine with a persistent writable
    # Nix store, so place that overlay on the VM disk instead.
    virtualisation.writableStore = true;
    virtualisation.writableStoreUseTmpfs = false;
    virtualisation.diskSize = 12288;
    virtualisation.memorySize = 1536;
    virtualisation.cores = 2;
  };

  testScript = ''
    import re
    import shlex

    AUTHORITY = "${rootHarnesses}/bin/blob-materialization-authority-vm"
    SOURCE = "${materializationFlake}"
    NIX = "${pkgs.nix}/bin/nix"
    NIX_STORE = "${pkgs.nix}/bin/nix-store"
    DEST = "${busName}"
    PATH = "${objectPath}"
    IFACE = "${interfaceName}"
    OP = "op:materialization-to-request"
    CANDIDATE_ID = "candidate:materialization-to-request"
    SYSTEM_SPEC = "system:materialization-to-request"
    INTENTS = "/var/lib/theblob/materialization-intents"
    ADMISSIONS = "/var/lib/theblob/materialization-admissions"
    REQUESTS = "/var/lib/theblob/prepared-activations"

    def field(output, key):
        match = re.search(r"^" + re.escape(key) + r"=(.+)$", output, re.MULTILINE)
        assert match is not None, (key, output)
        return match.group(1).strip()

    def authority(mode):
        command = (
            AUTHORITY
            + " --mode " + mode
            + " --operation " + shlex.quote(OP)
            + " --nix " + shlex.quote(NIX)
            + " --nix-store " + shlex.quote(NIX_STORE)
        )
        if mode == "begin":
            command += (
                " --candidate " + shlex.quote(CANDIDATE_ID)
                + " --system-spec " + shlex.quote(SYSTEM_SPEC)
                + " --source " + shlex.quote(SOURCE)
                + " --attribute packages.x86_64-linux.candidate"
            )
        return machine.execute(command + " 2>&1")

    def prepare_preview():
        inner = (
            "busctl --system call " + DEST + " " + PATH + " " + IFACE
            + " Prepare ss " + shlex.quote(OP) + " preview 2>&1"
        )
        return machine.execute("su -s /bin/sh alice -c " + shlex.quote(inner))

    def ready_count():
        return int(machine.succeed(
            "find " + REQUESTS + "/ready -maxdepth 1 -type f -name '*.request' | wc -l"
        ).strip())

    machine.start(allow_reboot = True)
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("blob-materialization-to-request.service")
    machine.succeed("test -f " + shlex.quote(SOURCE + "/flake.nix"))
    machine.succeed("test -x " + shlex.quote(SOURCE + "/builder"))

    baseline = machine.succeed("readlink -f /run/current-system").strip()
    first_boot = machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()

    # Root commits candidate/SystemSpec/source/attribute/.drv/output before any
    # ordinary-user materialization exists.
    status, begin_output = authority("begin")
    assert status == 0, (status, begin_output)
    derivation = field(begin_output, "derivation")
    expected = field(begin_output, "expected-output")
    target = field(begin_output, "build-target")
    assert target == derivation + "^out", (target, derivation)
    machine.succeed("test ! -e " + shlex.quote(expected))
    assert ready_count() == 0
    machine.succeed("test -z \"$(find " + ADMISSIONS + " -maxdepth 1 -type f -print -quit)\"")

    # Recovery before realization: reboot, retain the exact immutable source,
    # then re-resolve from root-owned source+attribute and require exact identity.
    machine.reboot()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("blob-materialization-to-request.service")
    machine.succeed("test -f " + shlex.quote(SOURCE + "/flake.nix"))
    machine.succeed("test -x " + shlex.quote(SOURCE + "/builder"))
    second_boot = machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()
    assert second_boot != first_boot, (first_boot, second_boot)
    status, resume_output = authority("resume")
    assert status == 0, (status, resume_output)
    assert field(resume_output, "derivation") == derivation, resume_output
    assert field(resume_output, "expected-output") == expected, resume_output
    assert field(resume_output, "build-target") == target, resume_output

    # The non-root materializer can realize the committed target but cannot read
    # or rewrite the pending root intent.
    pending = machine.succeed(
        "find " + INTENTS + "/pending -maxdepth 1 -type f -name '*.intent' -print -quit"
    ).strip()
    assert pending, pending
    machine.fail("su -s /bin/sh alice -c " + shlex.quote("cat " + pending))
    machine.succeed(
        "su -s /bin/sh alice -c "
        + shlex.quote("HOME=/home/alice " + NIX + " build --no-link --no-write-lock-file " + shlex.quote(target))
    )
    machine.succeed("grep -qx MATERIALIZED_FOR_PUBLISHER " + shlex.quote(expected + "/blob-marker"))

    # Recovery after realization but before admission: the immutable source is
    # retained again; recovery must revalidate the same committed identity.
    machine.reboot()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("blob-materialization-to-request.service")
    machine.succeed("test -f " + shlex.quote(SOURCE + "/flake.nix"))
    machine.succeed("test -x " + shlex.quote(SOURCE + "/builder"))
    third_boot = machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()
    assert third_boot != second_boot, (second_boot, third_boot)
    status, resume_after_build = authority("resume")
    assert status == 0, (status, resume_after_build)
    assert field(resume_after_build, "derivation") == derivation, resume_after_build
    assert field(resume_after_build, "expected-output") == expected, resume_after_build
    assert field(resume_after_build, "build-target") == target, resume_after_build

    status, complete_output = authority("complete")
    assert status == 0, (status, complete_output)
    assert field(complete_output, "closure") == expected, complete_output
    machine.succeed("test -z \"$(find " + INTENTS + "/pending -maxdepth 1 -type f -print -quit)\"")
    admission = machine.succeed(
        "find " + ADMISSIONS + " -maxdepth 1 -type f -name '*.admission' -print -quit"
    ).strip()
    assert admission, admission
    machine.succeed("test \"$(stat -c '%u:%a' " + shlex.quote(admission) + ")\" = 0:600")
    machine.fail("su -s /bin/sh alice -c " + shlex.quote("cat " + admission))

    # Once completed, recovery cannot reopen the operation.
    status, replay_resume = authority("resume")
    assert status != 0, (status, replay_resume)

    # Give the root readiness probe the installed boot-default invariant used by
    # the real request publisher, then prepare from the admission created above.
    machine.succeed("nix-env --profile /nix/var/nix/profiles/system --set " + shlex.quote(baseline))
    machine.wait_for_unit("polkit.service")
    status, prepared_output = prepare_preview()
    assert status == 0, (status, prepared_output)
    assert expected in prepared_output, prepared_output
    assert CANDIDATE_ID in prepared_output, prepared_output
    assert SYSTEM_SPEC in prepared_output, prepared_output
    assert OP in prepared_output, prepared_output
    assert ready_count() == 1

    ready = machine.succeed(
        "find " + REQUESTS + "/ready -maxdepth 1 -type f -name '*.request' -print -quit"
    ).strip()
    assert ready, ready
    machine.succeed("test \"$(stat -c '%u:%a' " + shlex.quote(ready) + ")\" = 0:600")
    machine.fail("su -s /bin/sh alice -c " + shlex.quote("cat " + ready))

    # The publisher cannot prepare an operation that never received a root
    # materialization admission.
    inner = (
        "busctl --system call " + DEST + " " + PATH + " " + IFACE
        + " Prepare ss op:not-admitted preview 2>&1"
    )
    status, unknown_output = machine.execute(
        "su -s /bin/sh alice -c " + shlex.quote(inner)
    )
    assert status != 0, (status, unknown_output)
    assert ready_count() == 1
  '';
}
