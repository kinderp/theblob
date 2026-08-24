# ADR 0006 — Separate AI interpretation from deterministic resolution

**Status:** Accepted for Architecture v0.2

## Context

Intent and Situation interpretation benefit from AI because user meaning and real-world context are ambiguous. Capability selection, permissions and privileged execution require repeatable and auditable guarantees.

Plan B's constraint-based resource selection reinforces the value of separating semantic understanding from valid resource binding.

## Decision

AI may produce structured `CapabilityRequirement`, `Situation` and `PlanCandidate` objects. A deterministic typed constraint solver and Policy Engine must validate types, constraints, permissions, effects, trust and placement before execution.

The AI cannot authorize an otherwise invalid binding.

## Consequences

- Core requirement/resolution structures must be typed and serializable.
- Resolver failures should be structured and explainable.
- AI can rank/explain valid solutions but cannot bypass policy.
- MVP-0 should include a tiny deterministic resolver rather than hard-code one executable command.
