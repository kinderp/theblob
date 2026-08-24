{ ... }:
{
  # Reference-machine scaffolding only. These values describe the disposable
  # validation VM/boot target and are intentionally separate from the user
  # SemanticBuildProfile produced by SystemSpec.
  fileSystems."/" = {
    device = "/dev/vda";
    fsType = "ext4";
  };

  boot.loader.grub.devices = [ "/dev/vda" ];

  system.stateVersion = "26.05";
}
