# ADR-0030: Real polkit authority proof in a disposable NixOS VM

## Status

Accepted as the mandatory authority checkpoint before installing any Blob privileged IPC service on physical hardware.

## Context

ADR-0029 selected polkit authorization against a caller's unique system D-Bus name and bound a successful OS authorization grant to the exact canonical activation permit. Those contracts were unit/integration tested, but the Linux Pilot still needed evidence that the chosen subject model behaves as expected in a real NixOS userspace with the real D-Bus daemon and polkit daemon.

NixOS enables polkit through `security.polkit.enable`, installs its rules through `security.polkit.extraConfig`, and links `/share/polkit-1` from the system path. Therefore The Blob's action definitions can be packaged as normal Nix store content while the test-only rule remains an explicit NixOS configuration input.

## Decision

Add a dedicated `runNixOSTest` KVM proof named `polkit-authority`, gated in CI to the `feature/nixos-polkit-authority-vm` checkpoint branch.

The disposable VM contains:

- real `polkitd`;
- real system D-Bus;
- a non-root `alice` user;
- The Blob preview/test polkit action definitions;
- root-only `/var/lib/theblob/activation-permits` and `/var/lib/theblob/privileged-executions` directories;
- a non-root `dbus-test-tool black-hole --system` process that keeps a real D-Bus connection alive without requesting a well-known name.

The test discovers the connection's actual D-Bus-assigned unique `:1.x` name by matching the service PID against the system-bus connection list. It never fabricates the unique name.

A test-only polkit rule then gives deterministic outcomes without introducing an authentication agent:

- preview activation for `alice`: YES;
- test activation for `alice`: NO.

The test invokes real `pkcheck --system-bus-name <unique-name> --allow-user-interaction` for both actions and requires the expected allow/deny results.

The test then stops the non-root D-Bus client and proves that the old unique name can no longer be authorized. After restarting the client, the new connection must receive a different unique name and the allow/deny behavior is rechecked against that new live identity.

This proves the D-Bus lifetime property on which ADR-0029 relies: the subject identifier is tied to a live connection and is not reused or manually claimable as a unique name.

## Consequences

Passing this checkpoint validates the real NixOS/D-Bus/polkit substrate for the chosen authority model, including action installation and the root-only state directories used by the later permit and replay boundaries.

It does not test a password dialog or graphical authentication agent. The deterministic YES rule exists only in the disposable VM; production action defaults remain administrative authentication and the production service must fail closed when no suitable agent exists.

It also does not install The Blob's root daemon or D-Bus method contract. The next checkpoint may introduce that IPC service, but it must first be tested in the same disposable NixOS environment with both denied and authorized paths before any physical-node installation.

Persistent NixOS `switch` and `boot` remain unsupported.
