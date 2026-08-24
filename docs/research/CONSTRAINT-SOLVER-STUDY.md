# Constraint Solver Study — From Plan B Unification to a Deterministic Resolution Engine

**Status:** architecture research informing v0.4  
**Scope:** runtime capability/resource resolution, converter paths, version/dependency selection, placement and future scheduling.

## 1. Research question

Plan B showed that resources should be selected jointly under constraints rather than one endpoint at a time. Our v0.3 architecture therefore introduced a `RequirementGraph` and a deterministic resolver.

The open question was which solving model should sit behind that resolver:

- Plan B-style unification;
- Datalog / Horn-clause reasoning;
- SAT / MaxSAT;
- SMT (Z3/cvc5);
- constraint programming / CP-SAT;
- dependency solvers such as PubGrub;
- or a custom solver.

The conclusion is **not one universal solver**. The resolution problem naturally separates into stages with different mathematical structure.

## 2. Decision in one diagram

```text
                  RequirementGraph
                         |
                         v
             Constraint IR + typed facts
                         |
       +-----------------+------------------+
       |                                    |
       v                                    v
 Candidate / relation closure         Policy pre-filter
   Datalog-style engine               deterministic Rust
       |                                    |
       +-----------------+------------------+
                         |
                         v
                 Candidate Graph
                         |
                         v
                SMT / MaxSMT backend
              (Z3 first implementation)
                         |
             feasibility + ranking
                         |
                         v
                    BindingPlan
                         |
                         v
              Independent Rust Verifier
                         |
                  valid / reject
                         |
                         v
                  BindingLease
```

Specialized sub-problems are delegated to specialized solvers:

```text
capsule/recipe versions   -> PubGrub-style dependency solver
future global scheduling -> CP-SAT / operations-research backend
recursive graph facts     -> Datalog-style derivation
runtime feasibility       -> SMT / MaxSMT
```

## 3. Why Plan B unification alone is not enough

Plan B's constraint/unification model is elegant for matching attributes and jointly resolving resource pairs or tuples. Our problem adds constraints that are substantially richer:

```text
RAM >= 20 GiB
latency <= 150 ms
cost == 0
data_residency == local
trust(node) >= managed
implementation.version in compatible-range
if network == public -> encryption == required
at most one irreversible external effect
maximize quality
minimize energy
prefer current binding unless benefit exceeds threshold
```

A modern solver should support Boolean structure, equality, finite-domain choices, integer/rational arithmetic and explicit optimization.

Plan B remains the conceptual ancestor of the **joint-resolution model**, but not the final solving technology.

## 4. Datalog: excellent for derivation, not final optimization

Datalog naturally represents recursive facts such as:

```text
reachable_type(A, B) :- converter(A, B).
reachable_type(A, C) :- converter(A, B), reachable_type(B, C).

can_execute(Cap, Node) :- implements(Impl, Cap), supported_on(Impl, Node).

trusted_path(A, B) :- trusted_link(A, B).
trusted_path(A, C) :- trusted_link(A, B), trusted_path(B, C).
```

This is exactly the kind of work needed to derive:

- converter reachability;
- capability implementation candidates;
- resource visibility;
- trust/relationship closure;
- policy facts;
- compatibility relations;
- facts derived from the Compute Fabric.

Datalog engines also have a useful notion of derivation/provenance: a fact can be explained by the rules and facts that derived it.

### Why Datalog is not the whole resolver

It is less natural as the only engine for:

- rich numeric optimization;
- multi-objective ranking;
- arbitrary combinations of arithmetic and Boolean constraints;
- soft preferences with weights/priorities;
- scheduling.

**Decision:** use Datalog-style reasoning for closure and candidate derivation, not as the sole binding solver.

### Rust path

`Ascent` is a Rust-embedded Datalog-style system and supports recursive fixed-point relations. It is a plausible experimental implementation. We should keep the architecture independent of Ascent and may start MVP-0 with ordinary Rust indexed graph algorithms before introducing a logic engine.

## 5. SMT / MaxSMT: best fit for runtime binding

SMT (Satisfiability Modulo Theories) combines SAT-style Boolean reasoning with theories such as integers, reals, arrays and algebraic datatypes.

Our runtime resolution problem maps well to a bounded SMT model:

