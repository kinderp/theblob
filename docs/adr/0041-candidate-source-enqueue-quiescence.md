# ADR-0041: Candidate source reclamation requires enqueue quiescence

## Status

Accepted for validation in a disposable NixOS KVM VM. Physical-node execution remains disabled.

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

### Retirement side

Candidate source retirement must:

1. publish and fsync a root-owned `retiring` barrier;
2. require zero matching active leases;
3. invoke the existing ADR-0040 candidate-selection retirement proof;
4. require quiescence again;
5. atomically move `retiring -> retired` and fsync both directories;
6. recover the exact immutable source identity from ADR-0040's durable `source-retained:` evidence;
7. verify the source GC root is still an exact symlink to that source;
8. persist exact source-retirement evidence;
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

## Required VM proof

The disposable KVM test must prove:

1. a valid trusted candidate can be enqueued through the lease-aware path;
2. a safely terminal job exists so ADR-0040 candidate retirement is otherwise permitted;
3. an enqueue process can be killed after publishing its lease but before publishing a durable job;
4. source retirement publishes its barrier, returns `Busy`, and preserves manifest, producer receipt and source GC root;
5. after the barrier is present, a new enqueue is rejected before candidate selection is removed;
6. exclusive startup recovery removes exactly the abandoned pre-publication lease;
7. retry retires selection, creates the permanent retired tombstone and releases the exact source GC root;
8. exact source-retirement evidence exists before/with reclamation;
9. repeated retirement is idempotent only with that evidence;
10. a post-retirement enqueue remains rejected and cannot recreate a job;
11. malformed, symlinked or ownership-conflicting lease/barrier state fails closed;
12. a real Nix garbage-collection pass is safe after the source root leaves the lifecycle graph.

## Safety boundary

This checkpoint does not:

- cancel running materialization work;
- release admitted system-closure GC roots;
- delete completed intent/admission/prepared activation state;
- alter D-Bus, polkit, permit or activation authority;
- permit live or persistent activation;
- execute on physical hardware.

## Consequence

Candidate source retention can become bounded without guessing about in-flight enqueue state. The source-reclamation proof is now a handoff:

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

After this checkpoint is validated, the next architectural work should link the existing stage-local receipts into a persistent cross-stage causal log and then derive a trusted node/hardware profile before physical-node enablement is reconsidered.
