# ADR-0035: Bind materialization admission to a root-predicted derivation output

## Status

Validated in a disposable NixOS VM. Physical-node execution remains disabled.

## Context

ADR-0034 introduced a root request publisher that can prepare an exact privileged activation request only from a root-owned materialization admission. That removed caller control over the activation closure, program, argv and readiness evidence, but the admission itself was still staged by a test fixture.

The remaining provenance risk was therefore upstream: if an unprivileged materializer could report an arbitrary `/nix/store/...` path as its result, root could faithfully publish and execute the wrong closure even though every downstream boundary was correct.

The materialization boundary must not trust a reported output path.

## Decision

Add a root materialization admission authority with a two-phase protocol.

### Begin

Before a non-privileged build starts, root receives a materialization identity consisting of:

- local node;
- candidate id;
- SystemSpec id;
- materialization operation id;
- immutable flake root already located under `/nix/store/...`;
- bounded installable attribute.

The flake root must be canonical as supplied: root resolves it with the host filesystem and rejects it if canonicalization changes the path or escapes the immutable Nix store object. A lexically store-local subpath that traverses a symlink outside `/nix/store` is therefore rejected before Nix evaluation.

Root independently resolves the exact derivation and its `out` output with Nix and persists a canonical mode-0600 intent containing both:

- the `.drv` path;
- the exact expected output path.

The resulting build target is the derivation output (`<drv>^out`).

### Non-privileged realization

The actual build may be performed by an ordinary user. That process can realize the already selected derivation, but it cannot alter the root-owned intent and cannot nominate the path that root will later admit.

The VM proof performs this realization with substituters disabled and a pure flake evaluation. Its minimal fixture uses a declared static local builder input only to keep the test hermetic; the architectural property being proved is that the normal user realizes the exact `.drv` root committed to beforehand.

### Complete

Completion accepts only the materialization operation id. Root reloads its durable intent and checks the exact output it predicted before the build:

1. the output already exists; completion does not realize it;
2. `nix store verify --no-trust` succeeds;
3. the output's observed deriver is exactly the persisted `.drv`;
4. Nix returns a non-empty NAR hash.

Only then does root create the canonical materialization admission consumed by ADR-0034. The admitted closure is always the intent's precomputed expected output, never a caller-provided path.

The pending intent is then moved atomically to the completed state. Completed operations cannot be replayed to mint another admission.

## Trust and filesystem boundary

Materialization intents and admissions are stored below root-owned mode-0700 directories. Individual records are root-owned mode-0600 ordinary files. Symlinks, permissive modes, malformed records and non-canonical serialization fail closed.

The immutable flake root must be both lexically store-local and filesystem-canonical. The installable attribute is syntactically bounded and is persisted before build execution.

## Validated proof

The disposable KVM test proves all of the following:

1. a store-local path that resolves through a symlink outside the store is rejected before Nix evaluation;
2. root resolves and persists a concrete `.drv` and exact expected output before realization;
3. the pending intent is root-owned mode 0600 and unreadable by the normal materializer user;
4. completion before realization fails, leaves the output absent and creates no admission, proving completion is verification-only;
5. an ordinary non-root user realizes exactly the root-selected `<drv>^out` target;
6. completion accepts only the operation id and admits the precomputed expected output;
7. admission provenance records immutable source, installable attribute, derivation, expected output, verified deriver and NAR hash;
8. the resulting admission is root-owned mode 0600 and unreadable by the normal user;
9. the pending intent becomes completed after admission;
10. a separately realizable decoy derivation cannot substitute its output for the committed operation;
11. replaying a completed operation cannot create or mutate another admission.

The dedicated `materialization-admission` NixOS KVM check passed with network substituters disabled.

## Safety boundary

This checkpoint still does not allow:

- persistent NixOS `switch` or `boot`;
- mutable-source `nixos-rebuild`;
- caller-selected activation closure;
- caller-selected privileged program or argv;
- caller-reported materialization output paths;
- unprivileged writes to intent/admission stores;
- physical-node execution.

It also does not yet define the final production transport by which the control plane asks root to begin a materialization. That API must preserve the same property: security-sensitive source/SystemSpec/candidate identity must come from an already trusted control-plane record rather than arbitrary D-Bus text.

## Consequence

The materialization-to-admission boundary now has a validated immutable provenance rule: root commits to the exact derivation and expected output before a non-root build, independently verifies the realized output, and publishes only that precommitted identity for downstream privileged stages.

The next work should compose this authority directly with the ADR-0034 publisher in one KVM proof and define recovery for materializations that remain pending across daemon or machine restart. Only after that composition and recovery checkpoint is green should physical-hardware enablement be reconsidered.
