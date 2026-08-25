# ADR-0031: Root D-Bus IPC authority must be proven before activation IPC

## Status

Accepted for the Linux Pilot v0.1 authority path.

## Context

ADR-0029 bound polkit authorization to a caller's live unique system D-Bus name and ADR-0030 proved that subject model against real NixOS, system D-Bus and polkit in a disposable VM. The remaining unproven boundary was the root service itself: a privileged service must derive caller identity from the incoming D-Bus message rather than trust a sender token supplied by an unprivileged client.

Installing the full activation method at the same time would combine two authority expansions: privileged IPC and live NixOS mutation. The project advances those separately.

## Decision

Add a disposable NixOS VM proof containing a root-owned system D-Bus service at:

- bus name: `org.theblob.NixOsRoot`;
- object path: `/org/theblob/NixOsRoot`;
- interface: `org.theblob.NixOsRoot1`.

For this checkpoint the service exposes only two authorization probes with no input arguments:

- `PreviewAuthorized() -> (bool, observed_sender)`;
- `TestAuthorized() -> (bool, observed_sender)`.

The service obtains `observed_sender` only from the D-Bus method-call metadata supplied by the system bus. There is no caller-controlled sender field in the method signature. It maps the two methods to the fixed Blob polkit action IDs and invokes `pkcheck --system-bus-name <observed_sender> --allow-user-interaction`.

The VM's test-only polkit rule gives `alice` a deterministic YES for preview and NO for temporary test activation. This avoids an authentication agent while still exercising the real root service, real system bus and real polkit daemon.

## Required proof

The KVM test must prove that:

1. the service process runs as UID 0 and owns the expected system-bus name;
2. an unprivileged caller cannot add a fake sender argument because the D-Bus signature has no input fields;
3. a real `alice` preview call is authorized;
4. a real `alice` test-activation call is denied;
5. the service observes D-Bus-assigned unique `:1.x` sender names;
6. separate short-lived caller connections receive distinct unique names;
7. an unknown method fails closed.

## Safety boundary

This checkpoint does **not** issue activation permits, consume permits, execute `switch-to-configuration`, or expose `switch`/`boot`. It is an IPC/identity proof only.

The next authority-expanding checkpoint may compose this root D-Bus identity boundary with the already-tested exact permit issuer and root activation boundary. That composition must first run in the disposable VM. Physical hardware remains out of scope until the composed path is green.

## Consequences

The production service design must never accept a caller-provided D-Bus unique name as authorization identity. The live message sender is the source of truth. Any future activation method must retain fixed action mapping, exact permit binding, single-use replay barriers and immutable closure activation established by the preceding ADRs.
