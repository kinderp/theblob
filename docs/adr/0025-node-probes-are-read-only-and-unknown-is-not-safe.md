# ADR-0025 — Node probes are read-only and unknown is not safe

**Status:** Accepted for Linux Pilot v0.1

## Context

The physical test-node readiness model requires evidence about platform, power, storage and rollback state. Some facts can be gathered automatically; others require enrollment state or physical confirmation.

A tempting implementation would let the System Technician infer missing facts, run privileged diagnostic commands, or treat absent evidence as a reasonable default. That would turn a safety check into a probabilistic guess.

## Decision

The first NixOS physical-node probe will be read-only and unprivileged.

It may:

- read standard files/symlinks;
- inspect `/sys`/other read-only kernel interfaces;
- run fixed, audited non-shell commands needed for observation;
- produce structured evidence and warnings.

It must not:

- invoke `sudo` or request administrator authority;
- modify system profiles/generations;
- install diagnostic packages;
- run generated shell commands;
- turn AI inference into readiness evidence;
- treat an unknown safety fact as satisfied.

Facts such as trusted enrollment, storage health and confirmed physical-console recovery remain explicit confirmations or future deterministic subsystems.

## Consequences

- probe failure degrades to missing/unsafe evidence rather than expanding privilege;
- some nodes may require a manual confirmation before live testing even when technically healthy;
- the same semantic readiness model can later receive evidence from richer platform-specific probes;
- the System Technician can explain missing prerequisites without being the source of truth for them.

## Rationale

The Blob separates **observation, authorization and action**. A probe observes. Policy/readiness validation decides eligibility. A separately authorized executor acts.

This preserves the project invariant:

> AI may explain missing evidence, but it cannot manufacture safety evidence.
