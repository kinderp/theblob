# ADR-0027: Narrow NixOS privileged activation helper

## Status

Accepted for the Linux Pilot safety path.

## Context

The Blob can now materialize an immutable NixOS system closure, validate physical-node readiness, bind a single-use authorization receipt to the reviewed readiness snapshot, prepare an immutable activation plan, and prove in a KVM NixOS test that `dry-activate` is non-persistent while `test` is temporary and reboot restores the boot-default configuration.

The remaining boundary is the process that may actually invoke the reviewed closure with host-administrator authority. A generic root shell, `nixos-rebuild --flake`, or a reusable command runner would reintroduce mutation, replay, TOCTOU, and authority-expansion risks that the earlier gates intentionally removed.

## Decision

Introduce a deliberately narrow backend-specific privileged runtime boundary.

The helper:

- accepts only `PreparedPrivilegedActivation` produced by the activation gate;
- revalidates local node identity, timestamp ordering, authorization expiry, readiness freshness, authority, effect class, immutable `/nix/store` closure shape, exact program, exact argument, and bound evidence;
- permits only `<closure>/bin/switch-to-configuration dry-activate` or `<closure>/bin/switch-to-configuration test`;
- never evaluates a flake, rebuilds a candidate, invokes a shell, or accepts `switch`/`boot`;
- canonicalizes the candidate closure and verifies the exact executable before crossing the execution boundary;
- requires the live system to equal the boot-default closure before activation, rejecting stacked temporary activations;
- consumes the authorization again in a privileged, durable create-once ledger immediately before command execution, so replay remains blocked across helper process restarts;
- executes with a cleared environment and a small explicit root runtime environment;
- verifies postconditions: preview must not change `/run/current-system`; successful test activation must point to the exact approved candidate closure;
- on failed test activation, re-applies the exact pre-operation baseline closure with `test`; rollback failure is fail-closed and requires the already-required local recovery path.

The durable ledger directory is not created by the library. Deployment must provision `/var/lib/theblob/privileged-executions` (or an explicitly configured equivalent) as a non-symlink root-owned directory with no group/other permissions.

## Consequences

The privileged component is intentionally less expressive than the unprivileged orchestration layer. It cannot turn a different recipe into root execution and cannot persist a NixOS generation.

A crash after privileged ledger consumption may waste an authorization. This is intentional: loss of liveness is preferable to replay of authority.

A separate packaging/IPC checkpoint is still required before installing or invoking this helper as root on physical hardware. This ADR does not introduce setuid bits, sudoers rules, a root daemon, an IPC protocol, or any automatic physical-node execution.

Persistent `switch` and `boot` remain out of scope for the Linux Pilot.