```text
choose implementation_A : Bool
choose implementation_B : Bool
choose node_pc          : Bool
choose node_server      : Bool

exactly_one(implementation_A, implementation_B)
exactly_one(node_pc, node_server)

choose implementation_A -> gpu_memory(node) >= 12_GiB
confidential_input       -> network_path != public
chosen_capsule           -> signature_verified
```

### Hard constraints

Safety/policy constraints are assertions that must hold.

### Soft preferences

Preferences can be compiled to soft constraints / MaxSMT.

### Objectives

Latency, energy, cost and quality can be expressed as explicit objectives. Z3 supports lexicographic, Pareto and independent objective modes.

### Unsatisfiable cores

A major advantage is the ability to retrieve an **unsat core**: a subset of named constraints sufficient to make the problem impossible. This is useful for human explanations such as:

```text
No valid binding exists because:
- data must remain local;
- the local nodes have < 20 GiB GPU RAM;
- all implementations satisfying quality >= 0.95 require >= 20 GiB GPU RAM.
```

An unsat core is not by itself a good user explanation, but it is excellent raw material for our `ResolutionTrace` / explanation builder.

## 6. Why Z3 first

Z3 is the recommended first SMT backend because:

- mature SMT implementation;
- optimization / MaxSMT support;
- lexicographic and Pareto objectives;
- models and unsat cores;
- incremental scopes (`push`/`pop`);
- mature current Rust bindings (`z3` crate);
- MIT licensing;
- Datalog/Horn fixed-point engine also exists if useful for experimentation.

### Important architectural boundary

**Z3 is not part of the trusted authorization boundary.**

A solver may contain bugs, return `unknown`, time out, or select a non-optimal binding. Safety must not depend on trusting the solver implementation.

The flow is:

```text
Solver proposes BindingPlan
          |
          v
Independent Rust Verifier
          |
          +-- checks every hard constraint
          +-- checks policy/authority
          +-- checks type/effect compatibility
          +-- checks selected resources and versions exist
          +-- checks lease/rebind safety
          |
          v
      authorize or reject
```

If the solver produces a wrong SAT model, the verifier rejects it.

If the solver incorrectly reports `unsat`, execution does not happen; this is an availability failure, not an authority escalation.

Optimization correctness is desirable but is not a security boundary.

## 7. cvc5 as a second/reference backend

cvc5 supports models, incremental solving, unsat cores and proof production. It is a strong future candidate for:

- differential testing against Z3;
- high-assurance research;
- proof/certificate experiments;
- validating our Constraint IR compiler.

We should therefore keep a backend-neutral `SolverBackend` interface.

The initial production prototype should not depend on multiple solvers at runtime.

## 8. CP-SAT / constraint programming

Google OR-Tools CP-SAT is particularly strong for integer combinatorial optimization and scheduling.

It becomes attractive when our problem evolves from:

```text
bind this task now
```

to:

```text
schedule 300 tasks across 20 nodes over the next hour
respect GPU/CPU/memory capacities
respect deadlines
avoid device wakeups
minimize energy and migration
```

This is a different problem class.

### Decision

Do **not** use CP-SAT for MVP runtime binding. Reserve it as a future backend for:

- temporal scheduling;
- batch placement;
- fleet/resource planning;
- capacity planning.

CP-SAT works over integers, which is compatible with our planned fixed-point metric representation.

## 9. PubGrub: highly useful, but only for versions/dependencies

PubGrub is specialized for version solving. Its strengths are:

- transitive dependency resolution;
- conflict-driven solving;
- human-readable derivation trees explaining incompatibilities;
- a mature Rust implementation.

This makes it an excellent fit for:

```text
Workspace Recipe dependencies
Capability Capsule versions
adapter/plugin dependency versions
SDK/runtime compatibility
```

It is not a general replacement for the runtime RequirementGraph solver because it does not naturally express placement, latency, energy, privacy and arbitrary resource relations.

### Architectural lesson from PubGrub

Its **derivation-tree explanation style** is worth copying.

Our resolver should not expose raw SMT formulas. We should build a domain explanation tree from named constraints and incompatibilities.

## 10. SAT / MaxSAT directly

SAT and MaxSAT are excellent foundations and underlie several relevant solvers. Direct SAT encoding would give performance and control, but we would have to encode arithmetic, finite-domain attributes and other theories manually.

Because our early contract uses arithmetic and typed attributes extensively, direct SAT would increase implementation complexity without a clear benefit.

