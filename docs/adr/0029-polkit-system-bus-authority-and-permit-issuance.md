# ADR-0029: Polkit system-bus authority and exact permit issuance

## Status

Accepted for the Linux Pilot authority path.

## Context

ADR-0028 requires an exact root-owned activation permit before the intended privileged boundary can invoke the NixOS mechanism. Production permit minting was intentionally omitted until an operating-system authorization mechanism was chosen.

A custom Unix-socket authorization protocol would require us to invent and audit process identity handling. Polkit already defines process and system-bus subjects and provides user-interactive authorization. Its `pkcheck` interface supports `--system-bus-name`; its own documentation warns that PID-only process subjects are racy and recommends complete trusted identity data when process subjects are unavoidable.

The Blob will already need a local IPC boundary. Using a unique system D-Bus sender name gives the authorization layer a native polkit subject and avoids creating a separate peer-credential identity scheme merely for authorization.

References:

- https://polkit.pages.freedesktop.org/polkit/pkcheck.1.html
- https://polkit.pages.freedesktop.org/polkit/PolkitAuthority.html

## Decision

Add `blob-nix-nixos-authority` with two responsibilities only:

1. plan and interpret a fixed `pkcheck` authorization request for a unique system-bus subject;
2. after a successful OS authorization, issue the exact root-owned permit defined by ADR-0028.

### Polkit subject and actions

The supported action IDs are:

- `org.theblob.nixos.preview-activation`;
- `org.theblob.nixos.test-activation`.

Materialization and isolated-VM build do not cross this authority boundary. Persistent `switch` and `boot` remain unsupported.

The checker accepts only D-Bus unique names (for example `:1.42`), not well-known service names and not a caller-supplied UID or PID. `pkcheck` is invoked directly, never through a shell, with:

- exact action ID;
- exact unique system-bus name;
- `--allow-user-interaction`;
- non-authoritative details for node, candidate, SystemSpec, action and immutable closure.

`--allow-user-interaction` is used only through the explicitly user-initiated API. A future daemon must perform the potentially blocking authorization check on a worker rather than its dispatch thread.

No internal textual polkit agent is enabled automatically. If no suitable authentication agent exists, authorization fails closed.

### Exact authorization binding

A successful polkit check does not grant a reusable boolean "may test NixOS" capability.

The resulting `OsAuthorizationGrant` has no public constructor and carries:

- the authorized unique system-bus subject;
- the exact polkit action;
- the canonical ADR-0028 permit text that was presented for authorization;
- the authorization-check timestamp.

The root-owned permit issuer validates the `PreparedPrivilegedActivation` again and refuses issuance unless the grant's canonical permit is byte-for-byte identical to the permit derived from the request being issued. Therefore an authorization obtained for candidate A cannot be rebound after authentication to candidate B, another closure, another action, another node, or changed timestamps.

The issuer also requires:

- grant timestamp not before preparation and not in the future;
- bounded grant freshness;
- safe root-owned permit directory;
- create-new semantics for the permit file;
- mode `0600` and expected privileged owner;
- fsync of the permit and directory before reporting success.

Existing permits are never overwritten.

### IPC sequencing

The intended future IPC flow is one privileged transaction:

`receive exact prepared request -> validate -> polkit authorize sender -> issue exact permit -> execute through root boundary`

The client should not receive a general reusable privilege token. Keeping authorize, issue and execute in one root-side flow minimizes the interval in which a permit exists without being consumed.

## Consequences

A successful OS authentication is bound to the exact immutable operation the user initiated, rather than just to an action category.

The authority layer remains testable without running polkit by injecting only the command runner; production code uses the fixed `pkcheck` plan.

This ADR still does not install a D-Bus service, a root daemon, a policy file, or any physical-node execution. The next checkpoint must define the D-Bus wire contract, install the service and polkit policy in a disposable NixOS VM, and prove both deny and authorized paths there before physical hardware is considered.
