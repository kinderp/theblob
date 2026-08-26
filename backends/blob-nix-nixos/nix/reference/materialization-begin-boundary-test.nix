{ pkgs, lib, ... }:
let
  busName = "org.theblob.NixOsMaterialization";
  objectPath = "/org/theblob/NixOsMaterialization";
  interfaceName = "org.theblob.NixOsMaterialization1";

  rootHarnesses = pkgs.stdenv.mkDerivation {
    pname = "blob-materialization-begin-boundary-vm";
    version = "0.1.0";
    src = lib.cleanSource ../../../..;
    nativeBuildInputs = [ pkgs.cargo pkgs.rustc ];
    buildPhase = ''
      runHook preBuild
      export HOME="$TMPDIR/home"
      export CARGO_HOME="$TMPDIR/cargo-home"
      mkdir -p "$HOME" "$CARGO_HOME"
      cargo build --offline --release \
        -p blob-nix-nixos-materialization-begin \
        --example root_materialization_begin_vm
      cargo build --offline --release \
        -p blob-nix-nixos-materialization-begin \
        --example render_trusted_candidate
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/bin"
      cp target/release/examples/root_materialization_begin_vm \
        "$out/bin/blob-materialization-begin-vm"
      cp target/release/examples/render_trusted_candidate \
        "$out/bin/blob-render-trusted-candidate"
      runHook postInstall
    '';
  };

  materializationFlakeFile = pkgs.writeText "blob-materialization-begin-flake.nix" ''
    {
      outputs = { self }: {
        packages.x86_64-linux.candidate = builtins.derivation {
          name = "blob-materialization-begin-candidate";
          system = "x86_64-linux";
          builder = "''${self.outPath}/busybox";
          args = [
            "sh"
            "-c"
            "\"''${self.outPath}/busybox\" mkdir -p \"$out\"; printf '%s\\n' TRUSTED_MANIFEST_BUILD_OK > \"$out/blob-marker\""
          ];
        };
      };
    }
  '';

  materializationFlake = pkgs.runCommand "blob-materialization-begin-flake" { } ''
    mkdir -p "$out"
    cp ${materializationFlakeFile} "$out/flake.nix"
    cp ${pkgs.pkgsStatic.busybox}/bin/busybox "$out/busybox"
    chmod 0555 "$out/busybox"
  '';

  blobDbusPolicy = pkgs.writeTextDir "share/dbus-1/system.d/org.theblob.NixOsMaterialization.conf" ''
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
    name = "blob-materialization-begin-boundary-service";
    executable = true;
    text = ''
      #!${python}/bin/python3
      import subprocess
      from gi.repository import Gio, GLib

      BUS_NAME = ${builtins.toJSON busName}
      OBJECT_PATH = ${builtins.toJSON objectPath}
      INTERFACE = ${builtins.toJSON interfaceName}
      BEGIN = ${builtins.toJSON "${rootHarnesses}/bin/blob-materialization-begin-vm"}
      NIX = ${builtins.toJSON "${pkgs.nix}/bin/nix"}
      NIX_STORE = ${builtins.toJSON "${pkgs.nix}/bin/nix-store"}

      INTROSPECTION = f"""
      <node>
        <interface name='{INTERFACE}'>
          <method name='Begin'>
            <arg type='s' name='manifest_id' direction='in'/>
            <arg type='s' name='operation' direction='out'/>
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
              raise RuntimeError(detail[-1800:] or "root begin boundary rejected")
          return result.stdout.strip()

      def operation_from(evidence):
          for line in evidence.splitlines():
              if line.startswith("operation="):
                  operation = line.split("=", 1)[1]
                  if operation.startswith("op:materialize-"):
                      return operation
          raise RuntimeError("boundary did not return a root-generated operation id")

      def on_method_call(connection, sender, object_path, interface_name, method_name, parameters, invocation):
          if not sender or not sender.startswith(":"):
              invocation.return_dbus_error(
                  "org.theblob.Error.InvalidSender",
                  "The system bus did not provide a unique sender name",
              )
              return
          if method_name != "Begin":
              invocation.return_dbus_error(
                  "org.theblob.Error.UnsupportedMethod",
                  "Unsupported materialization method",
              )
              return
          try:
              manifest_id = parameters.unpack()[0]
              evidence = run_checked([
                  BEGIN,
                  "--mode", "begin",
                  "--manifest-id", manifest_id,
                  "--nix", NIX,
                  "--nix-store", NIX_STORE,
              ])
              operation = operation_from(evidence)
              evidence = "sender=" + sender + "\n" + evidence
              invocation.return_value(GLib.Variant("(ss)", (operation, evidence)))
          except Exception as error:
              invocation.return_dbus_error(
                  "org.theblob.Error.MaterializationBeginRejected",
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
          raise RuntimeError("failed to own " + BUS_NAME)
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
  name = "blob-materialization-begin-boundary";

  nodes.machine = { ... }: {
    nix.settings.experimental-features = [ "nix-command" "flakes" ];
    nix.settings.substituters = lib.mkForce [ ];
    system.extraDependencies = [ materializationFlake ];

    services.dbus.packages = [ blobDbusPolicy ];
    users.users.alice = { isNormalUser = true; uid = 1000; };

    environment.systemPackages = [ pkgs.dbus pkgs.nix rootHarnesses ];
    systemd.tmpfiles.rules = [
      "d /var/lib/theblob 0700 root root -"
      "d /var/lib/theblob/materialization-candidates 0700 root root -"
      "d /var/lib/theblob/materialization-intents 0700 root root -"
      "d /var/lib/theblob/materialization-intents/pending 0700 root root -"
      "d /var/lib/theblob/materialization-intents/completed 0700 root root -"
      "d /var/lib/theblob/materialization-admissions 0700 root root -"
      "d /nix/var/nix/gcroots/theblob-materializations 0700 root root -"
    ];

    systemd.services.blob-materialization-begin-boundary = {
      description = "The Blob trusted manifest materialization begin boundary";
      wantedBy = [ "multi-user.target" ];
      after = [ "dbus.service" "systemd-tmpfiles-setup.service" ];
      requires = [ "dbus.service" ];
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

    virtualisation.diskSize = 12288;
    virtualisation.memorySize = 1536;
    virtualisation.cores = 2;
  };

  testScript = ''
    import re
    import shlex

    DEST = "${busName}"
    PATH = "${objectPath}"
    IFACE = "${interfaceName}"
    RENDER = "${rootHarnesses}/bin/blob-render-trusted-candidate"
    BOUNDARY = "${rootHarnesses}/bin/blob-materialization-begin-vm"
    SOURCE = "${materializationFlake}"
    NIX = "${pkgs.nix}/bin/nix"
    NIX_STORE = "${pkgs.nix}/bin/nix-store"
    MANIFEST = "manifest:pilot-candidate"
    BAD_MANIFEST = "manifest:bad-mode"
    CANDIDATE = "candidate:trusted-begin"
    SYSTEM_SPEC = "system:trusted-begin"
    CANDIDATES = "/var/lib/theblob/materialization-candidates"
    INTENTS = "/var/lib/theblob/materialization-intents"
    ADMISSIONS = "/var/lib/theblob/materialization-admissions"
    GCROOTS = "/nix/var/nix/gcroots/theblob-materializations"

    def encoded(value):
        return value.encode("utf-8").hex()

    def manifest_path(manifest):
        return CANDIDATES + "/manifest-" + encoded(manifest) + ".candidate"

    def stage_manifest(manifest, mode="0600"):
        path = manifest_path(manifest)
        command = (
            "umask 077; " + shlex.quote(RENDER)
            + " --manifest-id " + shlex.quote(manifest)
            + " --candidate " + shlex.quote(CANDIDATE)
            + " --system-spec " + shlex.quote(SYSTEM_SPEC)
            + " --source " + shlex.quote(SOURCE)
            + " --attribute packages.x86_64-linux.candidate"
            + " > " + shlex.quote(path)
            + "; chmod " + mode + " " + shlex.quote(path)
        )
        machine.succeed(command)
        return path

    def begin(manifest, signature="s", *extra):
        args = " ".join([shlex.quote(manifest)] + [shlex.quote(value) for value in extra])
        inner = (
            "busctl --system call " + DEST + " " + PATH + " " + IFACE
            + " Begin " + signature + " " + args + " 2>&1"
        )
        return machine.execute("su -s /bin/sh alice -c " + shlex.quote(inner))

    def quoted(output):
        return re.findall(r'"([^"\\]*(?:\\.[^"\\]*)*)"', output)

    def field(evidence, key):
        match = re.search(r"^" + re.escape(key) + r"=(.+)$", evidence, re.MULTILINE)
        assert match is not None, (key, evidence)
        return match.group(1).strip()

    machine.start()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("blob-materialization-begin-boundary.service")
    machine.succeed("test -f " + shlex.quote(SOURCE + "/flake.nix"))
    machine.succeed("test -x " + shlex.quote(SOURCE + "/busybox"))

    good_path = stage_manifest(MANIFEST)
    bad_path = stage_manifest(BAD_MANIFEST, "0644")
    machine.succeed("test \"$(stat -c %u:%a " + shlex.quote(good_path) + ")\" = 0:600")
    machine.fail("su -s /bin/sh alice -c " + shlex.quote("cat " + good_path))

    # A caller cannot create work from an absent or malformed trusted record.
    status, output = begin("manifest:missing")
    assert status != 0, (status, output)
    status, output = begin(BAD_MANIFEST)
    assert status != 0, (status, output)
    machine.succeed("test -z \"$(find " + INTENTS + "/pending -maxdepth 1 -type f -print -quit)\"")
    machine.succeed("test -z \"$(find " + GCROOTS + " -maxdepth 1 -type l -print -quit)\"")

    # The public D-Bus shape has one input only. Trying to append a caller-chosen
    # source/closure-like value is rejected by D-Bus before the root harness runs.
    status, output = begin(MANIFEST, "ss", "/nix/store/evil")
    assert status != 0, (status, output)

    status, output = begin(MANIFEST)
    assert status == 0, (status, output)
    values = quoted(output)
    operation = next((value for value in values if value.startswith("op:materialize-")), None)
    evidence = next((value for value in values if "candidate=" in value and "derivation=" in value), None)
    assert operation is not None and evidence is not None, (operation, output)
    evidence = evidence.replace("\\n", "\n")

    assert field(evidence, "candidate") == CANDIDATE, evidence
    assert field(evidence, "system-spec") == SYSTEM_SPEC, evidence
    assert field(evidence, "source") == SOURCE, evidence
    assert field(evidence, "attribute") == "packages.x86_64-linux.candidate", evidence
    assert field(evidence, "operation") == operation, evidence
    assert field(evidence, "sender").startswith(":"), evidence
    derivation = field(evidence, "derivation")
    expected = field(evidence, "expected-output")
    target = field(evidence, "build-target")
    assert target == derivation + "^out", (target, derivation)

    pending = machine.succeed(
        "find " + INTENTS + "/pending -maxdepth 1 -type f -name '*.intent' -print -quit"
    ).strip()
    assert pending, pending
    machine.succeed("test \"$(stat -c %u:%a " + shlex.quote(pending) + ")\" = 0:600")
    machine.fail("su -s /bin/sh alice -c " + shlex.quote("cat " + pending))
    machine.succeed("test \"$(find " + GCROOTS + " -maxdepth 1 -type l | wc -l)\" = 1")
    assert machine.succeed("readlink $(find " + GCROOTS + " -maxdepth 1 -type l -print -quit)").strip() == derivation

    # Materialization remains non-privileged and uses only the root-committed target.
    alice_build = "HOME=/home/alice " + NIX + " build --no-link --no-write-lock-file " + shlex.quote(target)
    machine.succeed("su -s /bin/sh alice -c " + shlex.quote(alice_build))
    machine.succeed("grep -qx TRUSTED_MANIFEST_BUILD_OK " + shlex.quote(expected + "/blob-marker"))

    complete = (
        BOUNDARY
        + " --mode complete --operation " + shlex.quote(operation)
        + " --nix " + shlex.quote(NIX)
        + " --nix-store " + shlex.quote(NIX_STORE)
        + " 2>&1"
    )
    status, complete_output = machine.execute(complete)
    assert status == 0, (status, complete_output)
    assert "closure=" + expected in complete_output, complete_output
    machine.succeed("test -z \"$(find " + GCROOTS + " -maxdepth 1 -type l -print -quit)\"")
    machine.succeed("test -z \"$(find " + INTENTS + "/pending -maxdepth 1 -type f -print -quit)\"")
    machine.succeed("test -n \"$(find " + INTENTS + "/completed -maxdepth 1 -type f -print -quit)\"")
    machine.succeed("test -n \"$(find " + ADMISSIONS + " -maxdepth 1 -type f -print -quit)\"")
  '';
}