**Decision:** use SMT/MaxSMT first; revisit specialized SAT encodings only after profiling identifies a bottleneck.

## 11. Our Constraint IR — never expose raw SMT to AI or plugins

Neither LLMs nor Workspace/Capsule authors should generate arbitrary SMT-LIB.

The OS owns a small typed Constraint IR.

Example:

```text
ConstraintExpr
  Eq(lhs, rhs)
  Ne(lhs, rhs)
  Lt(lhs, rhs)
  Le(lhs, rhs)
  Gt(lhs, rhs)
  Ge(lhs, rhs)
  In(value, finite_set)
  And([...])
  Or([...])
  Not(expr)
  Implies(a, b)
  ExactlyOne([...])
  AtMost(n, [...])
```

Domain predicates compile into this IR:

```text
HasCapability(node, cap)
TrustAtLeast(node, level)
DataResidency(object, zone)
EffectAllowed(capability, effect)
TypeCompatible(output, input)
VersionCompatible(implementation, runtime)
```

The IR compiler targets Z3 initially and future backends later.

### Deliberate restriction

The runtime binding DSL should stay within a predictable decidable/tractable subset wherever possible:

- booleans;
- finite enums/sets;
- bounded integers;
- fixed-point integers for cost/latency/energy/quality;
- equality on stable IDs;
- explicit finite graph candidates.

Avoid arbitrary nonlinear arithmetic, unrestricted quantification or raw string reasoning in the runtime path.

This makes `unknown` far less likely and keeps explanations understandable.

## 12. Never use floating point in binding semantics

Metrics should use normalized integer units:

```text
latency_us
energy_uj
cost_microcurrency
quality_ppm
memory_bytes
bandwidth_bps
```

Reasons:

- deterministic comparisons;
- stable serialization;
- easier backend portability;
- no platform floating-point surprises;
- straightforward CP-SAT compatibility later.

## 13. Objective policy

Hard policy is never converted into an optimization objective.

For example, confidentiality is not something the solver can trade for speed unless the user's explicit policy says so.

Resolution has four classes:

```text
POLICY
  non-negotiable authorization/trust boundary

HARD
  task correctness requirements

PREFERENCE
  desirable Boolean/finite properties

OBJECTIVE
  measurable values to rank
```

### Automatic mode

Automatic execution should produce one deterministic binding using an **explicit lexicographic objective profile**.

Example:

```text
1. satisfy all policy + hard constraints
2. maximize required quality class
3. minimize external cost
4. minimize latency
5. minimize energy
6. minimize rebind churn
7. stable lexical tie-break by implementation/node IDs
```

The exact priority profile is Workspace/Task-policy controlled.

### Compare mode

When the user asks to compare alternatives, the solver may expose a Pareto frontier such as:

```text
A: fastest
B: lowest energy
C: highest quality
```

Do not use an unspecified Pareto choice for autonomous execution.

## 14. Stability is an objective: avoid pointless rebinding

Because our Fabric changes continuously, a theoretically better node may appear every few seconds.

Without hysteresis, the system could thrash.

Add a `rebind_churn` cost and a minimum-benefit threshold:

```text
keep current binding unless
  hard constraint becomes invalid
  OR expected improvement >= policy threshold
```

This complements `BindingLease`.

## 15. ResolutionTrace and explanations

Every solve should emit a first-class `ResolutionTrace` independent of the backend.

```text
ResolutionTrace {
  requirement_graph_hash,
  normalized_constraints,
  derived_candidates,
  rejected_candidates: [CandidateRejection],
  solver_backend,
  solver_status,
  unsat_core?,
  selected_binding?,
  objective_vector?,
  tie_break?,
  verification_result,
}
```

### Explanations have three sources

1. **Derivation provenance** — why a candidate/converter path exists.
2. **Constraint rejection reasons** — why candidates were eliminated.
3. **SMT unsat core / model** — why no combination works or what combination was selected.

These are converted into a domain explanation tree inspired by PubGrub:

```text
Because the document is Confidential,
  execution must remain inside the Personal Fabric.
Because quality >= 0.95 requires model M,
  a node with >= 20 GiB VRAM is required.
Because no trusted local node has >= 20 GiB VRAM,
  no valid binding exists.
```

The trace becomes input to the Temporal/Causal Graph for important decisions.

## 16. Dynamic/incremental resolution

Alfred continuously changes facts:

