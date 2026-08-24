# ADR 0010 — Projections and derived Representations are first-class

**Status:** Accepted for Architecture v0.3

## Context

The Box `select` operation allowed type-dependent semantic selection over an object. Box conversion/sharing also expressed generated views that could be regenerated from source data.

Our Knowledge Object model already separates object identity from PDF/DOCX/audio/etc. representations, but lacks a precise primitive for partial semantic access and dependency invalidation.

## Decision

Introduce `Projection` as a typed selection over a Knowledge Object/resource. Capability grants should prefer narrow Projections over whole-object access.

Representations are derived dependency nodes referencing source commits, transformation contracts and parameters. Materializations may be absent/fresh/stale and are rebuildable.

## Consequences

- Least-privilege data access becomes finer-grained.
- Derived formats behave more like build artifacts than duplicated source files.
- Temporal/Causal Graph can explain how a representation was produced.
- Dependency cycles, lossy transformations and consistency require explicit policy.
