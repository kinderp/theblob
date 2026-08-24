# Plan B Boxes — Deep Study and Architectural Consequences

**Status:** Research complete enough to inform Architecture v0.3 and Capability Contract v0.1.

## 1. Why this research matters

Plan B is not interesting to this project merely because it was a distributed OS. Its early **Box** abstraction attacked three problems that are central to our design:

1. resources are heterogeneous and appear/disappear/move;
2. applications should state *what resources must work together*, rather than manually wire low-level transports;
3. data transformations and placement should be selected by the system, not hard-coded into every program.

The most important conclusion from the study is that our v0.2 `CapabilityRequirement -> Resolver -> Binding` direction is sound, but too scalar. The Plan B design suggests resolving an **entire typed requirement graph** jointly.

## 2. Evolution of the idea

The relevant line is:

```text
1999  The Box: A Replacement for Files
      typed boxes + copy/share/select + converters

2004  Plan B: Boxes for networked resources
      boxes + constraints + dynamic discovery + late binding

2006-2007 Plan B evolves toward resource volumes / file-tree interfaces
      constraints + discovery + adaptive namespaces remain
      custom Box API is replaced at interoperability boundaries by
      high-level virtual file interfaces
```

This evolution is as important as the Box idea itself. The later Plan B work argues that a novel middleware abstraction can destroy interoperability with ordinary tools. Their response was to retain dynamic resource discovery, constraints, per-user/per-process namespaces and adaptation, while exposing resources through inspectable virtual file trees.

**Lesson for us:** keep a rich typed semantic model internally, but always provide simple, stable, inspectable compatibility projections. Do not require every tool in the world to speak a new protocol merely to participate.

## 3. What a Box actually was

A Box was intended as a uniform abstraction for data and resources. Important properties included:

- a **type**;
- data/content;
- possible nested/inner Boxes;
- a name within a namespace;
- in Plan B, a **constraint set** describing resource properties;
- operations that describe higher-level relationships between endpoints.

The key operations evolved, but the early Box work centers on:

```text
copy(source, target)
share(source, target)
select(container, selector)
```

The 2004 Plan B version emphasizes `copy` and `link`, plus metadata, discovery/import and related operations.

The important design idea is not the exact syscall list. It is that the OS sees enough of the *whole operation* to reason about placement, compatibility and adaptation.

## 4. `copy` is much more interesting than read/write

Traditional code often decomposes a semantic operation:

```text
read(source) -> client buffer -> write(target)
```

That hides the relationship between source and destination from the OS. A Box `copy(source,target)` tells the system both endpoints at once.

That enables the system to choose:

- a direct source-to-target data path;
- a converter if source and target types differ;
- a pair of resources that are mutually compatible;
- a compute node near the data;
- caching or copy-on-write strategies;
- whether data needs to move at all.

### Consequence for our OS

Capability execution should be **declarative dataflow**, not an opaque sequence of reads and writes controlled by a Capsule.

A Capsule should receive typed object/resource handles and an explicit data/effect envelope. The execution planner should retain authority over where data flows.

```text
Task
  |
  v
Requirement Graph
  |
  +-- input Knowledge Object
  +-- desired output
  +-- capability roles
  +-- resource roles
  +-- constraints
  |
  v
Binding Plan
  |
  +-- capsules
  +-- nodes
  +-- adapters
  +-- data routes
  +-- grants
```

This is stronger than binding one capability in isolation.

## 5. Joint resource resolution is the major Plan B lesson

Plan B constraints are not just resource tags. For an operation involving multiple Boxes, constraints are unified across the participating resources.

The classic example is executing a binary on a processor: selecting a binary independently and then selecting a CPU independently can produce an incompatible pair. Plan B searches for a pair whose constraints are jointly satisfiable.

### Architectural consequence: Requirement Graph

Replace the assumption that the resolver receives only one scalar `CapabilityRequirement` with a more general **RequirementGraph**.

```text
RequirementGraph

Role: source
  type = Document
  object = report-X

Role: output
  type = PrintedDocument

Role: printer
  capability = print.accept
  hard: location = near-user
  hard: trust >= managed

Role: renderer
  capability = document.render

Relations:
  source -> renderer -> printer

Global constraints:
  privacy = local-network-only
  deadline <= 10s
  cost = 0
```

