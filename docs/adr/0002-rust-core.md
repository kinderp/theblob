# ADR-0002: Rust-dominant trusted core

**Status:** provisional

## Decision
Use Rust for the trusted/core runtime: Personal World, Alfred, capability resolution, policy, Temporal/Causal Graph, Fabric, Workspace engine and system orchestration.

Capability implementations remain language-neutral and may use WASM/WASI, OCI, microVM, native or remote execution.

Python is appropriate for AI/ML experimentation and some non-trusted capability implementations. Kotlin may be used for Android platform glue. C/C++ are used where platform/kernel integration requires them.
