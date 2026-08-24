# ADR 0008 — Resolve complete Requirement Graphs jointly

**Status:** Accepted for Architecture v0.3

## Context

Plan B showed that resources participating in one operation cannot always be selected independently. Compatibility may depend on relationships between multiple resources, data types and placement constraints.

A scalar `CapabilityRequirement` is sufficient for simple operations but becomes awkward for multi-resource Tasks.

## Decision

Introduce `RequirementGraph` as the general resolution problem. A scalar CapabilityRequirement is a convenient one-role specialization.

The deterministic resolver binds roles, converter edges, Fabric resources, data routes and grants jointly and returns a `BindingPlan` plus a structured `ResolutionTrace`.

## Consequences

- Resolver interfaces must not hard-code a one-capability-at-a-time model.
- Local validity does not imply global validity.
- Adapter path search and Fabric placement become parts of the same deterministic problem.
- MVP-0 may use a one-role graph while preserving this data model.
