{
  description = "The Blob Linux Pilot NixOS reference evaluation";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs = { self, nixpkgs }:
    {
      nixosConfigurations.blob-pilot = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          ./base.nix
          ./generated.nix
        ];
      };
    };
}
