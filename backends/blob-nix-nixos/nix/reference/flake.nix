{
  description = "The Blob Linux Pilot NixOS reference evaluation";

  # Snapshot validated from the NixOS 26.05 stable branch on 2026-08-24.
  # Pinning the exact revision keeps the Pilot reference reproducible.
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/a9e6d84f9c2f9012f5fe7d964a7851352300e61a";

  outputs = { self, nixpkgs }:
    let
      mkBlobPilot = extraModules: nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          ./base.nix
          ./generated.nix
        ] ++ extraModules;
      };
    in
    {
      nixosConfigurations.blob-pilot = mkBlobPilot [ ];
      nixosConfigurations.blob-pilot-smoke = mkBlobPilot [ ./smoke.nix ];
    };
}
