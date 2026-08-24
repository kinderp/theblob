# Systems Archaeology — Research Lineage

**Status:** living research note  
**Architecture target:** v0.4

The Blob does not assume that distributed computing, semantic storage, dynamic modules, adaptive interfaces, capability security or pervasive computing are new ideas. Its research question is narrower and more useful:

> What does the research line from UNIX through Plan 9, Inferno, Plan B and pervasive personal computing become when its primitives also include AI reasoning, semantic memory, agents, heterogeneous personal devices, cloud/GPU resources, safe software synthesis and reversible system evolution?

## Lineage

```text
MULTICS
   |
   | persistent/shared computing environment
   v
UNIX
   |
   | small mechanisms, pipes, composition, uniform I/O
   v
PLAN 9
   |
   | private namespaces, 9P, distributed resources
   | Plumber, Acme, archival history, Factotum
   v
INFERNO
   |
   | portable VM, typed dynamically loaded modules
   | local/remote resource transparency
   v
PLAN B
   |
   | Boxes, constraints, dynamic resources
   | converter paths, late/contextual binding
   v
OMERO / OCTOPUS
   |
   | distributed UI, per-user pervasive computer
   | one logical environment across heterogeneous devices
   v
RELATED LINES
   | Semantic File Systems, Exokernel, NIX,
   | EROS/KeyKOS, Singularity, content-addressed storage
   v
THE BLOB
   |
   + Intent and persistent Goals
   + Semantic and causal memory
   + AI interpretation with deterministic authorization/resolution
   + Typed ephemeral Capability Fabric
   + AI-designed Workspaces and adaptive Surfaces
   + Safe, simulated and versioned system self-evolution
```

This is a genealogy of ideas, not a claim that each system directly descended from the previous one.

## UNIX — composition before intelligence

The most important inheritance is not a particular syscall. It is the discipline of building small mechanisms that compose.

```text
UNIX:
producer | transformer | transformer | consumer

The Blob:
Typed Object -> Capability -> typed output -> Capability -> Result
```

The Blob generalizes byte/text pipelines into typed semantic graphs selected under explicit policy and constraints.

## Plan 9 — one environment out of many machines

Plan 9 attacked a problem close to The Blob's starting point: computing had fragmented into independent workstations, each becoming its own resource and administrative island.

Its private namespaces and 9P allowed processes to compose local and remote resources into one useful environment. Different machines could specialize as terminals, CPU servers, storage servers or network providers.

The Blob generalizes that idea:

```text
Plan 9 private namespace
  resources + names
          |
          v
Workspace
  resources
  capabilities
  knowledge
  context
  tasks
  policy
  surfaces
```

Likewise, the Compute Fabric generalizes location transparency from files/devices to GPUs, phone cameras, watch sensors, servers, cloud models and other capabilities.

## Plumber — ancestor of Alfred

Plan 9's Plumber routed messages according to content, context and rules.

```text
Plumber:
message -> content/context -> rules -> dispatch

Alfred:
raw events
 -> normalization
 -> temporal correlation
 -> candidate Situation
 -> AI semantic interpretation
 -> structured Situation
 -> policy/authorization
 -> RequirementGraph
 -> verified action
```

Alfred adds time, multi-device context, goals, semantic memory and autonomous planning while preserving a deterministic authorization boundary.

## Acme — UI as inspectable system state

Acme blurred editor, shell and window system, and exposed important UI state to external tools rather than hiding everything inside opaque application internals.

The Blob inherits the principle, not the exact filesystem API: a Surface should be a typed semantic model that agents can inspect and manipulate without relying on pixel clicking.

```text
Workspace
   -> Surface Model
   -> Panel / Document / Selection / Action / Timeline / Editor
   -> renderer
```

## Plan 9 archival history and Venti

Plan 9 made historical filesystem states easy to navigate. Venti later used content addressing for immutable archival storage and deduplication.

The Blob separates two related concerns:

- **Temporal storage** reconstructs what existed.
- **Causal history** explains why a meaningful change happened.

A causal record can include WHAT, WHY, WHO/agent, TRIGGER, EVIDENCE, expected effect, actual effect, side effects, authorization and rollback references.

## Factotum — delegated identity

Factotum centralized authentication/key use instead of teaching every application to handle credentials independently.

