{ pkgs, ... }:
let
  previewAction = "org.theblob.nixos.preview-activation";
  testAction = "org.theblob.nixos.test-activation";

  blobPolkitActions = pkgs.writeTextDir "/share/polkit-1/actions/org.theblob.nixos.policy" ''
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
in
{
  name = "blob-polkit-authority";

  nodes.machine = { ... }: {
    security.polkit.enable = true;

    # This rule is deliberately test-only. It gives us deterministic YES and NO
    # paths without a graphical authentication agent while still exercising the
    # real polkit daemon and a real non-root system-bus subject.
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

    users.users.alice = {
      isNormalUser = true;
      uid = 1000;
    };

    environment.systemPackages = [
      blobPolkitActions
      pkgs.dbus
    ];

    systemd.tmpfiles.rules = [
      "d /var/lib/theblob 0700 root root -"
      "d /var/lib/theblob/activation-permits 0700 root root -"
      "d /var/lib/theblob/privileged-executions 0700 root root -"
    ];

    # No well-known name is requested. D-Bus assigns this connection a unique
    # :1.x name; the test discovers that exact live identity from the bus.
    systemd.services.blob-polkit-test-client = {
      description = "The Blob disposable non-root system-bus authorization subject";
      wantedBy = [ "multi-user.target" ];
      after = [ "dbus.service" ];
      requires = [ "dbus.service" ];
      serviceConfig = {
        Type = "simple";
        User = "alice";
        ExecStart = "${pkgs.dbus}/bin/dbus-test-tool black-hole --system";
        Restart = "no";
      };
    };

    virtualisation.memorySize = 1024;
    virtualisation.cores = 2;
  };

  testScript = ''
    import shlex

    PREVIEW = "${previewAction}"
    TEST = "${testAction}"

    def client_unique_name():
        pid = machine.succeed(
            "systemctl show -P MainPID blob-polkit-test-client.service"
        ).strip()
        assert pid.isdigit() and int(pid) > 0, pid

        listing = machine.succeed("busctl --system --no-pager --no-legend list")
        matches = []
        for line in listing.splitlines():
            fields = line.split()
            if len(fields) >= 2 and fields[0].startswith(":") and fields[1] == pid:
                matches.append(fields[0])
        assert len(matches) == 1, (pid, matches, listing)
        return matches[0]

    def pkcheck(action, unique):
        return machine.execute(
            "pkcheck --action-id "
            + shlex.quote(action)
            + " --system-bus-name "
            + shlex.quote(unique)
            + " --allow-user-interaction"
        )

    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("polkit.service")
    machine.wait_for_unit("blob-polkit-test-client.service")

    # Prove the action metadata really entered the NixOS polkit search path.
    machine.succeed(
        "test -f /run/current-system/sw/share/polkit-1/actions/org.theblob.nixos.policy"
    )
    machine.succeed(
        "grep -q 'org.theblob.nixos.preview-activation' "
        "/run/current-system/sw/share/polkit-1/actions/org.theblob.nixos.policy"
    )
    machine.succeed(
        "grep -q 'org.theblob.nixos.test-activation' "
        "/run/current-system/sw/share/polkit-1/actions/org.theblob.nixos.policy"
    )

    # Root-side capability/replay directories are provisioned fail-closed.
    machine.succeed("test \"$(stat -c %U:%G /var/lib/theblob/activation-permits)\" = root:root")
    machine.succeed("test \"$(stat -c %a /var/lib/theblob/activation-permits)\" = 700")
    machine.succeed("test \"$(stat -c %U:%G /var/lib/theblob/privileged-executions)\" = root:root")
    machine.succeed("test \"$(stat -c %a /var/lib/theblob/privileged-executions)\" = 700")

    first_unique = client_unique_name()
    assert first_unique.startswith(":"), first_unique

    # Same live non-root system-bus subject, two distinct actions: test-only
    # policy explicitly grants preview and explicitly denies live test activation.
    status, output = pkcheck(PREVIEW, first_unique)
    assert status == 0, (status, output, first_unique)

    status, output = pkcheck(TEST, first_unique)
    assert status != 0, (status, output, first_unique)

    # A D-Bus unique name is valid only while its connection is alive. Once the
    # sender disappears, the old subject must not remain usable for authorization.
    machine.succeed("systemctl stop blob-polkit-test-client.service")
    machine.wait_until_succeeds(
        "test \"$(systemctl show -P ActiveState blob-polkit-test-client.service)\" = inactive"
    )
    status, output = pkcheck(PREVIEW, first_unique)
    assert status != 0, (status, output, first_unique)

    # Reconnecting creates a new unspoofable unique name and authorization works
    # only for that currently-live identity.
    machine.succeed("systemctl start blob-polkit-test-client.service")
    machine.wait_for_unit("blob-polkit-test-client.service")
    second_unique = client_unique_name()
    assert second_unique.startswith(":"), second_unique
    assert second_unique != first_unique, (first_unique, second_unique)

    status, output = pkcheck(PREVIEW, second_unique)
    assert status == 0, (status, output, second_unique)
    status, output = pkcheck(TEST, second_unique)
    assert status != 0, (status, output, second_unique)
  '';
}
