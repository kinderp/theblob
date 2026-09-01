# Blob Native Slint Surface Renderer

This directory is a **standalone renderer workspace**, intentionally excluded from the main Rust workspace.

## Why separate?

The trusted/semantic core of The Blob is currently validated at Rust 1.85. Slint 1.17.1 requires Rust 1.92, so the graphical toolkit must not raise the MSRV of the core simply because one renderer evolves faster.

```text
blob-core / Alfred / resolver / history / system workspace
        Rust 1.85 baseline
                 |
          semantic Surface
                 |
       renderer adapter boundary
                 |
      blob-surface-slint
        Rust 1.92 + Slint 1.17.1
```

This separation is architectural, not temporary: future macOS Native, Android Native, Hyprland-specific or other renderers must be replaceable without redefining Workspace/Task/Capability/SystemSpec semantics.

## Current alpha Surface

The GUI now contains the first Calm OS alpha composition:

- **Now**: a calm overview with the validated Development activity and the Personal Computer workspace;
- **Personal Computer / System Workspace**: reads the real semantic demo baseline from `blob-system-workspace`;
- **Bluetooth**: clicking the switch creates `SystemWorkspaceProposal::bluetooth_demo()` and exposes the semantic `disabled -> enabled` diff;
- **Technician**: evidence-backed, collapsible, and still unable to authorize execution;
- **Inspector**: exact resolver/verifier/causal evidence from the validated MVP vertical slice;
- **History/Fabric**: intentional placeholders until real projections are wired.

The Bluetooth switch deliberately does **not** flip the baseline state immediately. It produces a semantic proposal and leaves Bluetooth OFF until a later deterministic backend/materialization/verification/authority path succeeds. The renderer does not emit raw Nix and does not activate the live system.

The major visual blocks are kept as reusable Slint components so a later layout engine can add move/resize/dock persistence without changing the semantic Workspace model.

## Run

Use Rust 1.92 or newer.

From this directory:

```text
cargo run
```

Or from the repository root:

```text
cargo run --manifest-path renderers/blob-surface-slint/Cargo.toml
```

The startup Development activity uses a controlled local `sh` process as the MVP execution Capsule. This is explicitly not a security sandbox. The System Workspace portion is a semantic isolated demo and performs no live activation.

## Slint licensing

Slint is separately licensed by its authors under multiple licensing options. Before distributing binaries that include Slint, the project must select and comply with an appropriate Slint licensing path (including any attribution or copyleft requirements that apply). Nothing in this directory relicenses Slint.

The renderer remains isolated partly so that toolkit/licensing choices do not define the core architecture of The Blob.
