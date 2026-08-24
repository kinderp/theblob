# Architecture v0.4

**Status:** research architecture; vocabulary is becoming stable enough for MVP-0, implementation choices remain replaceable.

Architecture v0.4 incorporates lessons from UNIX, Plan 9, Inferno, Plan B Boxes, Omero/Octopus, Semantic File Systems, Exokernel, capability systems, modern dependency solvers and SMT/constraint solving.

See:

- [`ARCHAEOLOGY.md`](ARCHAEOLOGY.md)
- [`research/PLAN-B-BOXES.md`](research/PLAN-B-BOXES.md)
- [`research/CONSTRAINT-SOLVER-STUDY.md`](research/CONSTRAINT-SOLVER-STUDY.md)
- [`CAPABILITY-CONTRACT-v0.1.md`](CAPABILITY-CONTRACT-v0.1.md)
- [`RESOLUTION-CONTRACT-v0.1.md`](RESOLUTION-CONTRACT-v0.1.md)

## 1. Central architectural rules

```text
AI interprets, reasons, proposes and synthesizes.
Deterministic systems verify, authorize and materialize.
```

And now, more precisely:

```text
A solver proposes a BindingPlan.
A simpler independent verifier decides whether that concrete plan is valid.
```

Neither an LLM nor an SMT solver is itself an authority boundary.

## 2. High-level model

```text
                              HUMAN
                                |
                     Intent / Goals / Choice
                                |
                                v
+-------------------------------------------------------------------+
|                         PERSONAL WORLD                            |
| identity | preferences | context | memory | objects | goals       |
| Workspaces | Tasks | policy | semantic state | causal references  |
+--------------------------------+----------------------------------+
                                 |
                +----------------+----------------+
                |                                 |
                v                                 v
+-------------------------------+   +--------------------------------+
|   SEMANTIC / COGNITIVE PLANE  |   |     ALFRED / NERVOUS SYSTEM    |
| intent interpretation          |   | event normalization             |
| goal decomposition             |   | deterministic correlation       |
| plan proposals                 |   | candidate situations            |
| requirement synthesis          |   | semantic interpretation          |
| explanation                    |   | action proposals                 |
+---------------+---------------+   +----------------+---------------+
                |                                    |
                +------------------+-----------------+
                                   |
                                   v
+-------------------------------------------------------------------+
|                       REQUIREMENT GRAPH                            |
| typed roles | relations | desired outputs | effect envelope        |
| policy | hard constraints | preferences | explicit objectives      |
+--------------------------------+----------------------------------+
                                 |
                                 v
+-------------------------------------------------------------------+
|                     RESOLUTION PIPELINE                            |
|                                                                   |
| candidate derivation / recursive closure                          |
|       Datalog-style / deterministic graph algorithms              |
|                         |                                         |
|                         v                                         |
|                   Candidate Graph                                 |
|                         |                                         |
|                         v                                         |
| constraint compilation -> SMT/MaxSMT backend (Z3 first)           |
|                         |                                         |
|                         v                                         |
|                   SolverProposal                                  |
|                         |                                         |
|                         v                                         |
|              independent Rust BindingVerifier                     |
|                         |                                         |
|                         v                                         |
|                   BindingPlan/Lease                               |
+--------------------------------+----------------------------------+
                                 |
                                 v
+-------------------------------------------------------------------+
|                         CAPABILITY FABRIC                          |
| contracts | registry | converter graph | versions | implementations|
+---------+-------------+-------------+-------------+----------------+
          |             |             |             |
        WASM           OCI         microVM        native
          |             |             |             |
          +-------------+------+------+-------------+
                               |      |
                            remote   hardware
                               |      |
                               +--+---+
                                  |
                                  v
+-------------------------------------------------------------------+
|                          COMPUTE FABRIC                            |
| PC | phone | watch | server | cloud | IoT | ambient resources      |
| node discovery | trust | availability | telemetry | placement      |
+--------------------------------+----------------------------------+
                                 |
                                 v
+-------------------------------------------------------------------+
|                         WORKSPACE ENGINE                           |
| semantic namespace | Recipes | Workspace state | Tasks             |
| experience grammar | Surface model | UI components | context views |
+--------------------------------+----------------------------------+
                                 |
                                 v
+-------------------------------------------------------------------+
|                         ADAPTIVE SYSTEM                            |
| declarative SystemSpec | kernel | drivers | runtime | services     |
| candidate branches | build/test/simulate | benchmark | activate   |
+--------------------------------+----------------------------------+
                                 |
                                 v
+-------------------------------------------------------------------+
|                       CONSTITUTIONAL CORE                          |
| identity root | trusted boot | delegated authority | verifier      |
| policy root | recovery | known-good boot | rollback               |
+-------------------------------------------------------------------+

       TEMPORAL / CAUSAL GRAPH spans every meaningful transition
```