The Blob extends this into capability-style delegation. A Capsule should receive a narrow grant such as:

```text
mail.send
recipient = X
expires = 10m
max_messages = 1
```

rather than a reusable mailbox password.

## Inferno — typed ephemeral modules

Inferno and Limbo demonstrated dynamically loaded modules with typed interfaces and multiple implementations, plus portability across heterogeneous systems.

This is a direct ancestor of the Capability/Implementation split:

```text
Capability contract
   |
   +-- WASM implementation
   +-- OCI implementation
   +-- native implementation
   +-- AI model
   +-- remote service
   +-- hardware capability
```

The Blob extends dynamic modules with policy, trust, cost, quality, energy, placement, ephemeral lifetimes and Fabric-wide resolution.

## Plan B — constraints, converters and dynamic resources

Plan B is one of the closest research relatives of The Blob. It explored heterogeneous resources that appear, disappear and move, typed Boxes, constraint-based resource selection, conversion paths and late binding.

The deep study is maintained in [`research/PLAN-B-BOXES.md`](research/PLAN-B-BOXES.md).

The most important lesson is **joint resolution**. Resources participating in one operation should not be selected independently when their compatibility depends on each other.

That directly motivates The Blob's `RequirementGraph`, `BindingPlan` and `BindingLease`.

Plan B's converter chains are an ancestor of the Capability/Adapter graph. Its type-dependent `select` operation motivated first-class semantic `Projection`s. Its later return to simple virtual file interfaces motivates The Blob's Compatibility/Introspection Plane: rich native semantics inside, simple universal projections outside.

## Omero and Octopus — distributed UI and the personal pervasive computer

Omero explored splitting, relocating and replicating pieces of UI across available devices rather than treating UI as permanently owned by one application on one display.

Octopus pursued a per-user pervasive environment: a logical computer composed from resources spread across devices.

These lines strongly support two Blob invariants:

```text
Workspace != Surface
Personal World != one physical machine
```

The same Workspace can appear as a rich desktop Surface, a phone review/action Surface or a watch status/action Surface.

## Semantic File Systems — View rather than folder

Semantic File Systems showed that virtual directories can be generated from semantic queries rather than only physical hierarchy.

The Blob extends this into Knowledge Objects with stable identity, provenance, relations and history. `View` is a saved semantic query; `Representation` is a derived artifact; `Projection` is a typed least-privilege slice of one object/resource.

## Exokernel and NIX — specialize the system for the workload

Exokernel research showed the value of keeping privileged mechanisms small and allowing higher layers to specialize abstractions. NIX explored dynamically assigning CPU roles for different workloads.

The Blob extends workload specialization into an AI-assisted, reversible process:

```text
observe workload
 -> understand bottleneck
 -> propose SystemSpec branch
 -> deterministic validation
 -> build isolated candidate
 -> simulate / benchmark / regress
 -> authorize if needed
 -> activate experimentally
 -> measure
 -> commit or rollback
```

The Constitutional Core remains outside normal adaptive mutation.

## Capability systems and Singularity — typed trust boundaries

Capability-oriented systems such as EROS/KeyKOS and research systems such as Singularity reinforce explicit authority, typed contracts and strong isolation boundaries.

The key Blob rule is:

> **AI interprets and proposes. Deterministic systems verify, authorize and materialize.**

An LLM is never the trusted computing base.

## Inheritance map

| Blob primitive | Historical relatives | Blob extension |
|---|---|---|
| Personal World | Plan 9 environment, Plan B, Octopus | Goals, semantic/context/causal memory |
| Compute Fabric | Plan 9, Plan B, Octopus | phone/watch/cloud/GPU placement |
| Workspace | Plan 9 private namespaces | capabilities + knowledge + UI + state + policy |
| Surface | Acme, Omero | adaptive semantic projection by device/context |
| Capability | UNIX tools, Inferno modules, Plan B Boxes | typed ability independent of runtime/location |
| Capability Capsule | Inferno modules | WASM/OCI/microVM/native/model/remote/hardware |
| Requirement Graph | Plan B joint constraints | policy/trust/privacy/cost/energy/quality |
| Conversion Graph | UNIX pipes, Plan B converters | typed semantic graph + controlled synthesis |
| Alfred | Plumber, Plan B context | temporal Situation reasoning + autonomous action |
| Knowledge Object | Semantic FS, Plan B Box | stable identity, relations, provenance, representations |
| Temporal/Causal Graph | Plan 9 dumps, Venti | branches + causal evidence and outcomes |
| Identity/Policy | Factotum, capability systems | human-readable delegated semantic authority |
| Adaptive System | Exokernel, NIX | AI-proposed, simulated, benchmarked, reversible evolution |
| AI Designed Workspace | partial precedents | personalized experience synthesis with measured trade-offs |
| Persistent Goal | weak direct precedent identified so far | objectives maintained across time/devices/workspaces |

