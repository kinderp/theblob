{ pkgs, lib, ... }:
let
  busName = "org.theblob.NixOsRoot";
  objectPath = "/org/theblob/NixOsRoot";
  interfaceName = "org.theblob.NixOsRoot1";
  previewAction = "org.theblob.nixos.preview-activation";
  testAction = "org.theblob.nixos.test-activation";

  rootHarnesses = pkgs.stdenv.mkDerivation {
    pname = "blob-root-request-publisher-vm";
    version = "0.1.0";
    src = lib.cleanSource ../../../..;
    nativeBuildInputs = [ pkgs.cargo pkgs.rustc ];
    buildPhase = ''
      runHook preBuild
      export HOME="$TMPDIR/home"
      export CARGO_HOME="$TMPDIR/cargo-home"
      mkdir -p "$HOME" "$CARGO_HOME"
      cargo build --offline --release \
        -p blob-nix-nixos-request-publisher \
        --example root_request_publisher_vm
      cargo build --offline --release \
        -p blob-nix-nixos-request-publisher \
        --example render_materialization_admission
      cargo build --offline --release \
        -p blob-nix-nixos-authority \
        --example root_prepared_request_daemon_vm
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/bin"
      cp target/release/examples/root_request_publisher_vm \
        "$out/bin/blob-root-request-publisher-vm"
      cp target/release/examples/render_materialization_admission \
        "$out/bin/blob-render-materialization-admission"
      cp target/release/examples/root_prepared_request_daemon_vm \
        "$out/bin/blob-root-prepared-request-daemon-vm"
      runHook postInstall
    '';
  };

  blobPolkitActions = pkgs.writeTextDir "share/polkit-1/actions/org.theblob.nixos.policy" ''
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE policyconfig PUBLIC
      "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
      "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
    <policyconfig>
      <vendor>The Blob</vendor>
      <vendor_url>https://github.com/kinderp/theblob</vendor_url>
      <action id="${previewAction}">
        <description>Prepare or execute an exact NixOS activation preview</description>
        <message>Authentication is required for the exact NixOS activation preview</message>
        <defaults>
          <allow_any>no</allow_any>
          <allow_inactive>auth_admin</allow_inactive>
          <allow_active>auth_admin</allow_active>
        </defaults>
      </action>
      <action id="${testAction}">
        <description>Prepare or execute an exact temporary NixOS activation</description>
        <message>Authentication is required for the exact temporary NixOS activation</message>
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
    name = "blob-root-request-publisher-service";
    executable = true;
    text = ''
      #!${python}/bin/python3
      import subprocess
      from gi.repository import Gio, GLib

      BUS_NAME = ${builtins.toJSON busName}
      OBJECT_PATH = ${builtins.toJSON objectPath}
      INTERFACE = ${builtins.toJSON interfaceName}
      PUBLISHER = ${builtins.toJSON "${rootHarnesses}/bin/blob-root-request-publisher-vm"}
      EXECUTOR = ${builtins.toJSON "${rootHarnesses}/bin/blob-root-prepared-request-daemon-vm"}
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
          <method name='Execute'>
            <arg type='s' name='authorization' direction='in'/>
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
              raise RuntimeError(detail[-1800:] or "root harness rejected")
          return result.stdout.strip()

      def published_authorization(evidence):
          for line in evidence.splitlines():
              if line.startswith("authorization="):
                  value = line.split("=", 1)[1]
                  if value.startswith("auth:published-"):
                      return value
          raise RuntimeError("publisher did not return a valid root-generated authorization id")

      def on_method_call(connection, sender, object_path, interface_name, method_name, parameters, invocation):
          if not sender or not sender.startswith(":"):
              invocation.return_dbus_error(
                  "org.theblob.Error.InvalidSender",
                  "The system bus did not provide a unique sender name",
              )
              return

          try:
              if method_name == "Prepare":
                  materialization_operation, action = parameters.unpack()
                  if action not in ("preview", "test"):
                      raise RuntimeError("unsupported prepared activation action")
                  evidence = run_checked([
                      PUBLISHER,
                      "--sender", sender,
                      "--materialization-operation", materialization_operation,
                      "--action", action,
                      "--pkcheck", PKCHECK,
                  ])
                  authorization = published_authorization(evidence)
                  invocation.return_value(GLib.Variant("(ss)", (authorization, evidence)))
                  return

              if method_name == "Execute":
                  authorization = parameters.unpack()[0]
                  evidence = run_checked([
                      EXECUTOR,
                      "--sender", sender,
                      "--authorization", authorization,
                      "--pkcheck", PKCHECK,
                  ])
                  invocation.return_value(GLib.Variant("(s)", (evidence,)))
                  return

              invocation.return_dbus_error(
                  "org.theblob.Error.UnsupportedMethod",
                  "Unsupported root activation method",
              )
          except Exception as error:
              invocation.return_dbus_error(
                  "org.theblob.Error.ActivationRejected",
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
  name = "blob-root-request-publisher";

  nodes.machine = { ... }: {
    environment.etc."blob-activation-state".text = "BASELINE\n";

    specialisation.blob-candidate = {
      inheritParentConfig = true;
      configuration = {
        system.nixos.tags = [ "blob-request-publisher-candidate" ];
        environment.etc."blob-activation-state".text = lib.mkForce "CANDIDATE\n";
      };
    };

    security.polkit.enable = true;
    security.polkit.extraConfig = ''
      polkit.addRule(function(action, subject) {
        if (subject.user == "alice" && action.id == "${previewAction}") {
          return "yes";
        }
        if (subject.user == "alice" && action.id == "${testAction}") {
          return "no";
        }
        if (subject.user == "bob" && action.id == "${testAction}") {
          return "yes";
        }
      });
    '';

    services.dbus.packages = [ blobDbusPolicy ];

    users.users.alice = { isNormalUser = true; uid = 1000; };
    users.users.bob = { isNormalUser = true; uid = 1001; };

    environment.systemPackages = [
      blobPolkitActions
      pkgs.dbus
      pkgs.polkit
      rootHarnesses
    ];

    systemd.tmpfiles.rules = [
      "d /var/lib/theblob 0700 root root -"
      "d /var/lib/theblob/activation-permits 0700 root root -"
      "d /var/lib/theblob/privileged-executions 0700 root root -"
      "d /var/lib/theblob/prepared-activations 0700 root root -"
      "d /var/lib/theblob/prepared-activations/ready 0700 root root -"
      "d /var/lib/theblob/prepared-activations/inflight 0700 root root -"
      "d /var/lib/theblob/prepared-activations/completed 0700 root root -"
      "d /var/lib/theblob/prepared-activations/failed 0700 root root -"
      "d /var/lib/theblob/materialization-admissions 0700 root root -"
    ];

    systemd.services.blob-root-request-publisher = {
      description = "The Blob root materialization-to-prepared-request publisher";
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

    # Keep the real Linux Pilot free-space gate instead of lowering it for CI.
    # The image is sparse, so this does not allocate 12 GiB eagerly on the runner.
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
    ADMISSION_FIXTURE = "${rootHarnesses}/bin/blob-render-materialization-admission"
    ADMISSIONS = "/var/lib/theblob/materialization-admissions"
    REQUESTS = "/var/lib/theblob/prepared-activations"
    PERMITS = "/var/lib/theblob/activation-permits"
    LEDGER = "/var/lib/theblob/privileged-executions"
    NODE = "node:blob-prepared-request-daemon-vm"
    MAT = "op:publisher-materialize"
    BAD = "op:publisher-bad-mode"
    CANDIDATE_ID = "candidate:publisher"
    SYSTEM_SPEC = "system:publisher"

    def encoded(value):
        return value.encode("utf-8").hex()

    def admission_path(operation):
        return ADMISSIONS + "/operation-" + encoded(operation) + ".admission"

    def request_path(state, authorization):
        return REQUESTS + "/" + state + "/authorization-" + encoded(authorization) + ".request"

    def stage_admission(operation, candidate):
        now_ms = machine.succeed("date +%s%3N").strip()
        path = admission_path(operation)
        command = (
            "umask 077; " + shlex.quote(ADMISSION_FIXTURE)
            + " --materialization-operation " + shlex.quote(operation)
            + " --node " + shlex.quote(NODE)
            + " --candidate " + shlex.quote(CANDIDATE_ID)
            + " --system-spec " + shlex.quote(SYSTEM_SPEC)
            + " --system-closure " + shlex.quote(candidate)
            + " --admitted-at-ms " + shlex.quote(now_ms)
            + " > " + shlex.quote(path)
        )
        machine.succeed(command)
        machine.succeed("test \"$(stat -c %u:%a " + shlex.quote(path) + ")\" = 0:600")

    def call(user, method, signature, *args):
        tail = ""
        if signature:
            tail = " " + signature + " " + " ".join(shlex.quote(arg) for arg in args)
        inner = "busctl --system call " + DEST + " " + PATH + " " + IFACE + " " + method + tail + " 2>&1"
        return machine.execute("su -s /bin/sh " + user + " -c " + shlex.quote(inner))

    def prepare(user, operation, action):
        status, output = call(user, "Prepare", "ss", operation, action)
        if status != 0:
            return status, output, None
        quoted = re.findall(r'"([^"\\]*(?:\\.[^"\\]*)*)"', output)
        auth = next((value for value in quoted if value.startswith("auth:published-")), None)
        return status, output, auth

    def execute(user, authorization):
        return call(user, "Execute", "s", authorization)

    def ready_count():
        return int(machine.succeed("find " + REQUESTS + "/ready -maxdepth 1 -type f -name '*.request' | wc -l").strip())

    def ledger_count():
        return int(machine.succeed("find " + LEDGER + " -maxdepth 1 -type f -name '*.used' | wc -l").strip())

    def assert_no_live_permit():
        machine.succeed("test -z \"$(find " + PERMITS + " -maxdepth 1 -type f -print -quit)\"")

    machine.start(allow_reboot = True)
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("polkit.service")
    machine.wait_for_unit("blob-root-request-publisher.service")

    baseline = machine.succeed("readlink -f /run/current-system").strip()
    candidate = machine.succeed("readlink -f /run/current-system/specialisation/blob-candidate").strip()
    baseline_boot_id = machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()
    assert baseline.startswith("/nix/store/"), baseline
    assert candidate.startswith("/nix/store/"), candidate
    assert candidate != baseline, (baseline, candidate)
    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")
    assert ledger_count() == 0
    assert ready_count() == 0
    assert_no_live_permit()

    stage_admission(MAT, candidate)
    machine.succeed("su -s /bin/sh alice -c 'test ! -r " + admission_path(MAT) + "'")

    # Unknown admission cannot inject a closure or create a ready request.
    status, output, auth = prepare("alice", "op:does-not-exist", "preview")
    assert status != 0 and auth is None, (status, output, auth)
    assert ready_count() == 0

    # A root-owned admission with the wrong mode is rejected before polkit/publication.
    stage_admission(BAD, candidate)
    machine.succeed("chmod 0644 " + shlex.quote(admission_path(BAD)))
    status, output, auth = prepare("alice", BAD, "preview")
    assert status != 0 and auth is None, (status, output, auth)
    assert ready_count() == 0

    # Before the VM has an installed boot-default profile, live activation readiness
    # is incomplete. Bob's test preparation fails before a request can be published.
    status, output, auth = prepare("bob", MAT, "test")
    assert status != 0 and auth is None, (status, output, auth)
    assert ready_count() == 0

    # Install the conventional boot profile used by the real privileged boundary
    # and by the root read-only readiness probe.
    machine.succeed("nix-env --profile /nix/var/nix/profiles/system --set " + shlex.quote(baseline))
    assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == baseline

    # Alice can prepare preview. The root publisher creates the authorization id,
    # reads candidate/SystemSpec/closure from the admission, probes readiness itself,
    # obtains real polkit authorization, and only then creates the 0600 ready file.
    status, output, preview_auth = prepare("alice", MAT, "preview")
    assert status == 0 and preview_auth is not None, (status, output, preview_auth)
    assert "closure=" + candidate in output, output
    assert "candidate=" + CANDIDATE_ID in output, output
    assert "system-spec=" + SYSTEM_SPEC in output, output
    machine.succeed("test -f " + shlex.quote(request_path("ready", preview_auth)))
    machine.succeed("test \"$(stat -c %u:%a " + shlex.quote(request_path("ready", preview_auth)) + ")\" = 0:600")
    assert ready_count() == 1

    # Execution still requires the independent second polkit grant and the crash-safe
    # claim/permit/root-boundary chain proved in #27.
    status, output = execute("alice", preview_auth)
    assert status == 0, (status, output)
    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")
    assert machine.succeed("readlink -f /run/current-system").strip() == baseline
    machine.succeed("test -f " + shlex.quote(request_path("completed", preview_auth)))
    assert ledger_count() == 1
    assert_no_live_permit()

    # Alice is denied test preparation by real polkit. No random request is left behind.
    ready_before = ready_count()
    status, output, denied_auth = prepare("alice", MAT, "test")
    assert status != 0 and denied_auth is None, (status, output, denied_auth)
    assert ready_count() == ready_before
    assert ledger_count() == 1

    # Bob can prepare and then independently authorize execution of the exact same
    # admitted materialization as a temporary live activation.
    status, output, test_auth = prepare("bob", MAT, "test")
    assert status == 0 and test_auth is not None, (status, output, test_auth)
    assert test_auth != preview_auth, (preview_auth, test_auth)
    machine.succeed("test -f " + shlex.quote(request_path("ready", test_auth)))
    status, output = execute("bob", test_auth)
    assert status == 0, (status, output)
    machine.wait_until_succeeds("grep -qx CANDIDATE /etc/blob-activation-state")
    assert machine.succeed("readlink -f /run/current-system").strip() == candidate
    assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == baseline
    assert ledger_count() == 2
    assert_no_live_permit()

    # Reboot remains the hard recovery boundary: baseline returns and published
    # completed requests cannot execute again.
    machine.reboot()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("blob-root-request-publisher.service")
    reboot_boot_id = machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()
    assert reboot_boot_id != baseline_boot_id, (baseline_boot_id, reboot_boot_id)
    machine.wait_until_succeeds("grep -qx BASELINE /etc/blob-activation-state")
    assert machine.succeed("readlink -f /run/current-system").strip() == baseline
    assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == baseline
    assert ledger_count() == 2
    assert_no_live_permit()
    machine.succeed("test -f " + shlex.quote(request_path("completed", preview_auth)))
    machine.succeed("test -f " + shlex.quote(request_path("completed", test_auth)))
    status, output = execute("bob", test_auth)
    assert status != 0, (status, output)
    assert ledger_count() == 2
    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")
  '';
}
