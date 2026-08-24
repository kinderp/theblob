# Research and Engineering Roadmap

## Product pilot overlay — **ACCEPTED**

The engineering phases below are internal sequencing, not the order in which the project must feel complete to a user.

The first user-visible product objective is **Pilot A: Linux — Arch/Gentoo power with mainstream-desktop simplicity**. Only after that experience becomes independently useful do we expand the same Personal World incrementally across a second Linux/Ubuntu node, macOS, Windows, Android and a Garmin wearable companion.

See:

- [`PILOT-ROADMAP.md`](PILOT-ROADMAP.md)
- [`BLOB-LINUX-PILOT-v0.1.md`](BLOB-LINUX-PILOT-v0.1.md)
- [`FAILURE-LESSONS.md`](FAILURE-LESSONS.md)
- [`adr/0018-linux-first-mainstream-compatible-pilot.md`](adr/0018-linux-first-mainstream-compatible-pilot.md)

The rule is: **do not build five incomplete platform clients at once**. Each new platform must add one measurable capability while preserving the already-working pilot.

## Linux Pilot implementation track — **ACTIVE**

This track intentionally pulls forward a narrow subset of later architecture phases when needed to make the first Linux product real.

Completed checkpoints:

### LP-0 — semantic SystemSpec — done / CI validated

- backend-neutral `SystemSpec v0.1` in `blob-core`;
- Ready / AI Designed / Expert construction mode represented in the same typed model;
- Semantic Build Profile, priorities and feature selections;
- deterministic validation;
- ADR-0021 freezes `SystemSpec != Nix`.

### LP-1 — minimal NixOS backend — done / CI validated

- deterministic `SystemSpec -> NixOS module` translation;
- semantic-to-Nix translation trace;
- unsupported feature/channel requests fail explicitly;
- no raw LLM-generated Nix as canonical managed state.

### LP-2 — real NixOS module evaluation — done / CI validated

- Rust-generated module matches the versioned reference byte-for-byte;
- exact validated Nixpkgs snapshot from the NixOS 26.05 stable line is pinned;
- real NixOS module system accepts the candidate;
- both `system.build.toplevel` and `system.build.vm` derivations evaluate.

### LP-3 — real immutable VM build — done / CI validated

- `system.build.vm` is materially built through Nix;
- immutable Nix-store VM runner is produced;
- no CI host activation occurs.

### LP-4 — real VM userspace boot — done / CI validated

- generated candidate boots under QEMU/KVM;
- Linux kernel reaches systemd userspace and `multi-user.target`;
- validation service emits `BLOB_VM_BOOT_OK` over the serial console;
- VM powers off cleanly;
- normal core, Slint, WASIp2 and NixOS-evaluation gates remain green.

### LP-5 — bounded System Candidate Operations — active

- semantic actions: `Materialize`, `BuildIsolatedVm`, `PreviewActivation`, `TestActivation`;
- canonical effect/authority classification;
- NixOS backend emits structured `program + argv`, never an AI shell string;
- forged action/effect/authority combinations are rejected;
- persistent `switch`/`boot` activation is absent from v0.1 by construction.

Next Linux Pilot checkpoints:

1. CI-validate LP-5;
2. implement a non-privileged executor for `Materialize` and `BuildIsolatedVm`;
3. return structured `SystemOperationResult` with derivation/output/evidence;
4. connect operation results to Temporal/Causal history;
5. expose candidate diff/explanation to the System Technician;
6. define a dedicated NixOS physical test-node enrollment/profile;
7. only then implement privileged `PreviewActivation`/`TestActivation` execution;
8. persistent `boot`/`switch` requires a separate authority/rollback ADR.

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
- no generic privileged executor dependency.

## Phase 2 — Capability Runtime — **ACTIVE: 2C NEXT**

Goal: replace the MVP process-backed execution proof with genuinely constrained, typed and disposable Capsule runtimes while preserving the Capability/Implementation/Binding separation.

### 2A — WebAssembly Component runtime — done / CI validated

- WebAssembly Component Model first typed portable Capsule runtime;
- deny-by-default host imports;
- no ambient filesystem/network/environment access;
- undeclared host import prevents instantiation;
- Wasmtime dependency remains outside the Rust 1.85 trusted semantic core;
- separate runtime workspace/toolchain.

### 2B — WASI explicit grants — done / CI validated

- WASI 0.2/WASIp2 host integration;
- explicit preopened filesystem grants;
- no filesystem grant -> no workspace access;
- read-only grant reads but cannot write;
- read-write grant writes only inside the preopen;
- parent traversal cannot escape the granted directory;
- TCP/UDP/name lookup denied;
- no ambient environment/argv/stdio inheritance;
- guest `proc_exit` is captured as structured guest exit status, distinct from runtime failure.

WASI 0.3/WASIp3 remains experimental/research until runtime/toolchain maturity justifies use in the trusted path.

### 2C — Capsule metadata / provenance — next

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

## Phase 7 — Adaptive system + System Technician — **PARTIALLY PULLED INTO LINUX PILOT**

Already proven in Linux Pilot track:

- declarative `SystemSpec v0.1`;
- minimal Nix/NixOS backend;
- isolated candidate evaluation/build/VM boot;
- initial bounded candidate operation semantics.

Still required for the full phase:

- generalized Nix/NixOS backend and nix-darwin/hosted-node strategy;
- safe experiment branches integrated with Personal World;
- benchmark/regression framework;
- full System Technician `ImprovementProposal` lifecycle;
- Improvement Watch against trusted/official sources;
- causal recording and rollback for system operations;
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
