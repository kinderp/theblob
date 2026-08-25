{ pkgs, lib, ... }:
let
  busName = "org.theblob.NixOsRoot";
  objectPath = "/org/theblob/NixOsRoot";
  interfaceName = "org.theblob.NixOsRoot1";
  previewAction = "org.theblob.nixos.preview-activation";
  testAction = "org.theblob.nixos.test-activation";

  rootFlowHarness = pkgs.stdenv.mkDerivation {
    pname = "blob-root-authorized-activation-vm";
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
        --example root_authorized_activation_vm
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/bin"
      cp target/release/examples/root_authorized_activation_vm \
        "$out/bin/blob-root-authorized-activation-vm"
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
        <description>Preview an exact approved NixOS activation</description>
        <message>Authentication is required to preview the approved NixOS activation</message>
        <defaults>
          <allow_any>no</allow_any>
          <allow_inactive>auth_admin</allow_inactive>
          <allow_active>auth_admin</allow_active>
        </defaults>
      </action>
      <action id="${testAction}">
        <description>Temporarily activate an exact approved NixOS system</description>
        <message>Authentication is required to temporarily activate the approved NixOS system</message>
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
    name = "blob-root-authorized-activation-service";
    executable = true;
    text = ''
      #!${python}/bin/python3
      import os
      import subprocess
      from gi.repository import Gio, GLib

      BUS_NAME = ${builtins.toJSON busName}
      OBJECT_PATH = ${builtins.toJSON objectPath}
      INTERFACE = ${builtins.toJSON interfaceName}
      HARNESS = ${builtins.toJSON "${rootFlowHarness}/bin/blob-root-authorized-activation-vm"}
      PKCHECK = ${builtins.toJSON "${pkgs.polkit}/bin/pkcheck"}
      CANDIDATE_LINK = "/run/current-system/specialisation/blob-candidate"

      INTROSPECTION = f"""
      <node>
        <interface name='{INTERFACE}'>
          <method name='Preview'>
            <arg type='s' name='observed_sender' direction='out'/>
            <arg type='s' name='evidence' direction='out'/>
          </method>
          <method name='Test'>
            <arg type='s' name='observed_sender' direction='out'/>
            <arg type='s' name='evidence' direction='out'/>
          </method>
        </interface>
      </node>
      """

      def run_authorized_activation(sender, action):
          candidate = os.path.realpath(CANDIDATE_LINK)
          if not candidate.startswith("/nix/store/"):
              raise RuntimeError("candidate did not resolve to an immutable store closure")
          result = subprocess.run(
              [
                  HARNESS,
                  "--sender", sender,
                  "--action", action,
                  "--candidate", candidate,
                  "--pkcheck", PKCHECK,
              ],
              stdin=subprocess.DEVNULL,
              stdout=subprocess.PIPE,
              stderr=subprocess.PIPE,
              text=True,
              check=False,
          )
          if result.returncode != 0:
              detail = result.stderr.strip().replace("\n", " | ")
              raise RuntimeError(detail[-1500:] or "authorized activation harness rejected")
          return result.stdout.strip()

      def on_method_call(connection, sender, object_path, interface_name, method_name, parameters, invocation):
          # The caller identity is accepted only from the system-bus message header.
          # Neither method has a sender, candidate, closure, program or argv input.
          if not sender or not sender.startswith(":"):
              invocation.return_dbus_error(
                  "org.theblob.Error.InvalidSender",
                  "The system bus did not provide a unique sender name",
              )
              return

          if method_name == "Preview":
              action = "preview"
          elif method_name == "Test":
              action = "test"
          else:
              invocation.return_dbus_error(
                  "org.theblob.Error.UnsupportedMethod",
                  "Unsupported root activation method",
              )
              return

          try:
              evidence = run_authorized_activation(sender, action)
          except Exception as error:
              invocation.return_dbus_error(
                  "org.theblob.Error.ActivationRejected",
                  str(error),
              )
              return

          invocation.return_value(GLib.Variant("(ss)", (sender, evidence)))

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
  name = "blob-root-authorized-activation";

  nodes.machine = { ... }: {
    environment.etc."blob-activation-state".text = "BASELINE\n";

    specialisation.blob-candidate = {
      inheritParentConfig = true;
      configuration = {
        system.nixos.tags = [ "blob-authorized-candidate" ];
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
    ];

    systemd.services.blob-root-authorized-activation = {
      description = "The Blob disposable authorized activation root service";
      wantedBy = [ "multi-user.target" ];
      after = [ "dbus.service" "polkit.service" "systemd-tmpfiles-setup.service" ];
      requires = [ "dbus.service" "polkit.service" ];
      # The privileged authority must not be restarted by the very temporary
      # activation transaction it is supervising. A controlled daemon upgrade
      # belongs to a separate handoff/reboot boundary, not this in-flight call.
      restartIfChanged = false;
      serviceConfig = {
        Type = "simple";
        User = "root";
        Group = "root";
        ExecStart = "${rootService}";
        Restart = "no";
        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectHome = true;
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
    PERMITS = "/var/lib/theblob/activation-permits"
    LEDGER = "/var/lib/theblob/privileged-executions"

    def user_call(user, method):
        inner = "busctl --system call " + DEST + " " + PATH + " " + IFACE + " " + method + " 2>&1"
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
    machine.wait_for_unit("blob-root-authorized-activation.service")

    baseline = machine.succeed("readlink -f /run/current-system").strip()
    candidate = machine.succeed(
        "readlink -f /run/current-system/specialisation/blob-candidate"
    ).strip()
    baseline_boot_id = machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()

    assert baseline.startswith("/nix/store/"), baseline
    assert baseline != candidate, (baseline, candidate)
    assert candidate.startswith("/nix/store/"), candidate

    # runNixOSTest boots directly from a store closure and does not install the
    # conventional /nix/var/nix/profiles/system profile. The privileged boundary
    # deliberately requires that installed-system invariant, so provision the VM's
    # boot-default profile exactly to the immutable closure it actually booted.
    machine.succeed(
        "nix-env --profile /nix/var/nix/profiles/system --set " + shlex.quote(baseline)
    )
    boot_default = machine.succeed(
        "readlink -f /nix/var/nix/profiles/system"
    ).strip()
    assert boot_default == baseline, (baseline, boot_default)

    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")
    machine.succeed("test -x " + candidate + "/bin/switch-to-configuration")
    assert ledger_count() == 0
    assert_no_live_permit()

    # The root D-Bus service itself is the only process that derives the live
    # system-bus sender and passes it to the Rust authority/permit/boundary flow.
    pid = machine.succeed("systemctl show -P MainPID blob-root-authorized-activation.service").strip()
    assert pid.isdigit() and int(pid) > 0, pid
    machine.succeed("test \"$(awk '/^Uid:/{print $2}' /proc/" + pid + "/status)\" = 0")
    machine.succeed("busctl --system status " + DEST + " >/dev/null")

    # alice is authorized for Preview. The exact candidate runs dry-activate,
    # the root-owned permit is destructively consumed, and the live system stays baseline.
    status, output = user_call("alice", "Preview")
    assert status == 0, (status, output)
    assert ":1." in output, output
    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")
    assert machine.succeed("readlink -f /run/current-system").strip() == baseline
    assert ledger_count() == 1
    assert_no_live_permit()

    # Reauthorizing the same exact prepared preview still cannot replay it:
    # the second durable root execution ledger rejects the reused authorization.
    status, output = user_call("alice", "Preview")
    assert status != 0, (status, output)
    assert "ActivationRejected" in output or "activation" in output.lower(), output
    assert ledger_count() == 1
    assert_no_live_permit()
    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")

    # alice is explicitly denied the live test action by real polkit. Denial occurs
    # before trusted permit issuance and before any switch-to-configuration execution.
    status, output = user_call("alice", "Test")
    assert status != 0, (status, output)
    assert ledger_count() == 1
    assert_no_live_permit()
    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")
    assert machine.succeed("readlink -f /run/current-system").strip() == baseline

    # bob is authorized for the temporary test action. The exact immutable closure
    # becomes the live system, but `test` deliberately does not alter how the VM boots.
    status, output = user_call("bob", "Test")
    assert status == 0, (status, output)
    machine.wait_until_succeeds("grep -qx CANDIDATE /etc/blob-activation-state")
    assert machine.succeed("readlink -f /run/current-system").strip() == candidate
    assert ledger_count() == 2
    assert_no_live_permit()

    # `test` is non-persistent. The authoritative proof in a NixOS VM is that a
    # real reboot returns to the original immutable closure and baseline marker.
    machine.reboot()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("blob-root-authorized-activation.service")
    reboot_boot_id = machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()
    assert reboot_boot_id != baseline_boot_id, (baseline_boot_id, reboot_boot_id)
    machine.wait_until_succeeds("grep -qx BASELINE /etc/blob-activation-state")
    assert machine.succeed("readlink -f /run/current-system").strip() == baseline
    assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == baseline
    assert ledger_count() == 2
    assert_no_live_permit()

    # The replay barrier is durable across reboot: bob may be authorized again by
    # polkit, but the same exact activation authorization can never execute twice.
    status, output = user_call("bob", "Test")
    assert status != 0, (status, output)
    assert ledger_count() == 2
    assert_no_live_permit()
    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")
  '';
}
