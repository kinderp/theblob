# ADR-0026 — Privileged system actions require scoped authorization

**Status:** Accepted for Linux Pilot v0.1

## Context

The Blob now separates physical-node readiness from operation policy. A machine may be ready for `PreviewActivation` or `TestActivation`, and the semantic operation may correctly require `HostAdministrator`, but neither fact proves that a human or other authorized principal approved this exact experiment.

A generic “administrator approved The Blob” permission would be too broad and reusable across candidates, nodes and effects.

## Decision

Every privileged physical system action must be accompanied by a short-lived `SystemOperationAuthorization` receipt bound to:

- exact `SystemOperationId`;
- exact candidate;
- exact `SystemSpec`;
- exact physical `NodeId`;
- exact `SystemCandidateAction`;
- required authority class;
- identified grantor and human-readable reason;
- issue time and expiry time.

Validation receives the current time explicitly and rejects mismatched, future or expired receipts.

An authorization for `PreviewActivation` is not valid for `TestActivation`. A receipt for one node/candidate/SystemSpec cannot be replayed on another.

## Separation of concerns

```text
Operation policy
  says which authority class is required

Physical readiness
  says whether the current node state is safe enough

Authorization receipt
  says whether an authorized principal approved this exact bounded action

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
- authorization provenance is recordable in causal history;
- expiry limits the time in which a previously reviewed preflight can be acted upon;
- AI output cannot mint a valid receipt merely by recommending an action.

## Non-goals v0.1

This ADR does not yet define:

- how the host OS authenticates/elevates the user (polkit, sudo, platform UI, etc.);
- cryptographic signing/storage of receipts;
- persistent `boot`/`switch` authorization;
- unattended autonomous live activation;
- delegation between different humans/admins.

Those mechanisms can evolve without weakening the semantic scope of the receipt.

## Next step

The future privileged executor for `PreviewActivation`/`TestActivation` must require both:

1. a green deterministic physical-node readiness result; and
2. a valid scoped `SystemOperationAuthorization`.

It still must independently revalidate the backend command shape before execution.
