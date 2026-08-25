# ADR-0026 — Privileged system actions require scoped authorization

**Status:** Accepted for Linux Pilot v0.1

## Context

The Blob now separates physical-node readiness from operation policy. A machine may be ready for `PreviewActivation` or `TestActivation`, and the semantic operation may correctly require `HostAdministrator`, but neither fact proves that a human or other authorized principal approved this exact experiment.

A generic “administrator approved The Blob” permission would be too broad and reusable across candidates, nodes, readiness states and effects.

## Decision

Every privileged physical system action must be accompanied by a short-lived `SystemOperationAuthorization` receipt bound to:

- exact `SystemOperationId`;
- exact candidate;
- exact `SystemSpec`;
- exact physical `NodeId`;
- exact `SystemCandidateAction`;
- required authority class;
- the exact `PhysicalTestNodeReadiness.observed_at_unix_ms` snapshot reviewed before approval;
- `SingleUse` use policy;
- identified grantor and human-readable reason;
- issue time and expiry time.

Validation receives the current time and the actual readiness object explicitly. It rejects mismatched readiness snapshots, mismatched operations/nodes/actions/candidates, approval issued before the reviewed readiness observation, and future/expired receipts.

An authorization for `PreviewActivation` is not valid for `TestActivation`. A receipt for one node/candidate/SystemSpec/readiness snapshot cannot be replayed on another.

## Single-use semantics

The semantic receipt declares `SingleUse`. The future privileged executor/authorization ledger must atomically consume the receipt ID before or as the privileged action begins, so a second execution attempt using the same receipt is rejected even if the expiry window has not elapsed.

The pure core validator cannot by itself know whether a receipt has previously been consumed; consumption state therefore belongs to the authority/execution boundary, not to the immutable receipt object.

## Separation of concerns

```text
Operation policy
  says which authority class is required

Physical readiness
  says whether the current node state is safe enough

Authorization receipt
  says whether an authorized principal approved
  this exact action against this exact readiness snapshot

Authorization ledger
  enforces single use / replay prevention

Executor
  may act only when all required gates pass
```

No one of these layers substitutes for another.

## Non-privileged actions

The v0.1 receipt is intentionally for `HostAdministrator` operations. `Materialize` and `BuildIsolatedVm` remain normal-user operations and must not require or accept an administrator receipt as a shortcut around their existing bounded executor rules.

## Security properties

- no ambient reusable “root permission for The Blob” token;
- no action escalation from preview to temporary live activation;
- no cross-node/cross-candidate replay;
- a fresh node probe invalidates an authorization for the previous readiness snapshot;
- semantic `SingleUse` requires replay prevention at the executor/authority boundary;
- authorization provenance is recordable in causal history;
- expiry limits the time in which a reviewed preflight can be acted upon;
- AI output cannot mint a valid receipt merely by recommending an action.

## Non-goals v0.1

This ADR does not yet define:

- how the host OS authenticates/elevates the user (polkit, sudo, platform UI, etc.);
- cryptographic signing/storage of receipts;
- the durable implementation of the single-use authorization ledger;
- persistent `boot`/`switch` authorization;
- unattended autonomous live activation;
- delegation between different humans/admins.

Those mechanisms can evolve without weakening the semantic scope of the receipt.

## Next step

The future privileged executor for `PreviewActivation`/`TestActivation` must require:

1. valid operation policy;
2. a green deterministic physical-node readiness result;
3. a valid scoped `SystemOperationAuthorization` referencing that exact readiness snapshot;
4. an unused receipt ID consumed atomically for the execution attempt;
5. an independently revalidated backend command plan.
