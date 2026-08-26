# ADR-0039: Materialization begin is a durable asynchronous operation

## Status

Accepted for validation in a disposable NixOS VM. Physical-node execution remains disabled.

## Context

ADR-0038 proved a semantic front door from canonical `SystemSpec` to a trusted immutable candidate manifest. During that proof, real NixOS derivation resolution inside ADR-0037 `Begin(manifest_id)` exceeded both the normal D-Bus client timeout and an explicit 120-second timeout.

The authority boundary itself was correct; the transport shape was not. A long Nix evaluation must not keep an interactive RPC open, and retry after transport failure must not allocate a second materialization operation.

## Decision

Introduce a root-owned durable materialization-begin queue and a single systemd-owned worker.

The public daemon surface is:

```text
EnqueueBegin(manifest_id)
  -> (request_id, operation_id)

GetBeginStatus(request_id)
  -> (state, operation_id, evidence)
```

`EnqueueBegin` performs no Nix derivation resolution. Root first validates that the trusted manifest exists, then generates and durably persists both a high-entropy request id and the future materialization operation id.

The job state machine is monotonic:

```text
queued -> running -> completed
                  -> failed
```

`completed` means that materialization **begin** completed and the exact pending materialization intent exists. Candidate realization is a later lifecycle stage.

## Caller identity and status privacy

The D-Bus service derives the unique sender from the message header and asks the system bus for that sender's Unix UID. UID is never accepted as a method argument.

The enqueue record stores both original sender and UID. Status access is authorized by UID rather than the ephemeral unique bus name so a later connection by the same local user can inspect its job. Another local UID receives a fail-closed status error even if the random request id is known.

## Durable identity before long work

The central invariant is:

> request id and materialization operation id are durable before Nix evaluation begins.

Therefore a retry never asks root to invent a new operation for an ambiguous old attempt.

The queue record binds:

- request id;
- requester UID;
- original system-bus sender;
- trusted manifest id;
- preallocated materialization operation id;
- enqueue timestamp.

Records are canonical root-owned mode-0600 files below root-owned mode-0700 state directories.

## Claim and recovery

A single systemd-owned worker atomically renames `queued -> running` before executing long work. A second worker cannot claim the same file by observing the same queued path after the rename.

The service uses systemd `KillMode=control-group`. On startup, after the previous service control group is gone, any stranded `running` records are moved back to `queued`.

The worker then calls a recoverable root coordinator using the operation id already stored in the job.

Recovery cases:

1. **Crash before derivation GC-root creation**: retry resolves the trusted manifest and uses the same operation id.
2. **Crash after GC-root creation but before pending intent**: retry observes/reuses the same operation-specific GC root, resolves the same `.drv`, and calls begin with the same operation id.
3. **Crash after pending intent creation but before queue completion**: retry loads that exact pending intent, verifies it still matches manifest + operation, and marks the queue job completed without creating another intent.
4. **Deterministic coordinator failure**: the running job moves to `failed` and is terminal.
5. **Completed job followed by daemon restart**: it remains terminal and is not replayed.

Any conflicting state, mismatched manifest/intent identity, mismatched GC-root target, malformed job file or owner mismatch fails closed.

## Authority

This queue starts only `Materialize`, which remains a non-live `User` authority operation. No new polkit elevation is introduced.

Preview/test activation continues through the independent authorization, permit, replay-ledger and privileged execution boundaries already validated in earlier checkpoints.

## Required VM proof

The KVM test must prove:

1. a valid ADR-0038 trusted manifest can be enqueued through D-Bus under the normal client timeout;
2. the enqueue result returns root-generated request and operation ids before long Nix evaluation finishes;
3. job files are root-owned mode 0600 and unreadable by the requester;
4. another local UID cannot inspect the request;
5. the worker claims the job as `running`;
6. after the exact operation derivation GC root appears, killing the whole service control group strands the running request without creating a terminal result;
7. systemd restarts the daemon and startup recovery requeues/reclaims the same request;
8. request and operation ids are unchanged across the crash;
9. recovered execution eventually reaches `completed`;
10. exactly one pending materialization intent and one exact derivation GC root exist for that operation;
11. queued/running/failed are empty and completed contains exactly one job;
12. restarting the daemon after completion does not replay or mutate the completed job;
13. an unknown request id fails closed.

## Safety boundary

This checkpoint still does not permit:

- caller-selected source/installable/derivation/output/operation id;
- raw Nix or shell canonical input;
- live activation;
- persistent `switch` or `boot`;
- mutable-source `nixos-rebuild`;
- physical-node execution.

## Consequence

Long Nix evaluation is now modeled as durable work rather than an RPC lifetime:

```text
manifest id
   |
   v
EnqueueBegin
   |
   +--> durable request + operation ids
   |
   v
queued -> running -- crash --> queued -> running
                                      |
                                      v
                                  completed
                                      |
                                      v
                          exact pending materialization intent
```

After this proof is green, the next Linux Pilot work is candidate/source and begin-job lifecycle (retention, cancellation/expiry, quotas, orphan reconciliation), persistent causal-log linkage, and trusted node/hardware profile derivation before physical-node activation is reconsidered.
