# ADR 0009 — Rich native semantics, simple compatibility projections

**Status:** Accepted for Architecture v0.3

## Context

Early Plan B explored Boxes as a replacement for files. Later Plan B editions deliberately returned to high-level virtual file interfaces because new abstractions can strand existing general-purpose tools and hurt interoperability.

Our Knowledge Object and Capability models are intentionally richer than files, but the system must remain inspectable, scriptable and interoperable.

## Decision

Use typed semantic contracts as the native model. Also require important system entities to support simple compatibility/introspection projections where practical.

The compatibility projection is not the semantic source of truth and must not constrain the native model to file semantics.

Possible adapters include virtual file trees, CLI/structured text, JSON/CBOR, 9P-like bridges and POSIX exports. The exact set remains open.

## Consequences

- Native components can use rich types without isolating the ecosystem.
- Legacy/general-purpose tools retain escape hatches.
- System state should be inspectable without requiring a proprietary GUI.
- Interface design must avoid leaking secrets through introspection projections.