The solver binds all roles and edges together.

This prevents a class of local-optimum mistakes and gives us a natural model for multi-device Tasks.

## 6. Constraints: what to keep and what to change

Plan B used syntactic constraints and unification inspired by logic programming. Later editions moved to clearer attribute/value forms and used constraints for properties such as location, owner and connection quality.

We should preserve the principle but modernize the model.

### Proposed constraint classes

```text
Hard constraints
  must be satisfied

Soft preferences
  desirable, can be traded off

Objectives
  quantities to minimize/maximize

Policies
  authority/trust rules that cannot be overridden by ranking
```

Examples:

```text
hard:
  data_residency = local
  network != public
  required_gpu_memory >= 20GiB

prefer:
  node = workstation
  implementation = cached

objectives:
  minimize latency
  minimize energy
  maximize quality

policy:
  model may read Object A
  model may not send Object A outside Fabric
```

The solver should not merely return the first valid solution. It should produce a **valid Pareto/ranked set** or a deterministic best binding under an explicit objective function.

## 7. Converter graph: an almost direct ancestor of our Capability Graph

The Box design supports automatic conversion when source and target types are not directly compatible. Converters can be composed into chains when no single converter is sufficient. A converter may also have multiple inputs and outputs.

This maps directly to our Adapter/Converter Capability model:

```text
A --converter-1--> B --converter-2--> C
```

But our graph must add metadata Plan B did not model strongly enough:

```text
lossiness
quality
trust
privacy
cost
latency
energy
determinism
reversibility
side effects
provenance
version
```

### Important rule

A type-compatible path is not automatically an acceptable path.

For example:

```text
ConfidentialDocument
   -> cloud OCR
   -> Text
```

may be type-correct but policy-invalid.

Therefore converter-path search belongs *inside* deterministic resolution and policy checking.

## 8. Derived data: Plan B anticipated our Representations

One of the strongest ideas in the Box paper is non-compatible `share` via a converter. Instead of pretending a lossy reverse conversion can reconstruct the source, the derived target can become a generated read-only dependency that is regenerated when needed.

That is extremely close to our model:

```text
Knowledge Object
       |
       +-- render.pdf ----> PDF Representation
       +-- summarize -----> Summary Representation
       +-- speech --------> Audio Representation
```

### Architecture refinement

Representations should be first-class **derived nodes** with:

```text
source commit(s)
transformation capability/version
parameters
provenance
fresh/stale state
materialization cache
rebuild policy
```

A change to an upstream Knowledge Object does not mutate every representation. It invalidates dependent materializations; they are regenerated lazily or proactively according to policy.

This gives us a build-system-like model for user data.

## 9. `select`: semantic projections instead of byte ranges

The early Box proposal allows selectors whose syntax depends on the Box type. A document can expose semantic selections such as sections or title; the same object can also expose lower-level textual selections.

This suggests a missing concept in our current vocabulary:

# Projection

A **Projection** is a typed, authorized selection over a Knowledge Object or resource.

Examples:

```text
Document.section("Architecture")
Document.text.lines(50..100)
Code.symbol("CapabilityResolver")
Image.region(...)
Table.rows(where=...)
MailThread.messages(after=...)
```

A Projection can be materialized lazily and need not correspond to a physically contiguous byte range.

### Security benefit

Capabilities should often receive a Projection rather than the entire Knowledge Object.

```text
grammar.check
  grant: Document.plain_text
  deny: comments
  deny: author metadata
  deny: embedded private assets
```

This makes least-privilege data access far more precise.

## 10. No `open`: late binding and resilience

Plan B deliberately challenged long-lived descriptors/connections. Names could be resolved at operation time so a different available resource could be selected after mobility or failure.

We should adopt the principle, but not literally rebind arbitrarily during non-idempotent work.

### Architecture refinement: Binding Lease

A **BindingLease** is a bounded commitment to a concrete binding.

```text
CapabilityRequirement / RequirementGraph
          |
          v
BindingPlan
          |
          v
BindingLease
  implementation
  node(s)
  grants
  expiry / validity conditions
  safe-rebind boundary
```

