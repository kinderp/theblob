# ADR-0014: Autonomous binding uses explicit deterministic objective profiles

**Status:** Accepted for prototype architecture v0.4

## Context

Multiple valid bindings can trade latency, quality, energy, cost and migration churn. A Pareto frontier does not define one autonomous choice.

## Decision

Autonomous resolution uses a versioned lexicographic objective profile with stable final tie-break keys. Policy and hard constraints are resolved before optimization and are never silently traded for performance.

Pareto results may be exposed in interactive compare/exploration mode.

## Consequences

- reproducible bindings for identical inputs;
- explanation can state exactly which objective decided the result;
- Workspace/Task profiles may express different priorities without changing the resolver.
