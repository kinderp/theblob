# ADR-0020: WASI grants are explicit resource bindings

**Status:** Accepted

## Context

The Phase 2A WebAssembly Component runtime deliberately uses an empty linker. Pure components execute, while any component that imports host functionality fails to instantiate.

Phase 2B must add useful host access without reintroducing ambient authority. In particular, filesystem access must be derived from an already-verified Blob `BindingLease`/Projection rather than inherited wholesale from the host process.

## Decision

The first WASI integration uses WASI 0.2 / WASIp2 through a dedicated grant-aware runtime adapter.

A Blob filesystem grant is materialized as an explicit WASI preopened directory with explicit guest-visible path and permissions.

The runtime must not use `inherit_*` helpers as its normal production path for filesystem, environment, arguments, stdio or networking.

Network access remains denied in Phase 2B.

The runtime records the concrete grant set used for execution as evidence, but the runtime itself is not an authorization authority: it only materializes grants already represented by a valid lease.

## Consequences

- a component with no filesystem grant sees no host filesystem;
- a component granted one directory cannot traverse above that preopen;
- read-only and read/write grants are distinct;
- undeclared host imports still fail;
- later Blob Projections can map to narrower WASI resources without changing the Capability/Binding model;
- Wasmtime/WASI remain replaceable runtime backends outside the trusted semantic core.

## Non-goals

- ambient home-directory access;
- inherited environment/argv/stdio by default;
- network grants in Phase 2B;
- treating WASI capability names as the native Blob permission model;
- moving authorization into Wasmtime.
