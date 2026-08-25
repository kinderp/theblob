# ADR-0033: Root daemon loads claimed prepared activation requests from durable storage

## Status

Accepted for validation in a disposable NixOS VM. Physical-node execution remains disabled.

## Context

ADR-0032 proved the full live path from a real non-root system-D-Bus sender through polkit, exact root-owned permit issuance, the root activation boundary, an immutable NixOS closure, temporary activation, reboot recovery and durable replay rejection.

That proof still constructed `PreparedPrivilegedActivation` inside a test harness. A production daemon must not accept closure, executable, argv or action from the D-Bus caller, and it must define what happens when the daemon crashes between authorization and execution.

## Decision

Introduce a root-owned prepared-request store and a production-shaped daemon boundary.

The D-Bus `Execute` method accepts only an opaque authorization/request id. The daemon derives the caller from the live system-bus sender and loads all execution semantics from a pre-staged root-owned request:

- node;
- readiness timestamp and evidence;
- authorization id and expiry;
- operation/candidate/SystemSpec/materialization ids;
- action and effect class;
- immutable `/nix/store/...` closure;
- exact `switch-to-configuration` program and argv;
- expected effects and rollback semantics.

The request representation is versioned, field-ordered and canonical. Text fields are lowercase hex encoded. Unknown, duplicated, reordered, non-canonical or trailing data fails closed. The store accepts only root-owned mode-0600 regular request files inside root-owned mode-0700 state directories.

The durable request states are:

- `ready`: available for validation and OS authorization;
- `inflight`: claimed and never automatically retried;
- `completed`: terminal successful execution;
- `failed`: terminal handled failure.

## Claim ordering

The daemon uses this order:

1. load the ready request read-only;
2. validate the prepared request semantically and for freshness;
3. derive the polkit action from the loaded request;
4. perform real `pkcheck --system-bus-name` for the live D-Bus sender;
5. create an inflight claim receipt with `O_CREAT|O_EXCL` and fsync it;
6. re-read and compare the exact ready request after the durable claim receipt;
7. atomically rename `ready -> inflight` and fsync both directories;
8. issue the exact root-owned activation permit;
9. enter the existing root activation boundary;
10. move the request to `completed` on success or `failed` on a handled failure.

A denied polkit request is never claimed, so an unauthorized caller cannot spend or strand work.

## Crash and recovery rules

Safety is preferred over automatic liveness.

- Crash before claim: the request may remain ready and can be attempted later only through a fresh caller/polkit flow.
- Crash after the durable claim receipt: the request is ambiguous and remains inflight. Restart does not reopen it.
- Crash after permit issuance or during privileged execution: the request is still inflight and is not automatically retried. The exact permit and privileged execution ledger remain independent replay barriers.
- Crash after privileged ledger consumption: the durable ledger prevents the same authorization from executing again even if higher layers are repaired incorrectly.
- An inflight request requires explicit operator/recovery handling; daemon restart or machine reboot never interprets it as permission to retry.

The KVM checkpoint injects a real daemon crash immediately after claim and before permit issuance. systemd restarts the daemon, but the request remains inflight, no permit exists, no privileged ledger receipt is added and a subsequent explicit `Execute` is rejected.

## Service lifecycle

The daemon:

- runs as root;
- requires the system bus;
- treats polkit as an ordered weak dependency so a polkit recycle cannot kill an in-flight activation transaction;
- uses `restartIfChanged = false` so the temporary NixOS activation it supervises cannot restart the daemon itself;
- uses `Restart=on-failure` so process crashes recover service availability without replaying claimed work;
- retains `NoNewPrivileges` and `PrivateTmp`;
- does not use `ProtectHome`, because the fixed NixOS activation program legitimately needs to update user/home state.

## Safety boundary

This checkpoint still exposes no persistent activation:

- no `switch`;
- no `boot`;
- no mutable-source `nixos-rebuild`;
- no caller-selected closure;
- no caller-selected program;
- no caller-selected argv;
- no physical-node execution.

The request producer/staging handoff is deliberately not declared solved by this ADR. The VM uses a test-only fixture renderer to stage root-owned canonical requests. A later checkpoint must bind production request provenance to the non-privileged control plane without weakening the root loader.

## Required proof

The KVM test must prove:

1. unknown caller-selected request ids cannot inject execution fields;
2. authorized preview consumes a staged request and ends in `completed` while leaving the live system on baseline;
3. completed requests cannot be reopened;
4. crash after claim restarts the daemon but leaves the request inflight, with no permit and no new privileged execution receipt;
5. inflight requests are not automatically or explicitly replayed after restart;
6. polkit denial leaves the live-activation request in `ready`;
7. an authorized second caller can then claim and temporarily activate that same exact request;
8. boot-default remains baseline while the candidate is live;
9. reboot restores baseline and preserves completed/inflight request state plus the privileged replay ledger;
10. neither completed nor ambiguous inflight work executes after reboot.

## Next checkpoint

If this proof is green, design the production prepared-request producer/handoff and provenance contract: how the non-privileged control plane publishes an exact prepared request into the root-owned inbox, how authenticity and causal provenance are bound, and how operator recovery resolves stranded inflight requests. Physical hardware remains unnecessary until that handoff is proven.
