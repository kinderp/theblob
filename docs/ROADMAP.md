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

Phase 0 remains open only for corrections that are required by implementation evidence. New conceptual changes should use ADRs rather than silently redefining frozen terms.

## Phase 1 — Single-node vertical slice — **ACTIVE**

Goal: prove that Workspace state survives ephemeral implementation lifetimes and that event-driven work flows through deterministic resolution/verification rather than direct AI command execution.

### 1A — Domain model + CI

- compile/test `blob-core` at the declared MSRV;
- encode architecture invariants as Rust tests;
- keep core domain free from Z3/Nix/Slint/platform dependencies.

### 1B — Alfred deterministic MVP

- normalized Event Envelope;
- in-memory event stream;
- one versioned deterministic correlation rule;
- `source.modified -> development.source-change-requires-test` Situation;
- idempotency/deduplication for the narrow MVP path.

### 1C — Minimal deterministic resolver

- one-role `RequirementGraph` for `test.run`;
- finite in-memory Capability/Implementation/Node registry;
- deterministic candidate filtering/ranking;
- independent `BindingVerifier`;
- `ResolutionTrace` and short `BindingLease`;
- **no Z3 yet**: ordinary Rust logic first, preserving the future Constraint IR boundary.

### 1D — First ephemeral execution backend

- one non-privileged local `test.run` Capsule implementation;
- structured execution request/result;
- explicit effect envelope;
- materialize/start/finish/release lifecycle;
- Capsule destruction must not destroy Task/Workspace state.

The first backend may use a local process implementation for the controlled MVP fixture. It is not treated as the final security sandbox. WASM/OCI isolation belongs to Phase 2.

### 1E — Temporal/Causal record

Record the meaningful vertical-slice chain:

```text
source event
-> Situation
-> Task/RequirementGraph
-> resolution/verification
-> BindingLease
-> execution result
-> Task state transition
```

Store enough explanation to answer why an implementation/node was chosen.

### 1F — First Surface

- minimal Development Workspace Surface;
- Blob Native/Slint is the first target when core flow is stable;
- present source-change Situation, Task status, selected binding and test result;
- renderer remains outside `blob-core`.

### 1G — System Technician read-only integration

- consume a failed-test/build Situation;
- explain evidence and current binding;
- create a non-privileged diagnostic/proposal object;
- no autonomous system mutation in MVP-0.

## Phase 2 — Capability runtime

- richer typed contracts and effect declarations;
- registry metadata and signatures/provenance;
- sandbox policy;
- caching/eviction;
- WASM/WASI plus OCI backends;
- adapter/converter edges;
- joint multi-role Requirement Graph resolution and converter-path solving;
- PubGrub-like version/dependency resolution where justified.

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
