{ pkgs, lib, ... }:
let
  busName = "org.theblob.NixOsRoot";
  objectPath = "/org/theblob/NixOsRoot";
  interfaceName = "org.theblob.NixOsRoot1";
  previewAction = "org.theblob.nixos.preview-activation";
  testAction = "org.theblob.nixos.test-activation";

  rootFlowHarness = pkgs.stdenv.mkDerivation {
    pname = "blob-root-prepared-request-daemon-vm";
    version = "0.1.0";
    src = lib.cleanSource ../../../..;
    nativeBuildInputs = [ pkgs.cargo pkgs.rustc ];
    buildPhase = ''
      runHook preBuild
      export HOME="$TMPDIR/home"
      export CARGO_HOME="$TMPDIR/cargo-home"
      mkdir -p "$HOME" "$CARGO_HOME"
      cargo build --offline --release \
        -p blob-nix-nixos-authority \
        --example root_prepared_request_daemon_vm
      cargo build --offline --release \
        -p blob-nix-nixos-request-store \
        --example render_prepared_request
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/bin"
      cp target/release/examples/root_prepared_request_daemon_vm \
        "$out/bin/blob-root-prepared-request-daemon-vm"
      cp target/release/examples/render_prepared_request \
        "$out/bin/blob-render-prepared-request"
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
        <description>Preview a staged exact NixOS activation</description>
        <message>Authentication is required to preview the staged NixOS activation</message>
        <defaults>
          <allow_any>no</allow_any>
          <allow_inactive>auth_admin</allow_inactive>
          <allow_active>auth_admin</allow_active>
        </defaults>
      </action>
      <action id="${testAction}">
        <description>Temporarily activate a staged exact NixOS system</description>
        <message>Authentication is required to temporarily activate the staged NixOS system</message>
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
    name = "blob-root-prepared-request-daemon-service";
    executable = true;
    text = ''
      #!${python}/bin/python3
      import os
      import subprocess
      from gi.repository import Gio, GLib

      BUS_NAME = ${builtins.toJSON busName}
      OBJECT_PATH = ${builtins.toJSON objectPath}
      INTERFACE = ${builtins.toJSON interfaceName}
      HARNESS = ${builtins.toJSON "${rootFlowHarness}/bin/blob-root-prepared-request-daemon-vm"}
      PKCHECK = ${builtins.toJSON "${pkgs.polkit}/bin/pkcheck"}
      FAULT_EXIT_CODE = 70

      INTROSPECTION = f"""
      <node>
        <interface name='{INTERFACE}'>
          <method name='Execute'>
            <arg type='s' name='authorization' direction='in'/>
            <arg type='s' name='observed_sender' direction='out'/>
            <arg type='s' name='evidence' direction='out'/>
          </method>
          <method name='CrashAfterClaim'>
            <arg type='s' name='authorization' direction='in'/>
          </method>
        </interface>
      </node>
      """

      def validate_request_id(value):
          if not value.startswith("auth:") or len(value) > 160:
              raise RuntimeError("invalid prepared request authorization id")
          if any(ord(ch) < 0x20 or ord(ch) == 0x7f for ch in value):
              raise RuntimeError("invalid prepared request authorization id")

      def run_request(sender, authorization, fault_after_claim):
          validate_request_id(authorization)
          command = [
              HARNESS,
              "--sender", sender,
              "--authorization", authorization,
              "--pkcheck", PKCHECK,
          ]
          if fault_after_claim:
              command.append("--fault-after-claim")
          result = subprocess.run(
              command,
              stdin=subprocess.DEVNULL,
              stdout=subprocess.PIPE,
              stderr=subprocess.PIPE,
              text=True,
              check=False,
          )
          if fault_after_claim and result.returncode == FAULT_EXIT_CODE:
              # Test-only crash injection. The production-shaped Execute method
              # has no fault parameter and never exposes this path to callers.
              os._exit(FAULT_EXIT_CODE)
          if result.returncode != 0:
              detail = result.stderr.strip().replace("\n", " | ")
              raise RuntimeError(detail[-1800:] or "prepared request harness rejected")
          return result.stdout.strip()

      def on_method_call(connection, sender, object_path, interface_name, method_name, parameters, invocation):
          # Identity comes only from the system-bus header. The sole production-
          # shaped input is an opaque authorization/request id; closure, action,
          # executable and argv come only from the root-owned staged request.
          if not sender or not sender.startswith(":"):
              invocation.return_dbus_error(
                  "org.theblob.Error.InvalidSender",
                  "The system bus did not provide a unique sender name",
              )
              return

          authorization = parameters.unpack()[0]
          try:
              if method_name == "Execute":
                  evidence = run_request(sender, authorization, False)
                  invocation.return_value(GLib.Variant("(ss)", (sender, evidence)))
                  return
              if method_name == "CrashAfterClaim":
                  run_request(sender, authorization, True)
                  invocation.return_value(GLib.Variant("()", ()))
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
  name = "blob-root-prepared-request-daemon";

  nodes.machine = { ... }: {
    environment.etc."blob-activation-state".text = "BASELINE\n";

    specialisation.blob-candidate = {
      inheritParentConfig = true;
      configuration = {
        system.nixos.tags = [ "blob-prepared-request-candidate" ];
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

    users.users.alice = {
      isNormalUser = true;
      uid = 1000;
    };
    users.users.bob = {
      isNormalUser = true;
      uid = 1001;
    };

    environment.systemPackages = [
      blobPolkitActions
      pkgs.dbus
      pkgs.polkit
      rootFlowHarness
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
    ];

    systemd.services.blob-root-prepared-request-daemon = {
      description = "The Blob prepared activation request root daemon";
      wantedBy = [ "multi-user.target" ];
      after = [ "dbus.service" "polkit.service" "systemd-tmpfiles-setup.service" ];
      requires = [ "dbus.service" ];
      wants = [ "polkit.service" ];
      # The daemon must survive the temporary system activation it supervises.
      # A crash restarts the daemon, but claimed requests remain inflight and are
      # never automatically replayed by the restarted process.
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
        # ProtectHome is intentionally absent: the fixed NixOS activator must be
        # able to perform legitimate user/home activation work.
      };
    };

    virtualisation.memorySize = 1536;
    virtualisation.cores = 2;
  };

  testScript = ''
    import shlex

    DEST = "${busName}"
    PATH = "${objectPath}"
    IFACE = "${interfaceName}"
    FIXTURE = "${rootFlowHarness}/bin/blob-render-prepared-request"
    REQUESTS = "/var/lib/theblob/prepared-activations"
    PERMITS = "/var/lib/theblob/activation-permits"
    LEDGER = "/var/lib/theblob/privileged-executions"

    PREVIEW = "auth:prepared-preview"
    CRASH = "auth:prepared-crash"
    TEST = "auth:prepared-test"

    def encoded(value):
        return value.encode("utf-8").hex()

    def request_path(state, authorization, suffix="request"):
        return REQUESTS + "/" + state + "/authorization-" + encoded(authorization) + "." + suffix

    def stage(authorization, action, candidate):
        now_ms = machine.succeed("date +%s%3N").strip()
        path = request_path("ready", authorization)
        command = (
            "umask 077; " + shlex.quote(FIXTURE)
            + " --authorization " + shlex.quote(authorization)
            + " --action " + shlex.quote(action)
            + " --candidate " + shlex.quote(candidate)
            + " --now-ms " + shlex.quote(now_ms)
            + " > " + shlex.quote(path)
        )
        machine.succeed(command)
        machine.succeed("test \"$(stat -c %u:%a " + shlex.quote(path) + ")\" = 0:600")

    def user_call(user, method, authorization):
        inner = (
            "busctl --system call " + DEST + " " + PATH + " " + IFACE + " "
            + method + " s " + shlex.quote(authorization) + " 2>&1"
        )
        return machine.execute("su -s /bin/sh " + user + " -c " + shlex.quote(inner))

    def ledger_count():
        return int(machine.succeed(
            "find " + LEDGER + " -maxdepth 1 -type f -name '*.used' | wc -l"
        ).strip())

    def assert_no_live_permit():
        machine.succeed("test -z \"$(find " + PERMITS + " -maxdepth 1 -type f -print -quit)\"")

    machine.start(allow_reboot = True)
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("polkit.service")
    machine.wait_for_unit("blob-root-prepared-request-daemon.service")

    baseline = machine.succeed("readlink -f /run/current-system").strip()
    candidate = machine.succeed(
        "readlink -f /run/current-system/specialisation/blob-candidate"
    ).strip()
    baseline_boot_id = machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()
    assert baseline.startswith("/nix/store/"), baseline
    assert candidate.startswith("/nix/store/"), candidate
    assert candidate != baseline, (baseline, candidate)

    # The test VM boots directly from a store path; install the conventional
    # boot-default profile required by the production privileged boundary.
    machine.succeed(
        "nix-env --profile /nix/var/nix/profiles/system --set " + shlex.quote(baseline)
    )
    assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == baseline
    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")
    assert ledger_count() == 0
    assert_no_live_permit()

    # Caller selects only an opaque staged request id. Unknown ids cannot smuggle
    # a closure/program/argv and fail before authorization or privilege effects.
    status, output = user_call("alice", "Execute", "auth:/nix/store/attacker")
    assert status != 0, (status, output)
    assert ledger_count() == 0
    assert_no_live_permit()

    # Successful preview: root-owned request -> live sender -> polkit -> durable
    # claim -> exact permit -> boundary. The request becomes terminal completed.
    stage(PREVIEW, "preview", candidate)
    status, output = user_call("alice", "Execute", PREVIEW)
    assert status == 0, (status, output)
    assert "request-state=completed" in output, output
    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")
    assert machine.succeed("readlink -f /run/current-system").strip() == baseline
    assert ledger_count() == 1
    assert_no_live_permit()
    machine.succeed("test -f " + shlex.quote(request_path("completed", PREVIEW)))
    machine.succeed("test ! -e " + shlex.quote(request_path("ready", PREVIEW)))

    # Terminal requests are not reopened, even if the caller is authorized again.
    status, output = user_call("alice", "Execute", PREVIEW)
    assert status != 0, (status, output)
    assert ledger_count() == 1
    assert_no_live_permit()

    # Crash after the durable claim but before permit issuance. systemd restarts
    # the daemon, but ready is gone, inflight survives, and no execution occurred.
    stage(CRASH, "preview", candidate)
    old_pid = machine.succeed(
        "systemctl show -P MainPID blob-root-prepared-request-daemon.service"
    ).strip()
    status, output = user_call("alice", "CrashAfterClaim", CRASH)
    assert status != 0, (status, output)
    machine.wait_until_succeeds(
        "test \"$(systemctl show -P MainPID blob-root-prepared-request-daemon.service)\" != "
        + shlex.quote(old_pid)
    )
    machine.wait_for_unit("blob-root-prepared-request-daemon.service")
    machine.succeed("test -f " + shlex.quote(request_path("inflight", CRASH)))
    machine.succeed("test -f " + shlex.quote(request_path("inflight", CRASH, "claim")))
    machine.succeed("test ! -e " + shlex.quote(request_path("ready", CRASH)))
    assert ledger_count() == 1
    assert_no_live_permit()
    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")

    # Recovery is deliberately fail-closed: restart/reboot never auto-retries an
    # ambiguous inflight request. An explicit retry is rejected as already claimed.
    status, output = user_call("alice", "Execute", CRASH)
    assert status != 0, (status, output)
    assert ledger_count() == 1
    assert_no_live_permit()

    # A denied caller cannot spend or strand a ready live-activation request.
    stage(TEST, "test", candidate)
    status, output = user_call("alice", "Execute", TEST)
    assert status != 0, (status, output)
    machine.succeed("test -f " + shlex.quote(request_path("ready", TEST)))
    machine.succeed("test ! -e " + shlex.quote(request_path("inflight", TEST)))
    assert ledger_count() == 1
    assert_no_live_permit()

    # Bob has the real polkit authority. The same staged request is claimed only
    # after his grant and activates the exact immutable candidate temporarily.
    status, output = user_call("bob", "Execute", TEST)
    assert status == 0, (status, output)
    assert "request-state=completed" in output, output
    machine.wait_until_succeeds("grep -qx CANDIDATE /etc/blob-activation-state")
    assert machine.succeed("readlink -f /run/current-system").strip() == candidate
    assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == baseline
    assert ledger_count() == 2
    assert_no_live_permit()
    machine.succeed("test -f " + shlex.quote(request_path("completed", TEST)))

    # Real reboot restores baseline. Request terminal/inflight state and the root
    # replay ledger survive; neither successful nor ambiguous requests auto-run.
    machine.reboot()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("blob-root-prepared-request-daemon.service")
    reboot_boot_id = machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()
    assert reboot_boot_id != baseline_boot_id, (baseline_boot_id, reboot_boot_id)
    machine.wait_until_succeeds("grep -qx BASELINE /etc/blob-activation-state")
    assert machine.succeed("readlink -f /run/current-system").strip() == baseline
    assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == baseline
    assert ledger_count() == 2
    assert_no_live_permit()
    machine.succeed("test -f " + shlex.quote(request_path("completed", PREVIEW)))
    machine.succeed("test -f " + shlex.quote(request_path("completed", TEST)))
    machine.succeed("test -f " + shlex.quote(request_path("inflight", CRASH)))

    status, output = user_call("bob", "Execute", TEST)
    assert status != 0, (status, output)
    status, output = user_call("alice", "Execute", CRASH)
    assert status != 0, (status, output)
    assert ledger_count() == 2
    assert_no_live_permit()
    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")
  '';
}
