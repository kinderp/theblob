# ADR-0032: Compose root D-Bus authority with exact temporary activation in a disposable VM

## Status

Accepted and validated in the Linux Pilot v0.1 authority path.

## Context

ADR-0029 defined the system-D-Bus/polkit authority model. ADR-0030 proved real `pkcheck --system-bus-name` behavior against a live non-root caller. ADR-0031 then proved that a root-owned D-Bus service can derive the caller exclusively from the incoming unique sender name instead of trusting caller-provided identity.

Separately, the Linux Pilot already has:

- an exact immutable NixOS activation plan bound to a materialized `/nix/store/...` system closure;
- a privileged helper that revalidates node, freshness, action, closure, program and arguments;
- a root-owned exact activation permit issuer bound to a successful OS authorization grant;
- a read/consume-only root permit store;
- a second durable privileged execution ledger;
- proven `dry-activate` and temporary `test` semantics with reboot recovery in a disposable NixOS VM.

The remaining gap before considering any physical-node privileged pilot was composition: a real unprivileged D-Bus caller had not yet traversed the complete sender → polkit → exact permit → root boundary → immutable activation path.

## Decision

Add a dedicated disposable NixOS KVM proof that composes those already-reviewed boundaries without adding persistent activation.

The VM installs a root system-D-Bus service at the existing Blob endpoint:

- bus name: `org.theblob.NixOsRoot`;
- object path: `/org/theblob/NixOsRoot`;
- interface: `org.theblob.NixOsRoot1`.

For this checkpoint it exposes two zero-input test methods:

- `Preview()`;
- `Test()`.

The service derives the unique caller name only from the D-Bus message header. The caller cannot supply a sender, candidate closure, executable or argv. The service resolves the preconfigured candidate specialisation to its canonical immutable `/nix/store/...` closure and invokes a small Rust integration harness.

The Rust harness uses the production semantic components rather than reimplementing them:

1. construct the exact prepared activation fixture for the VM candidate;
2. create `PolkitAuthorizationRequest` from the D-Bus-derived unique sender;
3. invoke real `pkcheck` through `PkcheckAuthorizationChecker`;
4. bind the resulting `OsAuthorizationGrant` to `RootOwnedActivationPermitIssuer`;
5. create the exact mode-0600 permit in the root-owned permit directory;
6. enter `RootOwnedNixOsActivationBoundary` with the production file permit store;
7. consume the permit destructively before activation;
8. use `LocalNixOsActivationHost`, the production `StdPrivilegedCommandRunner` semantics and the durable file execution ledger;
9. execute only the candidate closure's exact `switch-to-configuration dry-activate` or `switch-to-configuration test`.

The VM harness wraps the production command runner only to expose already-captured stdout/stderr when a command fails. The wrapper does not alter program, argv, environment, result or activation semantics.

The VM provisions both authority directories as root-owned mode-0700 paths. No unprivileged process can mint a permit or write the privileged execution ledger.

## Required proof

The KVM test proves all of the following:

1. the D-Bus service runs as UID 0 and owns the expected bus name;
2. a real non-root `alice` caller authorized for preview reaches the Rust authority chain;
3. preview consumes the exact root-owned permit, records the durable execution receipt and leaves `/run/current-system` on the baseline closure;
4. a second preview using the same activation authorization is rejected by the privileged replay barrier;
5. `alice` is denied the test action by real polkit before permit issuance or activation;
6. a different real non-root caller, `bob`, authorized for test activates the exact immutable candidate closure;
7. the boot-default profile remains the baseline while the candidate is live;
8. no permit file remains after either successful execution or replay rejection;
9. reboot restores the baseline closure and baseline marker;
10. the durable privileged execution ledger survives reboot and rejects a second attempt to execute the already-consumed test authorization.

## VM and daemon lifecycle constraints discovered by the proof

The end-to-end test exposed three integration properties that are now part of the design rather than hidden fixture assumptions.

### A real boot-default profile is required

`runNixOSTest` boots directly from a store closure and does not necessarily install the conventional `/nix/var/nix/profiles/system` profile. The privileged boundary intentionally requires a canonical boot-default closure, because temporary activation is only allowed when the live system still equals that boot default.

The VM therefore provisions `/nix/var/nix/profiles/system` to the exact immutable closure it actually booted before invoking the boundary. The test asserts this equality before activation and again after reboot. This adapts the disposable test environment to the installed-system invariant; it does not weaken the invariant.

### The authority must survive the activation it supervises

`switch-to-configuration test` may reload systemd, D-Bus and polkit while the request is still in flight. The root authority therefore uses `restartIfChanged = false` for this transaction boundary. A controlled daemon upgrade belongs to a separate handoff or reboot path rather than to the activation request currently being supervised.

Polkit is an ordered but weak service dependency (`Wants=` rather than `Requires=`). If polkit is recycled during a temporary NixOS activation, the already-authorized transaction can finish without killing the Blob authority. New authorization checks still fail closed while polkit is unavailable.

D-Bus remains a required dependency because the authority endpoint cannot function without the system bus.

### `ProtectHome` is incompatible with a full NixOS system activation

The NixOS activation script legitimately updates root/home ownership and user directories. Starting `switch-to-configuration` inside a systemd mount namespace with `ProtectHome=true` makes `/root` and `/home` unavailable or read-only to that child and causes the fixed NixOS activation to fail.

The activation authority therefore must not use `ProtectHome` around the process that launches the exact NixOS activator. This does not broaden what a caller may execute: the security boundary continues to come from the D-Bus-derived identity, real polkit decision, root-owned exact permit, immutable canonical closure, exact fixed executable/argv validation and destructive replay barriers. Other compatible service hardening such as `NoNewPrivileges` and `PrivateTmp` remains enabled in this VM proof.

## Safety boundary

This is still a disposable VM integration checkpoint, not physical-node deployment.

Persistent NixOS activation remains impossible in this path:

- no `switch` action;
- no `boot` action;
- no `nixos-rebuild` from mutable source;
- no shell-selected program or argv;
- no caller-selected system closure.

The test-only D-Bus service and prepared activation fixture are not yet the production daemon/API. Their purpose is to prove that the existing authority and activation components compose correctly under a real system bus, real polkit and real NixOS activation semantics.

## Consequence

With this checkpoint green, the next design step may move from a test-only root D-Bus service toward the production prepared-request loading and daemon boundary. That next checkpoint must explicitly cover request provenance, crash/interruption semantics, daemon handoff or upgrade behavior, recovery and idempotency before physical hardware is enabled.
