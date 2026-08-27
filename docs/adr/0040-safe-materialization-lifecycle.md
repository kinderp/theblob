# ADR-0040: Materialization lifecycle is a fail-closed retention protocol

## Status

Accepted for validation in a disposable NixOS VM. Physical-node execution remains disabled.

## Context

ADR-0039 made materialization begin durable and asynchronous. That proof intentionally retained job records, trusted candidate manifests, candidate source GC roots and operation-specific derivation GC roots rather than risk premature cleanup.

The next boundary is not ordinary garbage collection. These objects encode recovery authority and exact native identity. Deleting any of them based only on age or queue terminality can break the causal chain.

A second gap appears at successful materialization completion: the current completion boundary verifies the exact output and then releases the operation derivation GC root. The later root request publisher consumes the admitted `system_closure`. Therefore the admitted closure itself must become durably GC-rooted before the derivation root is eligible for release.

## Decision

Introduce a standalone `blob-nix-nixos-materialization-lifecycle` module. The Blob depends on this reusable lifecycle component; the component does not depend on product UI or AI policy.

### Safe completion wrapper

`RootSafeMaterializationFinalizer` performs:

```text
load exact pending intent
  -> root exact expected output closure
  -> verify + persist materialization admission
  -> existing completion boundary releases derivation root
  -> verify closure root still matches admission
```

The closure root is stored under:

```text
/nix/var/nix/gcroots/theblob-admitted-closures/
  operation-<hex(operation)>-closure
```

If completion fails after closure retention, the closure root is intentionally retained. Leaking a GC root is preferable to admitting or recovering from collected state.

### Begin-job lifecycle

Queued work may expire or be cancelled only when all of the following are true:

- the job is still in `queued`;
- the trusted caller UID matches for explicit cancellation;
- no pending or completed materialization intent exists for its preallocated operation;
- no materialization admission exists;
- no operation derivation GC root exists.

The lifecycle manager races the worker only by atomic rename. If the worker wins and the queued file disappears, cancellation/expiry fails closed. Running work is never killed by this API.

### Candidate selection retirement

A trusted candidate is removed from future selection only when every begin job currently referencing that manifest is terminal and reclaimable.

- `completed` begin job: requires no pending intent, an exact completed materialization intent, the matching materialization admission, and the matching admitted-closure GC root.
- `failed` begin job: requires no intent, no admission and no derivation GC root.

This checkpoint deletes only:

```text
trusted manifest -> producer receipt
```

The candidate source GC root is deliberately **retained**.

That narrower contract closes an enqueue/retirement race discovered during implementation. An enqueue may already have loaded a trusted manifest immediately before lifecycle retirement removes it. Without shared synchronization, deleting the source GC root after scanning jobs could let that already-in-flight enqueue publish work after source retention disappeared. Retaining the source root converts that race into a safe availability failure: the late job may fail because the manifest is no longer selectable, but the immutable source cannot become a use-after-GC.

Reclaiming candidate source roots therefore requires a later shared enqueue/retirement quiescence or lease protocol. This checkpoint does not guess at one.

### Terminal begin-job retirement

A terminal begin job is retired only after the related candidate has a valid lifecycle retirement receipt and the trusted manifest is already absent. This prevents deleting the last manifest-to-operation relationship before candidate selection retirement has been decided.

### Derivation GC-root reconciliation

- pending intent -> exact derivation root is required and retained;
- admission -> completed intent and exact admitted closure root are required before a leftover exact derivation root may be released;
- failed job with no native durable state -> an operation root may be released only if it is an exact `.drv` symlink;
- ambiguous/orphan roots -> retained and reported, not guessed away.

### Lifecycle receipts and bounds

Each destructive lifecycle decision creates a deterministic, root-owned mode-0600 receipt under `/var/lib/theblob/materialization-lifecycle/receipts` before destructive steps. The receipt directory has a hard bound in this checkpoint; when the bound is reached, new lifecycle mutations fail closed.

These are decision/authorization receipts, not yet the global causal log. A receipt can therefore precede the filesystem mutation it authorizes; if a racing worker wins or a crash occurs, later code must still re-prove current state before mutation.

## Non-goals in this checkpoint

This checkpoint does not yet:

- reclaim candidate source GC roots;
- define the shared enqueue/retirement quiescence or lease protocol needed for safe source reclamation;
- delete materialization admissions or completed intents;
- release admitted output closure roots after activation;
- delete prepared activation records or privileged execution ledger entries;
- cancel running Nix work;
- auto-delete unknown orphan roots;
- guarantee eventual cleanup of every crash-safe retirement leak;
- solve unused-never-enqueued candidate retention;
- provide the persistent cross-stage global causal log;
- derive trusted physical hardware/node profiles;
- enable physical-node activation.

These omissions are intentional: each requires an exact downstream liveness, concurrency or audit contract before deletion can be proved safe.

## Required VM proof

The disposable KVM proof must show:

1. a trusted root-owned test candidate can enter durable async begin and reach a pending materialization intent;
2. after the non-root exact build realizes the predicted output, safe finalization creates the exact admitted-closure GC root before completion releases the derivation root;
3. the admission, completed intent and closure root match path-for-path;
4. `nix-store --gc` cannot collect the admitted closure while that root exists;
5. lifecycle retirement can remove the now-unused trusted manifest and producer receipt while the candidate source GC root and admitted closure root both remain;
6. the completed begin job can be retired only after candidate retirement evidence exists;
7. a stale queued job with no native durable state can expire safely;
8. a queued job with an operation GC root is not expired;
9. explicit cancellation of a still-queued owned job succeeds, while cancellation of running work is rejected;
10. malformed, symlinked or root-ownership-conflicting lifecycle state fails closed;
11. an unknown orphan derivation root is retained rather than guessed away;
12. no live activation and no persistent `switch`/`boot` occurs.

The trusted-candidate fixture for the GC oracle may use a tiny hermetic BusyBox derivation rather than a complete NixOS closure. ADR-0038 separately proves canonical `SystemSpec` -> trusted immutable candidate production, and ADR-0039 separately proves the public D-Bus async begin boundary. This checkpoint isolates the new retention/lifecycle contract instead of rebuilding a full NixOS system merely to exercise garbage collection.

## Consequence

The materialization pipeline now has an explicit retention handoff:

```text
candidate source root  (retained through this checkpoint)
      |
      v
begin derivation root
      |
      v
exact realized output
      |
      +--> admitted closure root
               |
               v
       later activation lifecycle
```

Cleanup becomes a proof of downstream liveness, not a timer-driven file deletion routine. Source-root reclamation remains blocked until cross-process enqueue quiescence itself is provable.
