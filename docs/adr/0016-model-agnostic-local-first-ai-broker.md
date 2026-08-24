# ADR-0016: AI reasoning is model-agnostic and local-first with optional cloud fallback

**Status:** accepted for research architecture v0.5.

## Decision

The System Technician is a logical OS role, not a particular model. AI reasoning capabilities are resolved through an AI Broker across resident-local, stronger local, trusted Fabric and explicitly permitted cloud implementations.

The routing decision respects privacy/data residency, quality, latency, cost, energy and hardware constraints. Cloud reasoning returns proposals only and receives minimal policy-approved Projections.

## Rationale

Requiring a frontier-scale local model would exclude older/low-memory hardware and couple The Blob to a rapidly obsolete model generation. Requiring cloud would undermine privacy and offline operation.

## Consequences

- The Blob must degrade gracefully without AI.
- High-end local models are optional Capability implementations.
- The same Technician identity can transparently change model bindings.
- Cloud providers never become an authority boundary.