```text
node online/offline
battery
network quality
thermal state
new capability
new capsule version
new trust state
```

The resolver architecture should support incremental evolution, but MVP-0 should remain simple.

### MVP

Build a fresh small problem and solve it deterministically.

### Later

```text
Fabric facts
   |
 incremental derivation engine
   |
 changed candidate set
   |
 incremental solver assumptions/scopes
   |
 re-evaluate binding only when needed
```

Z3 supports scoped/incremental solving; Datalog-style engines can maintain recursive derived relations efficiently.

## 17. Timeout / unknown policy

Solver failure must be a defined OS state.

```text
SAT      -> verify -> possible execution
UNSAT    -> explain / request changed constraints
UNKNOWN  -> no new privileged binding
TIMEOUT  -> no new privileged binding
```

For `UNKNOWN` or timeout:

- keep an existing valid BindingLease if policy permits;
- use a prevalidated fallback binding if one exists;
- otherwise ask/degrade/fail closed.

Never interpret timeout as permission to relax policy.

## 18. Proposed Rust interfaces

```rust
trait CandidateDeriver {
    fn derive(&self, graph: &RequirementGraph, facts: &WorldFacts)
        -> CandidateGraph;
}

trait SolverBackend {
    fn solve(&self, problem: &CompiledResolutionProblem)
        -> SolverResult;
}

trait BindingVerifier {
    fn verify(
        &self,
        graph: &RequirementGraph,
        facts: &WorldFacts,
        plan: &BindingPlan,
    ) -> VerificationResult;
}
```

The backend boundary is deliberate. `BindingVerifier` must not call the SMT solver to re-check the same claim; it evaluates the selected finite plan directly against the canonical Rust rules and policy engine.

## 19. MVP-0 decision

MVP-0 should **not** embed Datalog, CP-SAT or PubGrub yet.

Implement:

```text
RequirementGraph with one capability role
2-3 candidate implementations
2 candidate nodes
hard constraints
one soft preference
one numeric objective
stable deterministic tie-break
Z3 backend
independent Rust verifier
ResolutionTrace
```

Test cases:

1. one obvious valid candidate;
2. two valid candidates, objective selects one;
3. policy rejects fastest candidate;
4. no solution -> explanation from named constraints;
5. malicious/incorrect fabricated BindingPlan -> verifier rejects;
6. equal objective values -> stable tie-break;
7. solver timeout/unknown simulation -> fail closed / preserve valid lease.

## 20. Architectural conclusion

The deterministic core should not be "the Z3 solver".

It is a **resolution architecture**:

```text
semantic intent
   -> typed RequirementGraph
   -> fact derivation
   -> candidate pruning
   -> constraint compilation
   -> solver proposal
   -> independent verification
   -> explainable BindingPlan
   -> BindingLease
```

Z3 is the first backend, not the ontology of the OS.

This preserves the Plan B insight — jointly solve the whole relationship between resources — while adding modern typed constraints, optimization, explainability, policy separation and a trusted verification boundary.

## 21. Sources consulted

- Z3 Guide — Optimization introduction: https://microsoft.github.io/z3guide/docs/optimization/intro/
- Z3 Guide — Combining objectives: https://microsoft.github.io/z3guide/docs/optimization/combiningobjectives/
- Z3 Guide — Arithmetic optimization and soft constraints: https://microsoft.github.io/z3guide/docs/optimization/arithmeticaloptimization/
- Z3 Guide — Fixedpoints / Datalog: https://microsoft.github.io/z3guide/docs/fixedpoints/intro/
- Z3 Guide — Basic Datalog and derivations: https://microsoft.github.io/z3guide/docs/fixedpoints/basicdatalog/
- Z3 Guide — scoped/incremental solving: https://microsoft.github.io/z3guide/docs/logic/basiccommands/
- cvc5 documentation — models, incremental solving, unsat cores: https://cvc5.github.io/docs/latest/binary/quickstart.html
- cvc5 tutorial — proofs and unsat cores: https://cvc5.github.io/tutorials/beginners/outputs.html
- Google OR-Tools — constraint optimization / CP-SAT: https://developers.google.com/optimization/cp
- PubGrub Rust: https://github.com/pubgrub-rs/pubgrub
- PubGrub crate documentation: https://docs.rs/pubgrub/latest/pubgrub/
- Ascent Rust Datalog: https://docs.rs/ascent/latest/ascent/
