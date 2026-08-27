# ADR-0041: Candidate source reclamation requires enqueue quiescence

## Status

Validated in a disposable NixOS KVM VM. Physical-node execution remains disabled.

## Context

ADR-0040 deliberately retained trusted-candidate source GC roots after retiring candidate selection state. A scan of durable begin jobs cannot prove that a source is unused: an enqueue may already have loaded the manifest while not yet having published its root-owned begin job.

Deleting the source GC root in that window creates a use-after-GC race:

```text
enqueue loads manifest
          |
          |          lifecycle sees no new job
          |                    |
          |                    v
          |            source GC root removed
          v
late durable begin job appears
```

Retention forever is safe but unbounded. Reclamation therefore needs a shared cross-process quiescence protocol rather than another state scan.

## Decision

Introduce a reusable root-owned candidate enqueue lease and monotonic retirement barrier.

### Enqueue side

Before candidate selection state may be read, enqueue must:

1. verify no `retiring` or `retired` barrier exists for the manifest id;
2. publish a root-owned mode-0600 active lease with a root-generated high-entropy token;
3. fsync the active-lease directory;
4. recheck the retirement barrier;
5. only after the second check, read the trusted manifest and publish the durable begin job;
6. release the lease after the durable job exists.

The second check is mandatory. It handles a retirement barrier that wins after the first check but before the lease becomes durable.

Lease and barrier records are canonical, versioned and root-owned. Manifest identity is hex encoded, timestamps are parsed as integers, trailing/unknown fields are rejected, and path existence is checked with `symlink_metadata` so dangling symlinks cannot masquerade as missing state.

### Retirement side

Candidate source retirement must:

1. publish and fsync a root-owned `retiring` barrier;
2. require zero matching active leases;
3. invoke the existing ADR-0040 candidate-selection retirement proof;
4. require quiescence again;
5. atomically move `retiring -> retired` and fsync both directories;
6. recover the exact immutable source identity from ADR-0040's durable indexed `source-retained:` evidence;
7. verify the source GC root is still an exact symlink to that source;
8. persist canonical exact source-retirement evidence;
9. remove and fsync the candidate source GC root.

The `retired` marker is a permanent tombstone for that manifest id.

## Race analysis

### Lease wins first

Retirement publishes its barrier but observes the active lease and returns `Busy`. Manifest/receipt/source retention remain intact. When the enqueue finishes or is recovered, retirement can retry.

### Barrier wins first

A new enqueue sees `retiring` immediately and stops before manifest access.

### Barrier races lease publication

An enqueue may pass its first check and create a transient lease after the barrier is published. Its mandatory post-create recheck observes the barrier, removes the transient lease best-effort, and rejects before candidate access. Therefore an empty lease set observed after the barrier is durable is sufficient for source retirement.

### Enqueue crashes before durable job publication

The active lease remains. This is a safe retention leak. It may be removed only at daemon startup after systemd has terminated the old service control group, reusing the exclusive recovery point already established by ADR-0039.

### Retirement crashes

The `retiring` barrier remains durable and continues blocking future enqueue attempts. Retirement resumes monotonically. Once `retired` exists it is never reopened.

### Source root is unexpectedly absent

Absence without exact durable source-retirement evidence is ambiguous and fails closed. A repeated retirement is accepted as already reclaimed only when its exact source-retirement receipt exists and matches the ADR-0040 source identity.

## Validated VM proof

The disposable KVM proof validated the complete lifecycle:

1. a valid trusted candidate was enqueued through the lease-aware path;
2. the begin job was safely cancelled before worker claim so ADR-0040 retirement was otherwise permitted;
3. a separate enqueue process acquired the pre-manifest lease and was SIGKILLed before durable job publication, deliberately leaving an abandoned active lease;
4. source retirement published its barrier, returned `Busy`, and preserved manifest, producer receipt and source GC root;
5. while selection files still existed, a later enqueue was rejected by `Retiring` before candidate access;
6. exclusive startup recovery removed exactly one abandoned pre-publication lease;
7. retirement retry removed selection state, created the permanent `retired` tombstone, persisted exact source-retirement evidence and released the exact candidate source GC root;
8. a repeated retirement returned the idempotent `AlreadyReclaimed` result only with exact durable evidence;
9. a post-retirement enqueue was rejected by `Retired` and created no queued job;
10. a root-owned malformed barrier was rejected as `Malformed`;
11. a dangling symlink barrier was rejected as conflicting owner/type state rather than treated as absence;
12. a real `nix-store --gc` pass completed after the candidate source root left the lifecycle graph.

The general CI matrix on the same implementation SHA also passed Rust core, NixOS evaluation/materialization, Slint renderer and WASM component runtime.

### Validation discoveries

The first dedicated run stopped before VM execution because the NixOS Python test driver rejected one unused import. Only the test lint was corrected.

The second run exercised the intended race successfully through `Busy`, barrier rejection and lease recovery, then exposed a parser defect in the new source-retirement wrapper: `evidence-count=...` was mistakenly treated as indexed `evidence-<n>=...`. The parser was corrected to read `evidence-count` separately and then consume only exact numeric evidence indices. No lease, barrier, retirement or authority rule was weakened.

## Safety boundary

This checkpoint does not:

- cancel running materialization work;
- release admitted system-closure GC roots;
- delete completed intent/admission/prepared activation state;
- alter D-Bus, polkit, permit or activation authority;
- permit live or persistent activation;
- execute on physical hardware.

## Consequence

Candidate source retention can now become bounded without guessing about in-flight enqueue state. The source-reclamation proof is a handoff:

```text
trusted candidate source root
    |
    +-- enqueue lease / barrier quiescence
    |
    +-- ADR-0040 materialization lifecycle
    |
    v
permanent retired tombstone + exact retirement receipt
    |
    v
source GC root released
```

The next architectural checkpoint should link the existing stage-local receipts into a persistent cross-stage causal log and then derive a trusted node/hardware profile before physical-node enablement is reconsidered.
