# `blob-nix-nixos-materialization-begin-queue`

This module moves the long-running ADR-0037 materialization begin operation out of an interactive D-Bus request.

The public daemon contract is intentionally small:

```text
EnqueueBegin(manifest_id) -> request_id, operation_id
GetBeginStatus(request_id) -> queued | running | completed | failed
```

Both identifiers are generated and persisted by root **before** Nix evaluation starts. The caller still controls only the trusted manifest id.

Queue records move monotonically through root-owned mode-0700 state directories:

```text
queued -> running -> completed | failed
```

Individual records are root-owned mode 0600. `completed` means materialization **begin** completed: the exact pending materialization intent and derivation GC root exist. It does not mean the candidate output has already been realized.

A systemd-owned single worker claims by atomic rename. On daemon startup, stranded `running` jobs are requeued only after the previous service control group has been terminated. The recoverable coordinator always reuses the operation id already stored in the job. If a crash happened after derivation retention or after the pending intent was created, retry reconciles the same identity instead of creating another materialization operation.

D-Bus status authorization uses the local Unix UID derived from the system bus. The original unique sender is retained as provenance, but a later connection by the same local user can still query its job. Another UID cannot read the status even if it learns the random request id.

No polkit elevation is introduced here: `Materialize` remains a non-live, user-authority operation. Privileged preview/test activation remains behind the independent authorization boundaries already proved elsewhere.