The drawing is layered for readability, but Workspace, Capability and Compute Fabric interact bidirectionally at runtime.

## 3. Personal World

The Personal World is the persistent per-user system image at the logical level. It is not the state directory of one machine.

It owns or references:

- identity and delegated authority;
- preferences and semantic policies;
- Workspaces and Workspace Recipes;
- Tasks and Goals;
- Knowledge Objects and their relations;
- semantic memory and provenance;
- causal references to important changes/actions;
- node membership and trust information.

Node-local caches and ephemeral runtime state do not automatically belong to the Personal World.

## 4. Workspace as semantic namespace

A Workspace is a relatively persistent, user-recognizable environment for a domain of activity.

It generalizes a Plan 9 private namespace beyond resources:

```text
Workspace
  + relevant Knowledge Objects / Views / Projections
  + persistent context
  + experience grammar
  + baseline/pinned Capability requirements
  + dynamic Capability requirements
  + policies
  + Task set
  + Surface state
```

A Workspace does **not** own user data or executable implementations.

### Creation modes

- **Ready** — curated, tested and benchmarked Recipe.
- **AI Designed** — AI Workspace Architect proposes a composition from goals, hardware and preferences; deterministic checks/benchmarks validate it.
- **Expert** — explicit component, policy, implementation and optimization control.

Workspace Recipes are intended to be forkable, mergeable, benchmarkable and publishable.

## 5. Surface model

`Workspace != Surface`.

A Workspace is logical; a Surface is its projection onto current I/O capabilities.

```text
                 Same Workspace
                      |
       +--------------+---------------+
       |              |               |
     desktop         phone           watch
  rich/full view   review/action   status/action
```

Surface state must be typed and inspectable so agents manipulate semantic elements rather than pixels where possible.

### Graphics prototype

```text
Workspace / Surface Model
          |
       UI Schema
          |
         Slint
          |
 prototype shell
          |
      Hyprland
          |
       Wayland
```

Hyprland is a prototype dependency only.

Longer-term option:

```text
Workspace / Surface Engine
          |
        Slint
          |
Our Wayland Shell
          |
Rust compositor (Smithay)
          |
DRM/KMS/Mesa/Linux
```

## 6. Knowledge Objects, Projections, Representations and Views

The physical layer may still contain bytes/files. The primary user abstraction is intended to be a persistent object with stable identity.

```text
Knowledge Object
  identity
  content/data
  logical structure
  semantic metadata
  relations
  provenance/confidence
  history
```

Three distinct concepts sit above it:

### Projection

A typed semantic slice exposed with least privilege.

```text
Document.section("Architecture")
Document.plain_text
Code.symbol("CapabilityResolver")
Image.region(...)
```

### Representation

A derived materialization such as PDF, DOCX, HTML, audio or thumbnail. Representations may be cached, stale or absent and rebuilt from source object commits.

### View

A saved semantic query/collection over objects, replacing many folder-use cases while remaining exportable through compatibility projections.

## 7. Capability model

A Capability is an abstract typed ability independent of implementation and execution location.

```text
capability: document.translate
input: Document
output: Document
```

A Capability Capsule is a concrete implementation, potentially:

- WASM/WASI component;
- OCI container;
- microVM;
- native component;
- local AI model;
- remote service;
- hardware/device capability.

