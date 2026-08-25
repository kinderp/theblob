# ADR-0035: Bind materialization admission to a root-predicted derivation output

## Status

Accepted for validation in a disposable NixOS VM. Physical-node execution remains disabled.

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

Root independently resolves the exact derivation and its `out` output with Nix and persists a canonical mode-0600 intent containing both:

- the `.drv` path;
- the exact expected output path.

The resulting build target is the derivation output (`<drv>^out`).

### Non-privileged realization

The actual build may be performed by an ordinary user. That process can realize the already selected derivation, but it cannot alter the root-owned intent and cannot nominate the path that root will later admit.

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

The immutable flake root must be a canonical subpath of `/nix/store`. The installable attribute is syntactically bounded and is persisted before build execution.

## Required proof

The disposable KVM test must prove:

1. root resolves and persists a concrete `.drv` and exact expected output before realization;
2. the pending intent is root-owned mode 0600 and unreadable by the normal materializer user;
3. completion before realization fails and does not build the missing output as a side effect;
4. an ordinary non-root user can realize exactly the root-selected `<drv>^out` target;
5. completion accepts only the operation id and admits the precomputed expected output;
6. admission provenance records immutable source, installable attribute, derivation, expected output, verified deriver and NAR hash;
7. the resulting admission is root-owned mode 0600 and unreadable by the normal user;
8. the pending intent becomes completed after admission;
9. a separately realizable decoy derivation cannot substitute its output for the committed operation;
10. replaying a completed operation cannot create or mutate another admission.

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

Once this checkpoint is green, the materialization-to-admission-to-request-to-execution chain has an end-to-end immutable closure provenance story: root commits to the derivation before a non-root build, independently verifies the realized output, and all later privileged stages consume that root-owned identity rather than caller-supplied paths.

The next work should compose this admission authority directly with the ADR-0034 publisher in one KVM proof and define recovery for materializations that are pending across daemon or machine restart before any physical hardware enablement is reconsidered.
