# Research and Engineering Roadmap

## Phase 0 — Vocabulary, archaeology and contracts — **BASELINE FROZEN**

Completed baseline:

- project named **The Blob** and repository established;
- `ARCHAEOLOGY.md` records the research lineage;
- Capability and Resolution contracts v0.1;
- Plan B Boxes and constraint-solver studies;
- System Technician / Improvement Watch / AI Broker accepted;
- Graphics/Experience model with Blob Native and platform-native Experience Profiles accepted;
- `ARCHITECTURE-FREEZE-v0.1.md` freezes semantic boundaries, not technologies;
- first concrete `Development Workspace v0.1` specified;
- Event/Situation Contract v0.1 specified;
- initial dependency-free Rust `blob-core` domain model created.

Phase 0 remains open only for corrections required by implementation evidence. New conceptual changes should use ADRs rather than silently redefining frozen terms.

## Phase 1 — Single-node vertical slice — **COMPLETE / CI VALIDATED**

The first vertical slice has been compiled/tested through GitHub CI and now demonstrates:

```text
source.modified
-> Alfred Situation
-> Task + RequirementGraph(test.run)
-> deterministic resolver
-> independent BindingVerifier
-> BindingLease
-> ephemeral execution result
-> Task transition
-> causal record chain
-> Blob Native Slint Surface
```

It also includes a read-only System Technician diagnostic slice for failed tests.

### 1A — Domain model + CI — done

- `blob-core` semantic model;
- architecture invariant tests;
- core MSRV Rust 1.85;
- trusted core independent from Slint/Z3/Nix/platform toolkits.

### 1B — Alfred deterministic MVP — done

- normalized Event model;
- event deduplication;
- versioned source-change rule;
- versioned failed-test rule;
- deterministic Situation provenance.

### 1C — Minimal deterministic resolver — done

- one-role `RequirementGraph` for `test.run`;
- finite implementation/node registry;
- deterministic filtering/ranking;
- independent `BindingVerifier`;
- `ResolutionTrace` and scoped `BindingLease`;
- unsupported Constraint IR explicitly rejected rather than ignored.

### 1D — First ephemeral execution backend — done

- controlled `LocalProcessCapsule` prototype;
- structured success/failure result;
- explicit statement that it is **not** a production sandbox.

### 1E — Temporal/Causal record — done

- append-only causal prototype;
- parent validation;
- end-to-end causal chain through execution.

### 1F — First Surface — done

- standalone Blob Native renderer using Slint 1.17.1;
- renderer uses Rust 1.92 independently from the Rust 1.85 core;
- Surface is driven by real vertical-slice semantic state;
- same semantic Surface model remains compatible with future macOS Native / Hyprland / Android renderers.

### 1G — System Technician read-only integration — done

- `development.test-failed` Situation;
- read-only `blob-technician` crate;
- evidence/binding explanation;
- Suggest-only `ImprovementProposal`;
- no executor/package-manager/SystemSpec dependency.

## Phase 2 — Capability Runtime — **ACTIVE**

Goal: replace the MVP process-backed execution proof with genuinely constrained, typed and disposable Capsule runtimes while preserving the Capability/Implementation/Binding separation.

### 2A — WebAssembly Component runtime — active

- use WebAssembly Component Model as the first typed portable Capsule runtime;
- deny-by-default host imports;
- no ambient filesystem/network/environment access;
- prove that an undeclared host import prevents instantiation;
- keep Wasmtime dependency outside the Rust 1.85 trusted semantic core;
- first runtime adapter uses a separately versioned/toolchained workspace.

### 2B — WASI explicit grants

After the deny-by-default component runtime is proven:

- add WASI 0.2/WASIp2 host integration;
- map Blob grants/Projections to explicit WASI resources;
- preopened-directory tests;
- stdio policy;
- network denied unless a future explicit grant exists;
- no ambient inheritance by default.

WASI 0.3/WASIp3 remains experimental/research until runtime/toolchain maturity justifies use in the trusted path.

