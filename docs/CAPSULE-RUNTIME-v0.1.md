# Capsule Runtime Contract v0.1

**Status:** Phase 2A implementation contract.

## 1. Purpose

A Capsule runtime materializes and executes one concrete Capability Implementation under the authority already represented by a verified BindingLease.

The runtime does not decide **which** implementation should run and does not grant itself permissions.

```text
RequirementGraph
      |
resolver + verifier
      |
BindingLease
      |
Capsule runtime
      |
structured ExecutionResult
```

## 2. Runtime invariants

- `Capability != CapabilityImplementation != Capsule instance`;
- execution starts only from an already-selected implementation/lease;
- no runtime receives ambient user state merely because it exists on the host;
- host resources are deny-by-default and explicitly linked/granted;
- Capsule process/instance lifetime is independent from Task/Workspace lifetime;
- execution produces structured result/evidence;
- runtime-specific errors do not silently mutate Task or policy state;
- implementation/runtime metadata is observable for causal history.

## 3. First runtime: WebAssembly Component Model

Phase 2A uses Wasmtime as a replaceable runtime backend for WebAssembly Components.

The first proof intentionally provides **no WASI and no host imports**.

```text
Component Capsule
      |
Wasmtime Engine
      |
empty Component Linker
      |
instantiate
      |
call typed export
```

A pure component with no imports can execute.

A component requiring an undeclared host import must fail instantiation.

This proves deny-by-default resource linking before filesystem/network APIs are introduced.

## 4. Why Component Model rather than only core Wasm

The target Capability architecture requires typed interfaces independent from one implementation language. The Component Model maps naturally to the future WIT-backed Capability ABI while preserving our own Capability Contract as the architectural source of truth.

The Blob does not equate its Capability model with one particular WASM/WASI version. Wasmtime is an adapter/backend.

## 5. Phase 2A execution shape

The first test interface is deliberately tiny:

```text
export run: func() -> u32
```

The runtime records:

- Task ID;
- BindingLease ID;
- Implementation ID;
- export name;
- returned status code;
- execution duration;
- runtime/backend identity.

This is a bootstrap interface only. Phase 2B+ will move toward WIT-generated typed Capability interfaces.

## 6. Host import policy

Phase 2A:

```text
filesystem   DENY
network      DENY
environment  DENY
stdio        DENY
clock        DENY unless pure Wasm runtime inherently needs host engine timing only
random       DENY
custom host  DENY
```

The component Linker is empty.

Phase 2B will add WASI 0.2 resources explicitly from Blob grants/Projections. The design must not use ambient `inherit_*` shortcuts as the normal production default.

## 7. WASI roadmap

### WASI 0.2 / WASIp2

First supported WASI host integration because it is the mature Component Model-oriented WASI line in current Wasmtime.

Planned mappings:

```text
Blob filesystem Projection/grant -> selected preopened directory/resource
Blob stdio policy               -> captured/explicit streams
Blob environment policy         -> selected key/value entries
Blob network policy             -> denied initially, explicit later
```

### WASI 0.3 / WASIp3

Tracked as research for native async/streams/futures, but not a Phase 2 trusted-path requirement while runtime support remains experimental/incomplete.

## 8. Toolchain isolation

Wasmtime evolves faster than the semantic core. The runtime therefore lives in a standalone Cargo workspace:

```text
Rust 1.85 semantic/trusted core
          |
Capability/Binding boundary
          |
standalone Wasmtime runtime workspace
```

The Wasmtime crate/toolchain may be upgraded without changing frozen semantic types unless the runtime contract itself changes.

## 9. Caching

Wasmtime Components can be compiled before instantiation and compiled artifacts can later be cached/serialized. Phase 2A does not yet build the Blob cache manager, but runtime identity must not prevent Phase 2D from caching compiled immutable components by content/build identity.

## 10. Non-goals of Phase 2A

- arbitrary guest filesystem access;
- networking;
- OCI;
- dynamic host service injection;
- full WIT Capability SDK;
- cryptographic Capsule signing;
- performance tuning;
- replacing `LocalProcessCapsule` everywhere immediately.

The goal is only to prove a typed portable runtime boundary with **zero ambient host capabilities by default**.
