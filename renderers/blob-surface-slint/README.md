# Blob Native Slint Surface Renderer

This directory is a **standalone renderer workspace**, intentionally excluded from the main Rust workspace.

## Why separate?

The trusted/semantic core of The Blob is currently validated at Rust 1.85. Slint 1.17.1 requires Rust 1.92, so the graphical toolkit must not raise the MSRV of the core simply because one renderer evolves faster.

```text
blob-core / Alfred / resolver / history
        Rust 1.85 baseline
                 |
          semantic Surface
                 |
       renderer adapter boundary
                 |
      blob-surface-slint
        Rust 1.92 + Slint 1.17.1
```

This separation is architectural, not temporary: future macOS Native, Android Native, Hyprland-specific or other renderers must be replaceable without redefining Workspace/Task/Capability semantics.

## What the first Surface shows

The demo runs the already-tested MVP vertical slice before opening the window and renders:

- the Alfred Situation;
- final Task state;
- selected Capability implementation and node;
- independent verifier evidence;
- structured execution result;
- the causal record sequence.

The UI therefore renders **semantic state produced by the system**, not a hand-authored fake screenshot.

## Run

Use Rust 1.92 or newer in this directory:

```text
cargo run
```

The demo uses a controlled local `sh` process as the MVP execution Capsule. This is explicitly not a security sandbox.

## Slint licensing

Slint is separately licensed by its authors under multiple licensing options. Before distributing binaries that include Slint, the project must select and comply with an appropriate Slint licensing path (including any attribution or copyleft requirements that apply). Nothing in this directory relicenses Slint.

The renderer remains isolated partly so that toolkit/licensing choices do not define the core architecture of The Blob.
