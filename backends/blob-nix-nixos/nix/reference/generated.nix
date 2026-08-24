{ pkgs, ... }:
{
  networking.hostName = "blob-pilot";
  nixpkgs.hostPlatform = "x86_64-linux";
  boot.kernelPackages = pkgs.linuxPackages_latest;
  hardware.bluetooth.enable = true;
  programs.hyprland.enable = true;
  services.pipewire.enable = true;
  services.printing.enable = false;
}