### 2C — Capsule metadata / provenance

- content hash;
- publisher/signature metadata;
- Capability Contract reference;
- runtime/platform requirements;
- declared effects/permissions;
- build provenance / SBOM references;
- cache identity.

### 2D — Cache / materialization lifecycle

- available / cached / warm / running / pinned / evictable states;
- immutable implementation identity;
- destruction/eviction proof that user Task/Workspace state survives;
- compiled Wasmtime component cache where appropriate.

### 2E — OCI runtime adapter

- first OCI/container Capsule backend;
- same Capability contract and BindingPlan semantics;
- explicit filesystem/network/device grants;
- compare startup/cost/isolation with WASM backend.

### 2F — Adapter / converter graph

- typed adapter/converter edges;
- deterministic path search;
- quality/lossiness/trust/cost metadata;
- multi-step conversion paths.

### 2G — Multi-role RequirementGraph

- jointly resolve source/capability/output/node/adapter roles;
- introduce the first useful subset of Constraint IR;
- preserve independent verification;
- introduce Z3 only when ordinary deterministic Rust logic becomes insufficient.

### 2H — Dependency/version resolution

- evaluate PubGrub-style resolution for Capsule/Recipe versions and transitive dependencies;
- produce human-readable incompatibility derivations.

## Phase 3 — Workspace system

- Ready / AI Designed / Expert builders;
- Recipe versioning/forking/merging;
- UI components and Experience Grammar editor;
- Experience Profiles including Blob Native, macOS Native and Hyprland integration;
- benchmark/optimization profiles;
- initial Workspace Registry format.

## Phase 4 — Second node / Compute Fabric

Add one remote Linux node and prove:

- Fabric discovery;
- node trust/capability facts;
- placement;
- late binding/re-resolution;
- BindingLease migration boundaries;
- shared Task state across nodes.

Then add hosted macOS node support to prove that The Blob is multi-substrate rather than a Linux distribution.

## Phase 5 — Knowledge Objects

- content-addressed storage;
- typed Projections;
- reactive derived Representations;
- semantic metadata;
- Views;
- provenance/confidence;
- object history;
- compatibility/introspection projections.

## Phase 6 — Alfred Situations

Move from simple deterministic correlation to:

- multi-source temporal Situations;
- incremental correlation;
- AI-assisted semantic interpretation with provenance;
- persistent Context;
- policy-aware structured plans.

## Phase 7 — Adaptive system + System Technician

- declarative `SystemSpec`;
- Nix/NixOS backend and nix-darwin/hosted-node strategy;
- safe experiment branches;
- simulation/VM testing;
- benchmark/regression;
- System Technician `ImprovementProposal` lifecycle;
- Improvement Watch against trusted/official sources;
- causal recording and rollback;
- kernel/driver/build-profile experiments only behind explicit authority policy.

## Phase 8 — AI Broker

- resident local reasoning capability;
- stronger local/Fabric model routing;
- optional cloud providers;
- privacy-aware Projection building;
- quality/cost/latency/energy-aware model binding;
- AI failure must not impair deterministic system operation.

## Phase 9 — Mobile Surface

Android/GKI node + shared Personal World + phone Surface for one existing Workspace/Task. Later add watch-level status/action Surface.

## Phase 10 — Capability synthesis research

Experiment with AI-generated adapters/capabilities behind strict typed contracts, sandboxing, automated tests, policy checks and local trust scopes.

## Phase 11 — Advanced resolution and scheduling

- introduce Z3/SMT backend behind Constraint IR when multi-role problems justify it;
- differential experiments with cvc5;
- Datalog-style incremental derivation if profiling justifies it;
- CP-SAT for multi-task temporal scheduling/placement.

## Phase 12 — Custom graphical substrate

Evaluate replacing Hyprland with a Smithay compositor only once Surface/Workspace primitives are proven and compositor-level semantic control has measurable value.