Runtime code is usually disposable; user state is not.

## 8. Requirement Graph

A Task requests outcomes and relations, not executables.

A simple scalar requirement is a one-role graph. Complex tasks bind multiple roles jointly.

```text
RequirementGraph

source: ConfidentialDocument
translator: document.translate
model: quality >= 950000 ppm
node: local Personal Fabric only
output: TranslatedDocument

relations:
source -> translator -> output
translator uses model
translator executes_on node
```

Constraint classes are explicitly separated:

```text
POLICY       non-negotiable authority/trust/privacy
HARD         task correctness
PREFERENCE   desirable but tradeable
OBJECTIVE    measurable ranking value
```

## 9. Resolution engine v0.4

The resolver is intentionally **hybrid and staged**.

### Stage A — candidate derivation

Derive finite candidates and recursive relations:

- type/converter reachability;
- implementation availability;
- node/resource compatibility;
- trust and visibility closure;
- derived policy facts.

The conceptual model is Datalog-like. MVP-0 may use ordinary deterministic Rust graph code. Later we can introduce an incremental Datalog engine if profiling/complexity justifies it.

### Stage B — policy pre-filter

Reject obviously invalid candidates before optimization and record domain-level reasons.

### Stage C — SMT/MaxSMT solving

Compile the remaining finite problem from our own Constraint IR into an SMT backend. Z3 is the first backend.

SMT determines feasibility and ranks valid combinations under explicit objectives.

### Stage D — independent verification

A simple Rust `BindingVerifier` checks the concrete selected plan against canonical rules. It does not rely on the SMT solver to authorize the result.

### Stage E — lease

A verified BindingPlan becomes a scoped BindingLease with defined rebind boundaries.

## 10. Constraint IR

No LLM, Capsule or Workspace Recipe may inject raw SMT.

The OS owns a backend-neutral typed IR using a restricted domain:

```text
bool
stable IDs
finite enums/sets
bounded integers
fixed-point integer metrics
```

Typical expressions:

```text
Eq / Ne
Lt / Le / Gt / Ge
In
And / Or / Not / Implies
ExactlyOne / AtMost / AtLeast
```

Avoid arbitrary nonlinear arithmetic, unrestricted quantification and raw string reasoning in the runtime binding path.

Metrics use integer canonical units such as `latency_us`, `energy_uj`, `quality_ppm` and `memory_bytes`.

## 11. Optimization and deterministic choice

Autonomous binding uses an explicit versioned **lexicographic objective profile**.

Example:

```text
1 policy + hard validity
2 quality class
3 external monetary cost
4 latency
5 energy
6 rebind churn
7 stable implementation/node ID tie-break
```

Pareto fronts are useful in interactive comparison mode, not as an unspecified autonomous choice rule.

Policy is never silently traded for performance.

## 12. Explanation and ResolutionTrace

Every resolution produces a backend-neutral trace containing:

```text
RequirementGraph hash
world/fabric snapshot
normalized constraints
candidate provenance
candidate rejections
solver backend/status
unsat core if available
solver proposal
verification result
objective vector
tie-break decision
selected BindingPlan
```

Explanations combine:

1. derivation provenance — why a path/candidate exists;
2. rejection reasons — why candidates were pruned;
3. solver evidence — model or unsat core.

The user receives domain explanations, not SMT syntax. Important traces become causal evidence.

## 13. Specialized solvers

One universal solver is a non-goal.

### PubGrub-like dependency solver

Used for Capsule/Workspace Recipe versions and transitive dependencies. Its derivation-tree error style is a model for human-readable incompatibility explanations.

### CP-SAT / operations research

Reserved for future global temporal scheduling and large-scale resource placement across many tasks/nodes.

### cvc5

Candidate reference/differential backend for proof/unsat-core experiments and verification of our solver compiler.

## 14. Late binding and safe re-resolution

A Task depends primarily on abstract Capability roles. Concrete implementation/node binding happens as late as practical and may be reconsidered only at defined safe boundaries.

