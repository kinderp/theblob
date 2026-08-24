{ pkgs, ... }:
{
  # Validation-only module. It proves that a generated Blob candidate reaches
  # NixOS userspace; it is not part of the user's semantic SystemSpec.
  systemd.services.blob-vm-boot-smoke = {
    description = "The Blob NixOS VM boot smoke marker";
    wantedBy = [ "multi-user.target" ];
    serviceConfig.Type = "oneshot";
    script = ''
      echo BLOB_VM_BOOT_OK > /dev/ttyS0
      ${pkgs.systemd}/bin/systemctl poweroff --no-block
    '';
  };
}
