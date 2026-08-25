# ADR-0028: Root-owned exact activation permits

## Status

Accepted for the Linux Pilot privileged-boundary path.

## Context

ADR-0027 introduced a deliberately narrow NixOS activation mechanism: exact immutable closure, only `dry-activate` or `test`, runtime freshness checks, replay protection, live-state postconditions, and rollback behavior. It still accepted a `PreparedPrivilegedActivation` value as input.

That value is structurally validated but is ordinary process memory. An unprivileged or compromised caller can fabricate a structurally plausible value. A replay ledger proves only that an authorization identifier has not already executed; it does not prove that the root side previously admitted the exact request.

No root daemon, setuid program, sudoers rule, or IPC interface exists yet, so this authority-authenticity boundary must be fixed before any such entry point is introduced.

## Decision

Add a separate `blob-nix-nixos-root-boundary` crate. Any future installed root service or privileged IPC endpoint must use this boundary rather than invoking the lower-level NixOS mechanism directly.

The boundary requires a trusted activation permit before delegating to the mechanism. The permit is exact-bound to:

- authorization identifier;
- local node;
- activation operation;
- candidate;
- `SystemSpec`;
- materialization operation;
- semantic action;
- effect class;
- authority class;
- exact immutable Nix store system closure;
- exact executable path;
- exact argument vector;
- readiness observation timestamp;
- prepared timestamp;
- authorization expiry timestamp.

Permit text is versioned and canonical. All caller-controlled strings are hex encoded before serialization, preventing newline/key injection from changing the meaning of the permit.

The production permit store is read/consume only. It intentionally exposes no API that mints a production permit. Issuance belongs to a later OS-authenticated privileged authorization checkpoint.

The default production permit directory is `/var/lib/theblob/activation-permits`. The consumer rejects the store unless:

- the directory is an ordinary non-symlink directory;
- the directory owner is UID 0;
- group and other have no permissions on the directory;
- the permit is an ordinary non-symlink file;
- the permit owner is UID 0;
- the permit mode is exactly `0600`;
- the file content matches the canonical expected permit byte-for-byte.

On a successful match, the permit file is removed and the directory is synced before the lower-level activation mechanism is called. Concurrent consumers therefore have a single destructive winner. A failed or interrupted attempt may waste a permit; permits are never recreated automatically.

The lower-level privileged execution ledger from ADR-0027 remains in place as a second anti-replay barrier. Permit authenticity and execution replay are intentionally independent defenses.

## Consequences

A fabricated `PreparedPrivilegedActivation`, even if internally consistent, cannot cross the intended root boundary without an already-existing root-owned permit for the exact same operation.

Changing candidate identity, closure, action, arguments, node, timestamps, or any other execution-sensitive bound field invalidates the permit.

A mismatched request does not destroy the real permit, allowing the exact approved request to remain usable. A successfully matched permit is single-use and is destroyed before activation begins.

The root boundary is still a library contract. This ADR does not add a root process, IPC transport, polkit integration, sudoers entry, setuid bit, or physical-machine execution.

The next authority-expanding checkpoint must design an OS-authenticated permit issuer and IPC/package boundary, and must test that installed boundary in a disposable NixOS VM before physical-node use.
