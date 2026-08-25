# ADR-0028 — Privileged activation requires four independent gates

**Status:** Accepted for Linux Pilot v0.1

## Context

The Blob now has separate models for:

- bounded system operation policy;
- physical-node readiness;
- scoped/single-use authorization;
- immutable NixOS activation planning.

A privileged helper must not collapse those concerns into a single “AI/user said yes” check.

## Decision

Before a privileged `PreviewActivation` or `TestActivation` can be handed to an execution helper, `PrivilegedActivationGate` must successfully compose four independent proofs:

```text
1. SystemCandidateOperation policy is canonical
2. PhysicalTestNodeProfile + fresh Readiness are green
3. SystemOperationAuthorization matches exact operation/node/readiness and is valid now
4. Immutable activation plan derives from the exact materialized NixOS closure
```

Only after all four gates succeed is the authorization ID consumed and a `PreparedPrivilegedActivation` produced.

## Single-use handling

The v0.1 in-memory authorization ledger atomically inserts authorization IDs under a mutex. This is sufficient to prove the single-process replay-prevention semantics in tests. A physical-node product must replace/augment this with a durable authority ledger appropriate to the privileged service boundary.

The receipt is consumed **after** semantic/readiness/plan validation and **before** privileged execution. Failure before consumption leaves the receipt unused; a crash after consumption may waste the receipt but must not make it replayable.

## Prepared object is not execution

`PreparedPrivilegedActivation` contains:

- exact node and readiness timestamp;
- consumed authorization ID;
- exact immutable activation plan;
- readiness evidence;
- authorization evidence.

It contains no generic shell command and performs no process spawning.

The next execution layer may accept only this prepared object, re-check the immutable path/executable on the physical host, and run the fixed action under an explicitly designed privilege boundary.

## Failure ordering

Readiness and authorization failures happen before receipt consumption. Invalid materialized candidate/activation planning also fails before consumption. This makes safe recovery from preflight errors possible without creating ambiguous “maybe authorized” state.

## Persistent activation

Neither this gate nor the immutable planner contains `switch` or `boot`. Persistent activation remains a separate future authority/rollback problem.
