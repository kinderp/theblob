# NixOS Immutable Activation VM Proof v0.1

**Status:** Linux Pilot integration checkpoint before physical privileged execution.

## Purpose

Unit tests prove that The Blob plans `dry-activate` and `test` against an immutable NixOS system closure. Before allowing a privileged helper to touch physical hardware, the underlying NixOS semantics must be exercised end-to-end in an isolated QEMU/NixOS test VM.

## Test shape

The test boots one baseline NixOS machine with an inherited NixOS specialisation named `blob-candidate`.

The two immutable system closures differ in one observable file:

```text
baseline  -> /etc/blob-activation-state = BASELINE
candidate -> /etc/blob-activation-state = CANDIDATE
```

The candidate specialisation inherits the complete test-machine configuration so NixOS test instrumentation remains available after temporary activation.

## Assertions

The test must prove, in order:

1. the VM reaches `multi-user.target` as the baseline;
2. baseline and candidate resolve to distinct `/nix/store/...` system closures;
3. the candidate exposes `bin/switch-to-configuration`;
4. running `dry-activate` on the exact candidate closure does **not** change `/run/current-system`;
5. the baseline marker remains after the preview;
6. running `test` on that same closure changes the marker to `CANDIDATE`;
7. `/run/current-system` now resolves to that exact candidate closure;
8. reboot restores the baseline marker;
9. `/run/current-system` after reboot resolves to the original baseline closure.

No `switch` or `boot` action is used.

## Why a NixOS specialisation is acceptable for this proof

NixOS specialisations are themselves built system configurations and inherit the parent configuration by default. They provide a convenient way to ensure the candidate closure is already present in the VM's Nix store while retaining the test harness and all baseline services.

This test validates the **activation mechanism**, not the final production candidate-storage format. The production Linux Pilot still obtains its candidate closure from the actual Blob `SystemSpec -> Nix -> Materialize -> SystemOperationResult` path.

## CI

A dedicated KVM-enabled GitHub Actions job builds:

```text
.#checks.x86_64-linux.immutable-activation
```

only for the feature branch initially. If the test proves stable and sufficiently fast, its promotion to a permanent gate can be decided separately.

## Exit criterion

Physical privileged-executor design may begin only after this check passes in CI and demonstrates temporary activation plus reboot recovery using the exact immutable closure path.
