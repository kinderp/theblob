{
  description = "The Blob Linux Pilot NixOS reference evaluation";

  # Snapshot validated from the NixOS 26.05 stable branch on 2026-08-24.
  # Pinning the exact revision keeps the Pilot reference reproducible.
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/a9e6d84f9c2f9012f5fe7d964a7851352300e61a";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      mkBlobPilot = extraModules: nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          ./base.nix
          ./generated.nix
        ] ++ extraModules;
      };

      # Hermetic fixture for the product-level Bluetooth composition test. This
      # is evaluated and materialized by the outer Nix build, where the pinned
      # nixpkgs cache is available, then copied into the deliberately offline
      # nested VM through system.extraDependencies. Runtime code does not get
      # this path as authority input; it must independently derive the same
      # SystemSpec translation and exact Nix output identity.
      bluetoothDemoGenerated = pkgs.writeText "blob-system-workspace-demo-generated.nix" ''
        { pkgs, ... }:
        {
          networking.hostName = "blob-demo";
          nixpkgs.hostPlatform = "x86_64-linux";
          hardware.bluetooth.enable = true;
          services.pipewire.enable = true;
          services.printing.enable = false;
        }
      '';
      bluetoothDemoSystem = (nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          ./base.nix
          bluetoothDemoGenerated
        ];
      }).config.system.build.toplevel;
      bluetoothDemoTest = import ./system-workspace-bluetooth-demo-test.nix {
        inherit pkgs bluetoothDemoGenerated bluetoothDemoSystem;
        lib = nixpkgs.lib;
      };
    in
    {
      nixosConfigurations.blob-pilot = mkBlobPilot [ ];
      nixosConfigurations.blob-pilot-smoke = mkBlobPilot [ ./smoke.nix ];

      checks.${system} = {
        immutable-activation = pkgs.testers.runNixOSTest {
          imports = [ ./immutable-activation-test.nix ];
        };
        polkit-authority = pkgs.testers.runNixOSTest {
          imports = [ ./polkit-authority-test.nix ];
        };
        root-dbus-ipc = pkgs.testers.runNixOSTest {
          imports = [ ./root-dbus-ipc-test.nix ];
        };
        root-authorized-activation = pkgs.testers.runNixOSTest {
          imports = [ ./root-authorized-activation-test.nix ];
        };
        root-prepared-request-daemon = pkgs.testers.runNixOSTest {
          imports = [ ./root-prepared-request-daemon-test.nix ];
        };
        root-request-publisher = pkgs.testers.runNixOSTest {
          imports = [
            ./root-request-publisher-test.nix
            ./root-request-publisher-writable-store.nix
          ];
        };
        materialization-admission = pkgs.testers.runNixOSTest {
          imports = [ ./materialization-admission-test.nix ];
        };
        materialization-to-request = pkgs.testers.runNixOSTest {
          imports = [ ./materialization-to-request-test.nix ];
        };
        materialization-begin-boundary = pkgs.testers.runNixOSTest {
          imports = [ ./materialization-begin-boundary-test.nix ];
        };
        systemspec-candidate-producer = pkgs.testers.runNixOSTest {
          imports = [ ./systemspec-candidate-producer-test.nix ];
        };
        async-materialization-begin = pkgs.testers.runNixOSTest {
          imports = [ ./async-materialization-begin-test.nix ];
        };
        materialization-lifecycle = pkgs.testers.runNixOSTest {
          imports = [ ./materialization-lifecycle-test.nix ];
        };
        candidate-source-quiescence = pkgs.testers.runNixOSTest {
          imports = [ ./candidate-source-quiescence-test.nix ];
        };
        system-workspace-bluetooth-demo = pkgs.testers.runNixOSTest {
          imports = [ bluetoothDemoTest ];
        };
      };
    };
}
