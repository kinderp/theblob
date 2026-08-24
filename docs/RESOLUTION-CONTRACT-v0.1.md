# Resolution Contract v0.1 — Draft

**Status:** Draft deterministic-resolution contract derived from Plan B research and solver study.

## 1. Principle

> Semantic AI produces a typed problem. A deterministic resolver proposes a binding. An independent verifier authorizes the concrete plan.

No LLM, plugin or Workspace Recipe can submit raw SMT expressions or directly choose privileged runtime bindings.

## 2. Resolution pipeline

```text
Intent / Situation / Task
        |
        v
RequirementGraph
        |
        v
ConstraintNormalizer
        |
        +---- World/Fabric facts
        |
        v
CandidateDeriver
        |
        v
CandidateGraph
        |
        v
ConstraintCompiler
        |
        v
SolverBackend
        |
        v
SolverProposal
        |
        v
BindingVerifier
        |
        +--- reject
        |
        v
BindingPlan
        |
        v
BindingLease
```

## 3. Constraint classes

```text
PolicyConstraint
HardConstraint
SoftPreference
Objective
```

### PolicyConstraint

Non-negotiable trust/authority/privacy boundary.

### HardConstraint

Required for task correctness.

### SoftPreference

Desirable but tradeable.

### Objective

A measurable score to minimize/maximize.

Policy constraints are never silently demoted to preferences or objectives.

## 4. Constraint IR

Conceptual AST:

```text
ConstraintExpr :=
    True
  | False
  | Eq(ValueExpr, ValueExpr)
  | Ne(ValueExpr, ValueExpr)
  | Lt(ValueExpr, ValueExpr)
  | Le(ValueExpr, ValueExpr)
  | Gt(ValueExpr, ValueExpr)
  | Ge(ValueExpr, ValueExpr)
  | In(ValueExpr, FiniteSet)
  | And([ConstraintExpr])
  | Or([ConstraintExpr])
  | Not(ConstraintExpr)
  | Implies(ConstraintExpr, ConstraintExpr)
  | ExactlyOne([BoolExpr])
  | AtMost(u32, [BoolExpr])
  | AtLeast(u32, [BoolExpr])
```

Domain helpers compile to this AST rather than extending solver semantics arbitrarily.

## 5. Value domain

Runtime binding uses canonical deterministic values:

```text
Bool
StableId
Enum
BoundedInt
FixedPointInt
FiniteSet
```

Metrics use integer units:

```text
latency_us
energy_uj
cost_microcurrency
quality_ppm
memory_bytes
bandwidth_bps
```

No floating-point comparison is part of binding semantics.

## 6. Candidate graph

The candidate graph is finite and explicit for a solve.

```text
CandidateGraph {
  role_candidates,
  converter_paths,
  candidate_relations,
  derived_facts,
  provenance,
}
```

Recursive discovery/reachability can later use Datalog-style evaluation. The SMT backend should generally choose among already-derived finite candidates instead of discovering an unbounded graph itself.

## 7. Solver result

```text
SolverResult :=
  Sat(SolverProposal)
  | Unsat(UnsatEvidence)
  | Unknown(Diagnostic)
  | Timeout(Diagnostic)
  | Error(Diagnostic)
```

`Sat` is not authorization.

## 8. BindingPlan

```text
BindingPlan {
  role_bindings,
  converter_bindings,
  node_bindings,
  data_routes,
  grants,
  expected_effects,
  objective_vector,
  tie_break_key,
}
```

## 9. Independent verification

The verifier checks the concrete finite plan using canonical Rust code and policy state.

It verifies at minimum:

```text
all required roles are bound
selected identities exist
versions/contracts match
input/output types are compatible
converter chain is valid
all policy constraints hold
all hard constraints hold
authority grants are sufficient and no broader than allowed
effects match declared EffectContracts
resource envelopes fit the selected nodes
BindingLease/rebind rules are respected
```

The verifier does not assume solver correctness.

## 10. Optimization profile

Autonomous resolution uses an explicit deterministic lexicographic profile.

Example:

```text
quality_class desc
external_cost asc
latency_us asc
energy_uj asc
rebind_churn asc
implementation_id asc
node_id asc
```

Workspace/Task policy may select another named profile, but ordering must be explicit and versioned.

A Pareto frontier is an exploration/UI feature, not an unspecified autonomous choice rule.

## 11. ResolutionTrace

```text
ResolutionTrace {
  trace_id,
  requirement_graph_hash,
  world_snapshot_ref,
  constraint_ir_version,
  normalized_constraints,
  candidate_provenance,
  rejections,
  backend_name,
  backend_version,
  backend_status,
  unsat_core?,
  solver_proposal?,
  verification,
  selected_binding?,
  objective_vector?,
  deterministic_tie_break?,
}
```

Important traces can be committed into the Temporal/Causal Graph.

## 12. Candidate rejection

```text
CandidateRejection {
  candidate,
  constraint_id,
  category: policy | hard | type | version | resource | effect,
  evidence,
}
```

Prune early where possible so the solver receives a smaller finite problem and explanations are easier to build.

## 13. Unsat explanation

Every constraint entering an SMT backend receives a stable domain-level ID.

Backend unsat cores are mapped back to those IDs and then rendered as a domain derivation tree.

The user should see:

```text
No valid local translation binding exists.

Because:
1. the document is confidential and cannot leave the Personal Fabric;
2. required quality is at least 95%;
3. all local implementations meeting 95% require >= 20 GiB VRAM;
4. no currently trusted local node has >= 20 GiB VRAM.
```

not SMT syntax.

## 14. Rebinding

Binding changes are controlled by `BindingLease` plus an explicit churn cost.

```text
rebind_allowed_at = before | checkpoint | idempotent_step | never
```

A new candidate does not automatically cause migration. Policy may require a minimum improvement threshold.

## 15. Failure behavior

```text
SAT + verified      -> may execute
SAT + verifier fail -> reject + record anomaly
UNSAT               -> explain / request changed constraints
UNKNOWN/TIMEOUT     -> no new privileged binding
```

An existing still-valid lease may continue if policy explicitly allows it.

## 16. SolverBackend abstraction

First backend: Z3.

Future candidates:

```text
cvc5     differential/proof experiments
CP-SAT   temporal scheduling/global placement
SAT      specialized high-volume finite encodings
```

The Constraint IR and verifier must remain backend-independent.

## 17. Specialized version resolution

Capsule and Workspace Recipe version/dependency selection is a separate subproblem and may use PubGrub.

The resulting version assignment becomes a fact/input to the runtime binding problem.

Do not force general placement/resource optimization into the version solver.

## 18. MVP-0 implementation subset

```text
one Task
one capability role
2 implementations
2 nodes
no converter chain
PolicyConstraint
HardConstraint
one SoftPreference
one numeric Objective
Z3 backend
BindingVerifier
ResolutionTrace
```

This is sufficient to prove the architecture without prematurely building a distributed optimizer.