Safe re-resolution can happen:

- before an operation;
- between idempotent steps;
- when a lease expires;
- after a node/resource failure;
- when policy changes;
- at explicit Task checkpoints.

It must **not** silently move a non-idempotent operation halfway through execution unless the operation contract explicitly supports that semantics.

## 11. Advertisements and dynamic environment

Plan B resources are advertised, discovered and bound into a process/user environment. When a resource disappears, the system can cleanly remove it and search for an alternative.

This maps to our Fabric:

```text
NodeAdvertisement
  node identity
  trust
  location
  connectivity
  resource inventory
  hardware capabilities
  runtime backends
  sensors/actuators
  current telemetry
```

and:

```text
CapabilityAdvertisement
  contract
  implementation
  version
  placement requirements
  trust/signature
  quality/cost profile
```

Advertisements are observations, not authority. A discovered provider still must pass identity, policy, attestation and contract validation.

## 12. The `/usr` insight: the user as a routable resource context

Plan B experimented with a user-associated resource describing preferred/current I/O resources. A task could therefore follow the human rather than remain attached to one terminal.

This is a direct ancestor of our Personal World + Surface concept.

Our stronger model should treat user presence/context as inputs to binding, not as a fixed device identity:

```text
user.current_surface
user.current_location
user.attention
user.available_input
user.available_output
```

Alfred updates the Situation; the Requirement Graph can then re-resolve display/input/output roles at safe boundaries.

## 13. Atomic operations and causal recording

Plan B made Box operations atomic relative to operations on the same Box. Our equivalent should be stronger where possible:

```text
plan
 -> authorize
 -> execute transaction/effect step
 -> verify
 -> causal commit
```

Not every external effect can be rolled back. Therefore a Capability Contract must declare effect semantics:

```text
pure
idempotent
transactional
compensatable
irreversible
```

This metadata directly affects rebinding, retries, simulation, approval and recovery.

## 14. Why later Plan B moved back toward file interfaces

The later Plan B work is a warning against falling in love with a beautiful abstraction.

The researchers argued that middleware-specific abstractions force users/programmers to acquire specialized tools and hurt interoperability. They moved toward **high-level resource interfaces projected as virtual file trees**, while retaining discovery, constraints, adaptive importing and namespaces.

This is not evidence that Boxes were useless. It says two things:

1. rich internal semantics are valuable;
2. the universal interoperability/introspection boundary must be extremely simple.

### Consequence for our OS: Compatibility / Introspection Plane

Native components should use typed contracts. But every important object/resource/capability should have one or more simple projections for generic tools.

Candidate bridges (not frozen):

```text
virtual file-tree projection
CLI / structured text
JSON/CBOR introspection
9P-like remote bridge
WIT/component interface
legacy POSIX file export
```

The bridge is not the semantic source of truth. It is an **escape hatch and interoperability surface**.

This preserves the Bell Labs lesson: the system should remain inspectable and scriptable even when its native model is richer than files.

## 15. What we should NOT copy from Plan B

### Positional/weak constraint semantics

Use versioned named schemas and units, not ad-hoc positional strings.

### First-match converter choice

Use explicit deterministic optimization objectives and explainable ranking.

### Weak security model

Provider discovery and type compatibility never imply authorization. The Constitutional Core and Policy Engine remain authoritative.

### Rebinding everywhere without effect semantics

Use BindingLeases and declared safe points.

### Type alone as meaning

Our type system needs semantic schemas, provenance and policy labels where relevant.

### Custom abstraction as the only integration interface

Provide compatibility/introspection projections from day one.

## 16. Proposed native resolution model

The key v0.3 abstraction should be:

```text
Intent / Situation
       |
       v
Semantic Planner
       |
       v
RequirementGraph
       |
       | roles + relations
       | typed inputs/outputs
       | hard constraints
       | preferences/objectives
       | effect requirements
       | authority requirements
       v
Policy-enriched deterministic solver
       |
       +---- Type / schema graph
       +---- Capability graph
       +---- Adapter graph
       +---- Fabric resource graph
       +---- Trust/authority graph
       +---- telemetry / availability
       |
       v
BindingPlan
       |
       + implementations / Capsules
       + Fabric node placement
       + adapters
       + data routes
       + grants
       + expected effects
       + score / alternatives
       + resolution explanation
       |
       v
BindingLease(s)
       |
       v
Execute -> Verify -> Causal Commit
```

