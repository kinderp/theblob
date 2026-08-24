# Wasmtime WebAssembly Component Capsule Runtime

Phase 2A runtime backend for The Blob.

This is a standalone Rust workspace so Wasmtime can evolve independently from the Rust 1.85 semantic/trusted core.

## Security posture of this prototype

The runtime uses an **empty Component Model linker**.

It does not install WASI, inherit stdio/environment, preopen files, expose sockets or register custom host functions.

Therefore:

```text
pure component / no imports -> may instantiate and execute
component requiring host import -> instantiation fails
```

This is only the first isolation proof. Phase 2B will add explicit WASI 0.2 grants and test that only authorized resources are visible.

## Current bootstrap interface

The first runtime expects one typed Component Model export:

```text
run: func() -> u32
```

This is not the final Capability ABI. Future versions will use WIT-generated interfaces mapped from The Blob Capability contracts.

## Run tests

Use Rust 1.92 or newer in this directory:

```text
cargo test
```

## Dependency boundary

Wasmtime is a runtime adapter, not an architectural authority. A runtime error cannot change policy, and Wasmtime output does not authorize execution. A verified Blob BindingLease is still the authority input to the runtime.
