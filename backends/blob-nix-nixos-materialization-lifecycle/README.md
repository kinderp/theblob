# `blob-nix-nixos-materialization-lifecycle`

Fail-closed lifecycle management for the NixOS materialization pipeline.

Cleanup is part of the trust boundary: deleting a manifest, source GC root, derivation GC root, or materialized closure too early can make crash recovery non-deterministic or leave an already-authorized activation pointing at collected state.

The first checkpoint provides two root-side components:

- `RootSafeMaterializationFinalizer`: roots the exact admitted system closure **before** the existing completion boundary may release the derivation GC root.
- `RootMaterializationLifecycleManager`: expires only safely-unclaimed queued work, rejects cancellation after native durable state exists, retires candidate artifacts only after every related begin job is safely terminal, retires terminal jobs only after candidate retirement is durable, and releases stale derivation roots only when an admitted closure root or terminal failure proves recovery no longer needs them.

Production roots include:

```text
/var/lib/theblob/materialization-begin-jobs
/var/lib/theblob/materialization-candidates
/var/lib/theblob/candidate-manifest-receipts
/var/lib/theblob/materialization-intents
/var/lib/theblob/materialization-admissions
/var/lib/theblob/materialization-lifecycle/receipts
/nix/var/nix/gcroots/theblob-candidate-sources
/nix/var/nix/gcroots/theblob-materializations
/nix/var/nix/gcroots/theblob-admitted-closures
```

Key invariants:

1. A pending materialization intent keeps its exact derivation GC root.
2. An admission is lifecycle-valid only when its exact output closure has a matching root-owned GC root and the completed intent agrees with it.
3. A completed begin job is **not** enough to retire a candidate; materialization completion and exact closure retention must already be durable.
4. A failed begin job is reclaimable only when no intent, admission, or operation GC root remains.
5. Running work cannot be cancelled by this module.
6. Queued cancellation/expiry is accepted only before native durable state exists; atomic rename races fail closed if the worker wins.
7. Candidate deletion is ordered manifest -> producer receipt -> source GC root, so a crash can leak retention but cannot deliberately leave a selectable manifest whose source was unrooted.
8. Lifecycle decisions leave deterministic root-owned receipts. Receipt storage is bounded and mutations fail closed when the bound is reached.
9. Unknown orphan roots are retained rather than guessed away.

This checkpoint deliberately does **not** delete materialization admissions, completed intents, admitted output closure roots, or prepared activation records. Their retention boundary depends on the later activation lifecycle and causal-log checkpoint.
