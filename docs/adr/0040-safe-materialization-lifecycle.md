# ADR-0040: Materialization lifecycle is a fail-closed retention protocol

## Status

Validated in a disposable NixOS KVM VM. Physical-node execution remains disabled.

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

## Validated VM proof

The dedicated disposable KVM test validated the new lifecycle boundary with a small hermetic BusyBox materialization oracle. The proof exercised the same root-owned candidate, async-begin, exact derivation and admission contracts without rebuilding a complete NixOS closure merely to test garbage collection.

The VM proved:

1. a trusted root-owned candidate enters durable async begin and produces exactly one pending materialization intent and operation derivation GC root;
2. a begin-completed queue job alone cannot retire its candidate while the materialization intent is still pending;
3. safe finalization before realization fails and creates no admitted-closure root;
4. the exact predicted derivation target can be realized by a normal non-root user;
5. successful safe finalization creates the exact admitted-closure GC root, persists the matching admission/completed intent, and then releases the operation derivation root;
6. an exact leftover derivation root can be reconciled away only when admission, completed intent and admitted closure root agree;
7. an unknown orphan derivation root is reported as `OrphanRetained` and remains untouched;
8. a real `nix-store --gc` after derivation-root release does not collect the admitted output because the new closure root has taken over liveness;
9. candidate selection retirement removes the manifest and producer receipt while both candidate source GC root and admitted closure GC root remain;
10. the terminal begin job cannot be retired before candidate-retirement evidence exists, and can be retired afterwards without deleting admission, completed intent or closure retention;
11. stale queued work with no native durable state can expire;
12. a queued job with an operation GC root cannot expire;
13. explicit cancellation is requester-UID bound and running work is rejected;
14. invalid lifecycle directory mode and a symlink planted at a deterministic receipt path both fail closed without moving the queued job;
15. no live activation and no persistent `switch` or `boot` occurs.

The general Rust, NixOS evaluation, Slint and WASM CI suite was also green on the same implementation SHA used by this first KVM validation.

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
