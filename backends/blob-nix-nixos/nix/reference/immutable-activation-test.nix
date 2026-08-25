{ pkgs, lib, ... }:
{
  name = "blob-immutable-activation";

  nodes.machine = { ... }: {
    # A deliberately tiny state difference lets the test prove exactly when
    # activation takes effect without relying on network access or extra tools.
    environment.etc."blob-activation-state".text = "BASELINE\n";

    specialisation.blob-candidate = {
      inheritParentConfig = true;
      configuration = {
        system.nixos.tags = [ "blob-candidate" ];
        environment.etc."blob-activation-state".text = lib.mkForce "CANDIDATE\n";
      };
    };

    virtualisation.memorySize = 1024;
    virtualisation.cores = 2;
  };

  testScript = ''
    # The NixOS test driver adds QEMU's -no-reboot unless this is explicit.
    # We need the same QEMU process to survive the guest reboot so that the
    # driver can reconnect and verify that `test` did not persist the candidate.
    machine.start(allow_reboot = True)
    machine.wait_for_unit("multi-user.target")

    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")
    baseline = machine.succeed("readlink -f /run/current-system").strip()
    baseline_boot_id = machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()
    candidate = machine.succeed(
        "readlink -f /run/current-system/specialisation/blob-candidate"
    ).strip()

    assert baseline != candidate, "candidate must be a distinct immutable system closure"
    assert baseline.startswith("/nix/store/"), baseline
    assert candidate.startswith("/nix/store/"), candidate
    assert baseline_boot_id != ""
    machine.succeed(f"test -x {candidate}/bin/switch-to-configuration")

    # Preview the exact reviewed closure. It may run explicitly dry-safe
    # activation snippets, but must not switch /run/current-system or our marker.
    machine.succeed(f"{candidate}/bin/switch-to-configuration dry-activate")
    machine.succeed("grep -qx BASELINE /etc/blob-activation-state")
    after_preview = machine.succeed("readlink -f /run/current-system").strip()
    assert after_preview == baseline, (baseline, after_preview)

    # Temporarily activate the exact immutable closure.
    machine.succeed(f"{candidate}/bin/switch-to-configuration test")
    machine.wait_until_succeeds("grep -qx CANDIDATE /etc/blob-activation-state")
    after_test = machine.succeed("readlink -f /run/current-system").strip()
    assert after_test == candidate, (candidate, after_test)

    # `test` must not make the candidate persistent. The test VM's original
    # boot closure remains authoritative; reboot must restore the baseline.
    machine.reboot()
    machine.wait_for_unit("multi-user.target")
    reboot_boot_id = machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()
    assert reboot_boot_id != ""
    assert reboot_boot_id != baseline_boot_id, (baseline_boot_id, reboot_boot_id)
    machine.wait_until_succeeds("grep -qx BASELINE /etc/blob-activation-state")
    after_reboot = machine.succeed("readlink -f /run/current-system").strip()
    assert after_reboot == baseline, (baseline, after_reboot)
  '';
}
