# ADR-0027 — Privileged NixOS activation uses the reviewed immutable closure

**Status:** Accepted for Linux Pilot v0.1

## Context

The Blob originally modeled privileged NixOS preview/test operations as `nixos-rebuild ... --flake <candidate>`. That is convenient, but it creates a review-to-execution integrity problem: a flake working tree is mutable. Files could change after candidate materialization, preflight or user authorization, causing the privileged step to build/activate bytes different from the reviewed candidate.

NixOS already provides a better primitive. A built NixOS system closure contains `$out/bin/switch-to-configuration`, which accepts activation actions such as `dry-activate` and `test`.

## Decision

The privileged Linux Pilot path will activate **only an already-materialized Nix store system closure**.

```text
SystemSpec
  -> deterministic Nix translation
  -> materialize system.build.toplevel
  -> SystemOperationResult
  -> exact /nix/store/<system-closure>
  -> causal evidence
  -> physical-node readiness
  -> scoped single-use authorization
  -> <closure>/bin/switch-to-configuration dry-activate|test
```

The privileged activation plan contains no flake selector and performs no source rebuild.

## Materialized candidate identity

`MaterializedNixOsCandidate` is created only from a successful `Materialize` `SystemOperationResult` with exactly one validated top-level `/nix/store/<entry>` output. It preserves:

- candidate ID;
- SystemSpec ID;
- materialization operation ID;
- exact system closure path.

Preview/test operations must match candidate and SystemSpec exactly.

## Preview

`PreviewActivation` maps to:

```text
/nix/store/<reviewed-system>/bin/switch-to-configuration dry-activate
```

Dry activation does not switch the running system, but NixOS activation snippets explicitly declaring dry-activation support may run; their effects remain evidence-worthy.

## Temporary live test

`TestActivation` maps to:

```text
/nix/store/<reviewed-system>/bin/switch-to-configuration test
```

This temporarily changes the live system without changing the boot-default generation. Physical readiness must already contain the known boot-default rollback reference and recovery proof.

## Security benefit

This removes a source-level TOCTOU window between review/authorization and privileged activation. The exact immutable closure that was materialized and recorded is the one considered for activation.

The future privileged helper must additionally verify that the store path exists, is the expected closure, contains the expected executable, and remains bound to the authorized candidate/readiness receipt before execution.

## Existing flake-based backend plans

Any older backend API capable of constructing `nixos-rebuild dry-activate/test --flake ...` is **not an authorized execution path** for the Linux Pilot. It may be removed/deprecated separately. The privileged executor must consume the immutable-closure plan defined by this ADR.

## Persistent activation

`switch` and `boot` are deliberately absent. Making a candidate persistent changes the system profile/bootloader and requires a separate future ADR, rollback protocol and stronger durability checks.

## References

This design follows the NixOS mechanism where `nixos-rebuild` delegates activation to `switch-to-configuration` inside the built system closure. The project documentation should link to the current official NixOS manual when presenting this mechanism to users.