```text
BindingLease
  valid_until
  rebind_allowed_at
  grants
  selected implementations/nodes
  objective baseline
```

To avoid thrashing, rebinding includes churn cost/hysteresis. A slightly faster resource appearing does not automatically trigger migration.

## 15. Alfred v0.4

```text
Sensors / Raw Events
        |
normalized Event Envelope
        |
deterministic temporal correlation
        |
candidate Situation
        |
AI semantic interpretation
        |
structured Situation
        |
policy / authority checks
        |
RequirementGraph / PlanCandidate
        |
resolution pipeline
        |
verified execution
```

Alfred changes facts in the Fabric/World; those changes may trigger incremental re-resolution later.

## 16. Failure semantics

Solver failure is a defined OS state:

```text
SAT      -> verify -> possibly execute
UNSAT    -> explain / request constraint change
UNKNOWN  -> no new privileged binding
TIMEOUT  -> no new privileged binding
```

A still-valid existing lease may continue only when policy permits. Timeout never relaxes policy.

## 17. Temporal storage vs causal history

These remain separate.

### Temporal storage

Preserves reconstructible state:

- SystemSpec/Nix generations;
- filesystem/object chunks;
- Workspace Recipes;
- Knowledge Object versions;
- policy versions;
- solver/problem artifacts when retention policy permits.

### Causal history

Explains meaningful changes:

```text
what
why
who/agent
trigger
evidence
expected effect
actual effect
side effects
authorization
resolution trace
parents / branch / merge
rollback reference
```

## 18. Identity and delegated authority

Credentials remain under a trusted identity/authority service. Capsules receive narrow scoped grants, never ambient reusable secrets.

```text
mail.send
scope = recipient:X
expires = +10m
max_messages = 1
```

Object Projections provide equivalent least-privilege data access.

## 19. Adaptive System

System evolution follows a transaction-like lifecycle:

```text
observation / goal
      |
AI candidate SystemSpec change
      |
policy + schema validation
      |
branch
      |
build isolated candidate
      |
simulation / VM / regression / benchmark
      |
explain predicted effects
      |
authorization if required
      |
controlled activation
      |
measure actual effects
      |
commit or rollback
```

Kernel/modules/drivers may eventually be modified, but the Constitutional Core cannot be disabled through normal adaptive mechanisms.

## 20. Node substrates

The Personal World is logically unified even when substrates differ.

```text
PC/server        Linux + NixOS prototype substrate
phone/watch      Android/GKI-compatible substrate initially
legacy systems   hosted Fabric node/runtime where practical
```

NixOS is a backend for declarative system materialization, not the definition of the product.

## 21. Prototype implementation direction

- trusted/core runtime: Rust;
- AI/ML experimentation: Python where useful;
- capability contracts: language-neutral abstraction;
- first resolution backend: Z3 behind our Constraint IR;
- candidate derivation: plain Rust first, Datalog-style engine later as justified;
- version dependency solving: PubGrub candidate;
- future scheduling: CP-SAT candidate;
- capability runtimes: WASM/WASI, OCI, microVM, native, remote;
- UI: Slint over prototype Wayland environment;
- Hyprland: prototype compositor only;
- Nix: initial declarative SystemSpec backend.

## 22. Architectural non-goals for v0.4

Architecture v0.4 does not require:

- a new kernel;
- one identical OS image on every device;
- replacing all files or legacy apps immediately;
- a runtime Datalog engine in MVP-0;
- CP-SAT in MVP-0;
- allowing LLM-generated raw solver formulas;
- trusting Z3/cvc5 as authorization components;
- proving globally optimal placement for every action;
- choosing one permanent capability ABI now.

## 23. Architecture invariant

```text
Human intent / system situation
            |
     semantic understanding
            |
       RequirementGraph
            |
 candidate derivation + policy
            |
 deterministic solver proposal
            |
 independent verification
            |
       BindingLease
            |
 ephemeral capability execution
            |
       verified outcome
            |
 persistent state + causal record
```

If an implementation collapses this into "LLM runs arbitrary commands" or "solver output automatically grants authority", it is no longer implementing this architecture.
