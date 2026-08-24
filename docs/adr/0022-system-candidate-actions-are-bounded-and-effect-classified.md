# ADR-0022: System candidate actions are bounded and effect-classified

**Status:** Accepted

## Context

The Linux Pilot can now translate a backend-neutral `SystemSpec` into NixOS, evaluate/build a real VM candidate, and boot that VM to userspace. The next step is exposing candidate lifecycle operations to the System Technician and future UI without turning AI output into arbitrary privileged shell execution.

NixOS operations have materially different effects. `build`/VM build do not activate the live system, `dry-activate` previews activation but may run explicitly supported dry hooks, and `test` temporarily changes the running system while keeping the previous boot default.

## Decision

The trusted semantic core defines a closed v0.1 action set:

- `Materialize`;
- `BuildIsolatedVm`;
- `PreviewActivation`;
- `TestActivation`.

Each action has one canonical `SystemEffectClass` and `SystemAuthorityClass`. The mapping is recomputed/validated at boundaries rather than trusted from serialized fields.

The NixOS backend translates a validated semantic action into a structured `program + argv` plan. It does not accept arbitrary action strings or AI-generated shell fragments.

Persistent activation (`switch`, `boot`, boot-default mutation) is **not present in v0.1** and therefore cannot be accidentally requested through the API.

## Effect classes

```text
MaterializationOnly
PreviewHooks
TemporaryLiveActivation
```

## Authority classes

```text
User
HostAdministrator
```

## Consequences

- the System Technician can discuss/prepare candidates without gaining a generic shell authority;
- UI can clearly distinguish build/preview/test consequences;
- forged mismatches between action and declared authority/effect are rejected;
- the first executor can implement non-privileged materialization before any physical-host activation work;
- persistent activation requires a future explicit ADR and rollback/authority design.

## NixOS mapping

```text
Materialize       -> nix build ...system.build.toplevel
BuildIsolatedVm   -> nix build ...system.build.vm
PreviewActivation -> nixos-rebuild dry-activate --flake ...
TestActivation    -> nixos-rebuild test --flake ...
```

The backend treats dry activation as a distinct effect class because NixOS activation snippets may opt into dry-activation execution.