## 17. A concrete example: “print this document near me”

```text
Intent
  "print this document near me"

Semantic interpretation
  source = Object#report
  outcome = physical.printed-copy
  context = user.current_location

RequirementGraph
  [Object#report: StructuredDocument]
          |
          v
  [document.render]
          |
          v
  [Printer accepts ?]

Hard constraints
  printer.location = user.location
  document.classification != leave-private-fabric
  authorization = user.print

Soft preferences
  duplex = true
  color = false
  prefer low energy

Solver discovers
  Printer A accepts PDF
  Printer B accepts PostScript
  local renderer creates PDF
  server renderer creates PostScript

BindingPlan 1
  local renderer -> PDF -> Printer A
  no external data route
  latency 4.2s
  energy low

BindingPlan 2
  server renderer -> PS -> Printer B
  latency 2.8s
  policy invalid: document may not leave local node

Result
  Plan 1 selected for policy validity, not merely speed.
```

This example captures the key lesson: AI understands “near me” and “print”; the deterministic graph resolver proves how it can be done within policy.

## 18. New/modified concepts proposed after the study

| Concept | Change |
|---|---|
| CapabilityRequirement | remains valid for simple single-role requests |
| **RequirementGraph** | NEW: joint multi-role resolution problem |
| Capability Binding | retained as an individual bound role |
| **BindingPlan** | NEW: complete valid execution/placement/data-flow solution |
| **BindingLease** | NEW/refinement: scoped binding with safe re-resolution rules |
| Adapter/Converter Capability | strengthened with path metadata and optimization |
| **Projection** | NEW: typed semantic selection over a Knowledge Object/resource |
| Representation | strengthened as reactive derived node with provenance/invalidation |
| Compatibility Plane | NEW: generic inspectable projections for old/general-purpose tools |

## 19. Implications for MVP-0

MVP-0 should remain small, but two changes are worth making now:

1. represent its `test.run` resolution internally as a one-node `RequirementGraph`, so the data model scales without a rewrite;
2. require the resolver to return a `BindingPlan` with an explanation, even when there is only one valid implementation.

Do **not** add multi-hop conversion, distributed placement or live rebinding to MVP-0 yet.

A small second test case can prove the graph model without making the demo larger:

```text
Requirement: test.run
Candidates:
  local-test-capsule
    network = denied
  cloud-test-service
    network = required

Policy:
  network = denied

Expected:
  local candidate selected
  cloud candidate rejected with structured reason
```

## 20. Research questions left open

- What constraint formalism should v0.1 use: custom typed predicates, SMT, Datalog/logic engine, or a deliberately tiny evaluator?
- How should hard constraints and soft multi-objective ranking compose deterministically?
- What constitutes type compatibility: nominal schema, structural schema, semantic ontology, or layered combination?
- How are units, ranges and uncertainty represented?
- How much of the BindingPlan must be cryptographically attestable?
- How should consistency be expressed when a Knowledge Object is replicated across Fabric nodes?
- What is the minimal generic compatibility projection that remains useful without becoming the native semantic model?
- Can the conversion graph be cached safely across policy/context changes?
- How do we detect lossy or non-reversible conversion cycles?

## 21. Sources

Primary/near-primary material used for this study:

- Francisco J. Ballesteros, Sergio Arévalo, **The Box: A Replacement for Files**, HotOS VII, 1999. DOI: 10.1109/HOTOS.1999.798373.
- Francisco J. Ballesteros et al., **Plan B: Boxes for networked resources**, Journal of the Brazilian Computer Society 10, 2004. DOI: 10.1007/BF03192352.
- Francisco J. Ballesteros et al., **Plan B: Using Files instead of Middleware Abstractions**, IEEE Pervasive Computing 6(3), 2007. DOI: 10.1109/MPRV.2007.65.
- Francisco J. Ballesteros, author project/publication archive: https://lsub.org/books-papers/

The historical evolution is deliberately treated as design evidence, not as a requirement to reproduce Plan B APIs.