## Architecture rules derived from the archaeology

1. **Intent does not directly execute software.** Intent becomes a typed problem, then deterministic resolution and authorization precede execution.
2. **Capability identity is independent of implementation.** `document.translate` remains the same semantic ability regardless of runtime/provider.
3. **Bind late and rebind only at safe boundaries.** Tasks should survive resource movement without arbitrary mid-effect migration.
4. **Converters/adapters are first-class capabilities.** Their cost, trust, quality, lossiness and provenance are explicit.
5. **Workspace is a semantic namespace.** It composes relevant knowledge, capabilities, tasks, policy and interaction structure.
6. **UI is inspectable system state.** Agents should target semantic Surface objects rather than pixels when possible.
7. **Historical state without causality is insufficient.** Meaningful autonomous changes need trigger, reason, evidence, authorization and observed outcome.
8. **AI is outside the trusted core.** It cannot bypass typed contracts, policy or recovery boundaries.
9. **Compatibility is a projection, not the architecture.** Legacy files/apps/windows remain supported without defining the native model.
10. **One Personal World may span heterogeneous host substrates.** The project does not require rewriting every device driver before proving its core model.

## What appears genuinely new in the composition

The Blob does not claim distributed OSs, semantic filesystems or dynamic modules as inventions. The research contribution may lie in the complete chain:

```text
Human Intent / persistent Goal
        |
        v
semantic understanding
        |
        v
RequirementGraph
        |
        v
deterministic candidate derivation + constraints
        |
        v
solver proposal
        |
        v
independent authorization/verification
        |
        v
late-bound Capability execution across Compute Fabric
        |
        v
verified outcome
        |
        v
persistent semantic state + causal memory
```

combined with adaptive Workspaces, device-specific Surfaces and safe self-evolution of the system itself.

## Research backlog

Important future archaeology includes disconnected/offline operation (Coda and related systems), CRDT/event-sourced multi-node state, KeyKOS/EROS persistence and capability security, Factotum delegation UX, Plan 9 namespace operations as Workspace inspiration, Omero split/merge semantics, Octopus legacy compatibility, Venti/Fossil storage, and the Wasm Component Model/WIT as one possible Capability ABI backend.

## Primary/high-value sources

- Plan 9 from Bell Labs — https://9p.io/sys/doc/9.html
- Plan 9 documentation — https://9p.io/sys/doc/index.html
- Acme — https://www.usenix.org/legacy/publications/library/proceedings/sf94/full_papers/pike.pdf
- Plumbing and Other Utilities — https://www.usenix.org/conference/2000-usenix-annual-technical-conference/plumbing-and-other-utilities
- Inferno Design Principles — https://inferno-os.org/inferno/design.html
- A Descent into Limbo — https://inferno-os.org/inferno/papers/descent.html
- Plan B / Ballesteros publication archive — https://lsub.org/books-papers/
- Personal Pervasive Environments / Octopus — https://pmc.ncbi.nlm.nih.gov/articles/PMC3435969/
- Semantic File Systems — DOI 10.1145/121132.121138
- Exokernel — https://pdos.csail.mit.edu/6.828/2008/readings/engler95exokernel.pdf
- Singularity — https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/osr2007_rethinkingsoftwarestack.pdf

## Current thesis

The Blob is not “Linux plus an LLM”. It is an attempt to build a per-user computational environment in which Intent is translated into typed requirements; deterministic mechanisms compose and place ephemeral Capabilities across a changing Fabric; persistent Workspaces, Knowledge Objects, Goals and semantic memory preserve continuity; Alfred interprets Situations over time; meaningful mutations are policy-controlled, explainable and causally versioned; and even the system itself can evolve through simulated, measurable and reversible experiments.
