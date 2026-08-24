# ADR-0013: Define a backend-neutral restricted Constraint IR

**Status:** Accepted for prototype architecture v0.4

## Context

Allowing LLMs, plugins or Workspace Recipes to emit raw SMT would couple domain semantics to a backend and create a large unsafe surface.

## Decision

Define a small typed Constraint IR owned by the OS. Compile it to Z3 initially and future backends later.

Runtime values are booleans, stable IDs, finite enums/sets, bounded integers and fixed-point integer metrics. Avoid arbitrary quantifiers, nonlinear arithmetic and raw string reasoning in the binding hot path.

## Consequences

- stable semantics independent of backend syntax;
- easier verification and explanation;
- more predictable solve behavior;
- some complex future rules require domain-specific compilation rather than direct solver expressions.
