# ADR-0011: Use a staged hybrid resolution engine

**Status:** Accepted for prototype architecture v0.4

## Context

The RequirementGraph contains recursive graph facts, hard constraints, soft preferences, numeric objectives, version dependencies and eventually scheduling constraints. No single solver model is ideal for every layer.

## Decision

Use a staged architecture:

1. Datalog-style/graph derivation for candidate closure and recursive relations.
2. SMT/MaxSMT for runtime feasibility and ranking; Z3 is the first backend.
3. PubGrub-style solver for capsule/recipe version dependencies.
4. CP-SAT remains a future backend for global temporal scheduling/placement.

MVP-0 may implement candidate derivation with ordinary Rust graph code rather than embedding Datalog immediately.

## Consequences

- the Capability model is not coupled to Z3 or any one solver;
- each problem class can use a mature algorithm;
- additional interfaces are required between derivation, solving and verification;
- traces must combine provenance from multiple stages.
