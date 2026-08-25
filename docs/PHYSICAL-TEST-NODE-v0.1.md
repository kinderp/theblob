# Physical Test Node Profile v0.1

**Status:** Linux Pilot safety contract before any physical live activation.

## Purpose

The Blob has proven that a `SystemSpec` can be translated, evaluated, materialized, built as a VM and booted to userspace. The next boundary is more dangerous: allowing a candidate to affect a real machine.

A physical node must therefore be **enrolled and proven ready** before `PreviewActivation` or `TestActivation` can be executed.

The profile does not grant authority. It only states whether the observed machine satisfies the safety prerequisites for an already-authorized semantic action.

```text
SystemCandidateAction
        |
authority / policy
        |
PhysicalTestNodeProfile
        +
PhysicalTestNodeReadiness
        |
deterministic readiness validation
        |
eligible / rejected with explicit reasons
```

## Static profile vs observed readiness

The contract intentionally separates two things.

### `PhysicalTestNodeProfile`

Stable requirements for a test node:

- node identity;
- expected architecture;
- expected substrate;
- minimum free storage;
- whether preview activation is enabled;
- whether temporary live test activation is enabled;
- whether external power is required for live activation;
- whether local console recovery is required;
- whether a known previous boot generation/rollback reference is required.

### `PhysicalTestNodeReadiness`

Fresh observations gathered before the operation:

- enrolled/trusted state;
- observed architecture/substrate;
- external power state;
- free storage;
- storage health;
- current boot generation;
- rollback reference;
- local-console recovery confirmation;
- observation timestamp.

Readiness is evidence, not configuration.

## NixOS pilot defaults

`PhysicalTestNodeProfile::nixos_pilot(...)` currently requires:

- NixOS substrate;
- expected architecture match;
- at least 8 GiB free storage for candidate build material;
- enrolled and trusted node;
- storage health confirmed;
- external power for `TestActivation`;
- local console recovery for `TestActivation`;
- known current boot generation and explicit rollback reference for `TestActivation`.

These values are conservative pilot defaults, not permanent universal policy.

## Action behavior

### `Materialize`

Requires identity/trust/platform/storage readiness. It does not require power/console/rollback because it does not change the live configuration.

### `BuildIsolatedVm`

Same safety class as materialization. The host remains unchanged.

### `PreviewActivation`

Requires an enrolled, trusted and matching node and must be explicitly enabled by the node profile. Host-administrator authority remains separately required by the system-action policy. Preview may execute backend dry-activation hooks, so its evidence must be recorded.

### `TestActivation`

Requires the strongest v0.1 readiness:

- explicitly enabled by profile;
- external power when required;
- local console recovery confirmed;
- current boot generation known;
- rollback reference recorded;
- trust/storage/platform checks all green.

`TestActivation` is temporary live mutation. It still does **not** authorize persistent `boot` or `switch` activation.

## First physical machine rule

The first test node should be a recoverable Linux/NixOS machine for which the user can physically reach the console and boot a previous generation. It should not initially be the only machine holding important Personal World state.

No specific personal device is hard-coded into the architecture. Enrollment records the real selected test node later.

## Evidence and Technician integration

`PhysicalTestNodeReadiness::evidence_lines()` exposes stable structured evidence suitable for:

- policy/readiness traces;
- causal history;
- System Technician explanations;
- preflight UI.

The Technician should be able to say, for example:

> I can build this candidate now, but I will not test-activate it because the machine is on battery and no rollback generation has been confirmed.

The reason must come from deterministic readiness violations, not from an LLM guess.

## Non-goals v0.1

- automatic root/admin escalation;
- persistent `nixos-rebuild switch` or `boot`;
- remote-only recovery as sufficient proof;
- assuming Btrfs snapshots replace NixOS boot-generation recovery;
- hardware-specific tuning;
- selecting the user's real physical machine automatically;
- using AI output as readiness evidence.

## Exit criterion

This checkpoint is complete when the semantic core can deterministically distinguish:

```text
safe-to-materialize
safe-to-preview
safe-to-test-activate
unsafe + exact reasons
```

before any privileged physical executor is implemented.
