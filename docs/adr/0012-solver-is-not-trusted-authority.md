# ADR-0012: Solver output is a proposal, not authority

**Status:** Accepted for prototype architecture v0.4

## Context

External solvers are complex native software and can time out, return unknown, contain bugs or be upgraded independently. The OS must not make privilege safety depend on the correctness of an optimization engine.

## Decision

Every satisfiable solver result is converted to a finite `BindingPlan` and checked by an independent Rust `BindingVerifier` against canonical type, policy, authority, effect and resource rules.

The solver is outside the trusted authorization boundary.

## Consequences

- an incorrect SAT model is rejected;
- an incorrect UNSAT result is an availability problem, not a privilege escalation;
- the verifier must remain substantially simpler than the general solver;
- optimality is not a security claim;
- high-assurance proof/certificate work can be added later without changing the contract.
