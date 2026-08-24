# NixOS materialization v0.1 validation checkpoint

This checkpoint is the first time the Linux Pilot crosses from The Blob's semantic model into a real NixOS module system.

CI must prove that:

1. the reference `SystemSpec` is rendered by the Rust backend;
2. the generated module matches the versioned `generated.nix` fixture byte-for-byte;
3. the Nix flake is pinned to the NixOS 26.05 stable branch;
4. real Nix/NixOS module evaluation accepts the generated configuration;
5. NixOS can derive both `system.build.toplevel` and `system.build.vm` for the candidate;
6. no host activation or mutation occurs during this checkpoint.

The fixture is intentionally split:

```text
base.nix      -> minimal reference-machine scaffolding not yet represented by SystemSpec
generated.nix -> deterministic output owned by SystemSpec -> NixOS translation
```

This split is temporary and visible. Configuration must not silently migrate into `base.nix` merely to bypass the semantic model.

A successful checkpoint does **not** yet mean The Blob has built or booted a complete Linux Pilot. The next step is an actual `build-vm` candidate build and boot smoke test, followed by controlled `nixos-rebuild build/test` operations on a dedicated test node.
