{ pkgs, ... }:
let
  busName = "org.theblob.NixOsRoot";
  objectPath = "/org/theblob/NixOsRoot";
  interfaceName = "org.theblob.NixOsRoot1";
  previewAction = "org.theblob.nixos.preview-activation";
  testAction = "org.theblob.nixos.test-activation";

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
    name = "blob-root-dbus-ipc-test";
    executable = true;
    text = ''
      #!${python}/bin/python3
      import os
      import subprocess
      from gi.repository import Gio, GLib

      BUS_NAME = ${builtins.toJSON busName}
      OBJECT_PATH = ${builtins.toJSON objectPath}
      INTERFACE = ${builtins.toJSON interfaceName}
      PREVIEW = ${builtins.toJSON previewAction}
      TEST = ${builtins.toJSON testAction}
      PKCHECK = ${builtins.toJSON "${pkgs.polkit}/bin/pkcheck"}

      INTROSPECTION = f"""
      <node>
        <interface name='{INTERFACE}'>
          <method name='PreviewAuthorized'>
            <arg type='b' name='authorized' direction='out'/>
            <arg type='s' name='observed_sender' direction='out'/>
          </method>
          <method name='TestAuthorized'>
            <arg type='b' name='authorized' direction='out'/>
            <arg type='s' name='observed_sender' direction='out'/>
          </method>
        </interface>
      </node>
      """

      def authorize(action, sender):
          result = subprocess.run(
              [
                  PKCHECK,
                  "--action-id", action,
                  "--system-bus-name", sender,
                  "--allow-user-interaction",
              ],
              stdin=subprocess.DEVNULL,
              stdout=subprocess.PIPE,
              stderr=subprocess.PIPE,
              env={"LANG": "C", "PATH": "/run/current-system/sw/bin"},
              check=False,
          )
          return result.returncode == 0

      def on_method_call(connection, sender, object_path, interface_name, method_name, parameters, invocation):
          # `sender` is supplied by the system bus for this live connection.
          # There is deliberately no caller-provided sender argument in either method.
          if not sender or not sender.startswith(":"):
              invocation.return_dbus_error(
                  "org.theblob.Error.InvalidSender",
                  "The system bus did not provide a unique sender name",
              )
              return

          if method_name == "PreviewAuthorized":
              allowed = authorize(PREVIEW, sender)
          elif method_name == "TestAuthorized":
              allowed = authorize(TEST, sender)
          else:
              invocation.return_dbus_error(
                  "org.theblob.Error.UnsupportedMethod",
                  "Unsupported root IPC method",
              )
              return

          invocation.return_value(GLib.Variant("(bs)", (allowed, sender)))

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
  name = "blob-root-dbus-ipc";

  nodes.machine = { ... }: {
    security.polkit.enable = true;
    security.polkit.extraConfig = ''
      polkit.addRule(function(action, subject) {
        if (subject.user == "alice" && action.id == "${previewAction}") {
          return "yes";
        }
        if (subject.user == "alice" && action.id == "${testAction}") {
          return "no";
        }
      });
    '';

    services.dbus.packages = [ blobDbusPolicy ];

    users.users.alice = {
      isNormalUser = true;
      uid = 1000;
    };

    environment.systemPackages = [
      blobPolkitActions
      pkgs.dbus
      pkgs.polkit
    ];

    systemd.services.blob-root-dbus-ipc = {
      description = "The Blob disposable root D-Bus IPC authorization service";
      wantedBy = [ "multi-user.target" ];
      after = [ "dbus.service" "polkit.service" ];
      requires = [ "dbus.service" "polkit.service" ];
      serviceConfig = {
        Type = "simple";
        User = "root";
        Group = "root";
        ExecStart = "${rootService}";
        Restart = "no";
        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectHome = true;
        ProtectSystem = "strict";
      };
    };

    virtualisation.memorySize = 1024;
    virtualisation.cores = 2;
  };

  testScript = ''
    import shlex

    DEST = "${busName}"
    PATH = "${objectPath}"
    IFACE = "${interfaceName}"

    def alice_call(method, extra=""):
        inner = "busctl --system call " + DEST + " " + PATH + " " + IFACE + " " + method
        if extra:
            inner += " " + extra
        return machine.execute("su -s /bin/sh alice -c " + shlex.quote(inner))

    def decode_bool_sender(output):
        fields = shlex.split(output.strip())
        assert len(fields) == 3, fields
        assert fields[0] == "bs", fields
        assert fields[1] in ("true", "false"), fields
        assert fields[2].startswith(":"), fields
        return fields[1] == "true", fields[2]

    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("polkit.service")
    machine.wait_for_unit("blob-root-dbus-ipc.service")

    # The service really runs as root and owns the expected system-bus name.
    pid = machine.succeed("systemctl show -P MainPID blob-root-dbus-ipc.service").strip()
    assert pid.isdigit() and int(pid) > 0, pid
    machine.succeed("test \"$(awk '/^Uid:/{print $2}' /proc/" + pid + "/status)\" = 0")
    machine.succeed("busctl --system status " + DEST + " >/dev/null")

    # The D-Bus contract takes no sender argument. A spoof attempt with an extra
    # string must fail at signature validation before our handler can authorize it.
    status, output = alice_call("PreviewAuthorized", "s :1.999")
    assert status != 0, (status, output)

    # Real non-root caller, real D-Bus-assigned unique sender, real polkit YES.
    status, output = alice_call("PreviewAuthorized")
    assert status == 0, (status, output)
    preview_allowed, preview_sender = decode_bool_sender(output)
    assert preview_allowed, (preview_allowed, preview_sender)

    # Same root service and policy substrate, but the test action is explicitly NO.
    status, output = alice_call("TestAuthorized")
    assert status == 0, (status, output)
    test_allowed, test_sender = decode_bool_sender(output)
    assert not test_allowed, (test_allowed, test_sender)

    # Each busctl invocation is a new live connection. The service reports the
    # actual sender from the message header, proving it is not a reusable client token.
    assert preview_sender != test_sender, (preview_sender, test_sender)

    # Unknown methods fail closed at the D-Bus boundary.
    status, output = alice_call("Switch")
    assert status != 0, (status, output)
  '';
}
