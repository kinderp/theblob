# NixOS VM build v0.1 validation checkpoint

This checkpoint advances the Linux Pilot from **derivation evaluation** to a real Nix build.

The branch pins the exact Nixpkgs revision already validated from the NixOS 26.05 stable line. CI must then:

1. keep the normal core/Slint/WASIp2/NixOS evaluation suite green;
2. build `nixosConfigurations.blob-pilot.config.system.build.vm` through real Nix;
3. obtain a concrete immutable `/nix/store/...` VM runner output;
4. verify that the output exposes a runnable `bin` payload;
5. perform no activation or mutation of the GitHub runner host configuration.

This proves that the `SystemSpec -> generated NixOS module -> NixOS module system -> VM derivation` chain is not only evaluable but materializable.

The next checkpoint after a successful build is a bounded QEMU/KVM boot smoke test of the generated candidate. Only after that do we move toward controlled `nixos-rebuild build/test/build-vm` on a dedicated Linux test node.
