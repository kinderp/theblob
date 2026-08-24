# ADR 0007 — Capabilities are typed, constrained and late-bound

**Status:** Accepted for Architecture v0.2

## Context

Inferno demonstrates typed dynamically loaded modules. Plan B demonstrates typed resource abstractions, constraints, conversion paths and dynamic environments where resources appear/disappear.

Our Tasks must survive implementation changes and movement between devices.

## Decision

A Task depends on an abstract Capability contract rather than a permanent implementation. Concrete Capsule and Fabric-node bindings should occur as late as practical and may be safely re-resolved when context or resource availability changes.

Converters/adapters are first-class Capability Graph edges with their own trust, quality, cost and effect metadata.

## Consequences

- Workspace state cannot depend on the lifetime of a Capsule.
- Resolver inputs must include runtime Situation and node telemetry.
- Rebinding must be observable and causally recorded when meaningful.
- Implementation identity is not user-data identity.
