# Capability Contract v0.1 — Draft

**Status:** Draft research contract. Intended to constrain MVP-0; not yet a stable ABI.

This contract incorporates the Plan B Box study. Its central principle is:

> A Task requests typed outcomes and relations. It does not select executables, containers, devices or network paths.

## 1. Core entities

```text
CapabilityContract
CapabilityImplementation
RequirementGraph
BindingPlan
BindingLease
Projection
EffectContract
```

## 2. Capability Contract

Conceptual schema:

```text
CapabilityContract {
  id: CapabilityId,
  contract_version: Version,

  inputs: [Port],
  outputs: [Port],

  effects: EffectContract,
  authority: [AuthorityRequirement],

  properties: {
    deterministic?: bool,
    idempotent?: bool,
    cacheable?: bool,
    reversible?: bool,
  }
}

Port {
  name: string,
  schema: TypeRef,
  cardinality: one | optional | many,
}
```

The contract describes **what** can be done. It contains no container image, executable path or node identity.

## 3. Capability Implementation

```text
CapabilityImplementation {
  id: ImplementationId,
  implements: CapabilityId,
  implementation_version: Version,

  backend: wasm | oci | microvm | native | model | remote | hardware,
  artifact: ArtifactRef,

  supported_types: ...,
  resource_requirements: ...,
  placement_constraints: ...,
  quality_profile: ...,
  trust_evidence: ...,
  signatures: ...,
}
```

Implementations may disappear without destroying user state.

## 4. Constraints

Constraints are divided deliberately:

```text
HardConstraint
Preference
Objective
PolicyConstraint
```

Policy constraints are resolved by the trusted Policy Engine and are never traded away for a better score.

## 5. Requirement Graph

```text
RequirementGraph {
  roles: [RequirementRole],
  relations: [RequirementRelation],
  hard_constraints: [...],
  preferences: [...],
  objectives: [...],
  requested_effects: [...],
}
```

A trivial request may contain one role. Complex requests can express an entire data/resource/capability graph.

Example:

```text
source:Object<Document>
  -> translate:Capability<document.translate>
  -> output:Object<Document>

hard:
  data_residency = local
  output.language = it

objective:
  maximize quality
  then minimize latency
```

## 6. Binding Plan

A resolver returns a complete, auditable candidate:

```text
BindingPlan {
  role_bindings: [...],
  adapters: [...],
  placements: [...],
  data_routes: [...],
  grants: [...],
  expected_effects: [...],
  score: ...,
  validity: valid | invalid,
  explanation: ResolutionTrace,
}
```

An invalid plan is never executable.

The `ResolutionTrace` must make it possible to answer:

```text
why this implementation?
why this node?
why this adapter path?
which candidate was rejected and why?
which policy authorized each effect?
```

## 7. Binding Lease

Execution receives a scoped binding, not permanent ambient access.

```text
BindingLease {
  plan_ref: BindingPlanId,
  valid_until: TimeOrCondition,
  granted_authority: [...],
  safe_rebind: before | checkpoint | idempotent-step | never,
}
```

## 8. Effects

```text
EffectContract {
  reads: [ProjectionPattern],
  writes: [ObjectOrProjectionPattern],
  network: NetworkEffect,
  device_actions: [...],
  external_actions: [...],
  semantics: pure | idempotent | transactional | compensatable | irreversible,
}
```

Effect semantics influence retries, migration, approval, rollback and causal recording.

## 9. Projection

```text
Projection {
  object: ObjectId,
  schema: TypeRef,
  selector: TypedSelector,
  provenance: ...,
}
```

A capability should receive the narrowest Projection required by its contract.

## 10. Adapters / converters

Adapters are ordinary Capability Contracts whose output schema differs from their input schema.

The resolver may compose adapters only if the complete path satisfies:

```text
type compatibility
policy
trust
quality/loss budget
latency
cost
energy
effect constraints
```

AI-synthesized adapters remain future work and must pass the same contract.

## 11. Derived representations

A Representation is modeled as a dependency on source object commits plus a transformation contract:

```text
RepresentationDependency {
  sources: [ObjectCommitRef],
  capability: CapabilityId,
  implementation_policy: ...,
  parameters: ...,
  materialized_artifact?: ArtifactRef,
  state: fresh | stale | absent,
}
```

Source changes mark dependent materializations stale; policy decides lazy/proactive rebuild.

## 12. Compatibility projections

Native typed contracts are not an excuse to create an isolated ecosystem.

Important objects/resources/capabilities should expose generic inspectable projections where sensible. The exact bridge is not frozen; candidates include virtual file trees, CLI/structured text, JSON/CBOR and 9P-like interfaces.

## Resolution semantics

Runtime binding semantics are defined separately in [`RESOLUTION-CONTRACT-v0.1.md`](RESOLUTION-CONTRACT-v0.1.md).

A `CapabilityContract` declares what an implementation can do; the Resolution Contract defines how a Task jointly selects implementations, resources, converter paths and grants under policy.

The first resolver backend is Z3 behind an OS-owned Constraint IR. Solver output is only a proposal and must pass an independent Rust verifier before a `BindingLease` can be issued.

## 13. MVP-0 subset

MVP-0 implements only:

```text
CapabilityContract
CapabilityImplementation
one-role RequirementGraph
HardConstraint
PolicyConstraint
BindingPlan + ResolutionTrace
short BindingLease
EffectContract
```

No distributed graph solver, converter chain or dynamic migration is required for MVP-0.
