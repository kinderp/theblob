# ADR-0032: Compose root D-Bus authority with exact temporary activation in a disposable VM

## Status

Accepted for validation in the Linux Pilot v0.1 authority path.

## Context

ADR-0029 defined the system-D-Bus/polkit authority model. ADR-0030 proved real `pkcheck --system-bus-name` behavior against a live non-root caller. ADR-0031 then proved that a root-owned D-Bus service can derive the caller exclusively from the incoming unique sender name instead of trusting caller-provided identity.

Separately, the Linux Pilot already has:

- an exact immutable NixOS activation plan bound to a materialized `/nix/store/...` system closure;
- a privileged helper that revalidates node, freshness, action, closure, program and arguments;
- a root-owned exact activation permit issuer bound to a successful OS authorization grant;
- a read/consume-only root permit store;
- a second durable privileged execution ledger;
- proven `dry-activate` and temporary `test` semantics with reboot recovery in a disposable NixOS VM.

The remaining gap before considering any physical-node privileged pilot is composition: a real unprivileged D-Bus caller has not yet traversed the complete sender → polkit → exact permit → root boundary → immutable activation path.

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
8. use `LocalNixOsActivationHost`, `StdPrivilegedCommandRunner` and the durable file execution ledger;
9. execute only the candidate closure's exact `switch-to-configuration dry-activate` or `switch-to-configuration test`.

The VM provisions both authority directories as root-owned mode-0700 paths. No unprivileged process can mint a permit or write the privileged execution ledger.

## Required proof

The KVM test must prove all of the following:

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

If this checkpoint is green, the next design step may move from a test-only root D-Bus service toward the production request-loading/daemon boundary, while keeping physical hardware disabled until the service protocol, prepared-request provenance, crash behavior and recovery rules have been reviewed and validated independently.
