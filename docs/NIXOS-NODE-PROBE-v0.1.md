# NixOS Read-Only Node Probe v0.1

**Status:** Linux Pilot implementation contract.

## Purpose

`PHYSICAL-TEST-NODE-v0.1.md` defines which facts must be true before a physical test node is eligible for increasingly powerful system operations. This probe gathers the subset of those facts that can be observed locally without mutating the machine or requesting administrator authority.

The probe is intentionally not an AI task.

## Automatically observed facts

The v0.1 NixOS probe attempts to establish:

- CPU architecture from the running process architecture;
- NixOS substrate from `/etc/os-release`;
- external-power state from `/sys/class/power_supply` when the kernel exposes it;
- free bytes for `/nix/store` using fixed-argv POSIX `df`, never a generated shell command;
- current boot-default NixOS generation from `/nix/var/nix/profiles/system`;
- canonical running-system and boot-profile store paths when available;
- observation timestamp.

For a future `nixos-rebuild test`, the **current boot-default generation** is the initial rollback reference because a reboot should return to that profile. Persistent `boot`/`switch` remains outside the v0.1 authority model.

## Facts that are not automatically trusted

The following remain explicit confirmations/enrollment state:

- whether the node has been enrolled into the Personal World;
- whether that enrollment is trusted;
- storage health;
- whether the user has actually verified local-console recovery;
- external power when sysfs cannot establish it reliably.

The probe does not infer these with an LLM and does not silently convert unknown into safe.

## Unknown semantics

Unknown observations are conservative:

- unknown external power becomes `false` unless physically confirmed;
- unknown free space becomes zero in the readiness object;
- unknown generation/rollback remains absent;
- any resulting readiness violation blocks the relevant action.

This allows `Materialize`/`BuildIsolatedVm` to remain available when live-activation-only evidence is missing, while preventing `TestActivation` from crossing the safety boundary.

## Read-only discipline

The current probe:

- reads files/symlinks;
- enumerates sysfs;
- executes only the fixed program `df` with fixed option shape;
- does not use a shell;
- does not run `sudo`;
- does not edit Nix profiles;
- does not garbage-collect generations;
- does not call `nixos-rebuild`;
- does not install packages or drivers.

## Generation evidence

NixOS keeps system generations in the system profile. The probe records the profile symlink and parses a generation only when its conventional `system-N-link` shape is visible. If it cannot prove the generation, the physical live-activation readiness check remains unsatisfied.

The probe also compares the canonical running-system and boot-profile store paths when both are available. A difference is recorded as a warning rather than silently interpreted.

## Integration

```text
NixOsReadOnlyProbe
        |
NixOsProbeSnapshot
        +
NodeSafetyConfirmations
        |
PhysicalTestNodeReadiness
        |
PhysicalTestNodeProfile::validate_readiness(action)
        |
eligible / deterministic violations
```

The resulting evidence is suitable for causal history and System Technician explanation.

## Next checkpoint

After this probe is CI-valid, The Blob can build a **preflight report** for a real NixOS machine. Only after a user selects/enrolls a physical test node and the preflight is green should we implement the privileged executor for `PreviewActivation`/`TestActivation`.
