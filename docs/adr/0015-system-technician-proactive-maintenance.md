# ADR-0015: System Technician is proactive, explainable, and non-authoritative

**Status:** accepted for research architecture v0.5.

## Decision

The Blob includes a persistent System Technician role connected to Alfred. It may proactively diagnose regressions and discover relevant upstream improvements, but it cannot bypass policy, verification, simulation or authorization boundaries.

Update/improvement proposals must be evidence-backed, provenance-aware, explain applicability to the local Personal World, include verification/rollback plans and expose official upstream references where available.

## Rationale

The project aims to retain Arch/Gentoo-level freedom while removing their expert-only operational burden. A merely reactive chatbot does not achieve this; an unbounded autonomous administrator would be unsafe.

## Consequences

- Alfred/Situations become inputs to maintenance reasoning.
- external information is evidence, not authority;
- ImprovementProposal becomes a first-class typed object;
- preparation/testing can have broader autonomy than activation;
- important outcomes enter the Temporal/Causal Graph.
