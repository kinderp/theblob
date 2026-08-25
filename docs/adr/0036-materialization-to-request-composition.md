# ADR-0036: Compose recovered materialization admission into prepared request publication

## Status

Validated in a disposable NixOS VM. Physical-node execution remains disabled.

## Context

ADR-0035 proves that root commits to an exact derivation and expected output before an ordinary user materializes it, and that root later admits only that precommitted output after independent Nix verification. ADR-0034 proves that a root-owned admission can be converted into an exact prepared activation request without caller control over closure, program, argv or readiness evidence.

Those boundaries were validated separately. The remaining integration gap was whether a real admission produced by the derivation-bound authority is exactly what the request publisher consumes, including when the machine restarts while materialization is still pending.

The KVM work exposed an additional lifecycle requirement: a durable pending record is not sufficient by itself. The Nix objects needed to continue that exact materialization must remain live across restart as well.

## Decision

Validate one continuous VM path:

`root begin -> durable pending intent -> reboot -> root resume -> non-root realization -> reboot -> root resume/complete -> root admission -> D-Bus/polkit prepare -> root-owned prepared request`

The proof deliberately stops at prepared-request publication. Activation/execution is already covered by the previous privileged-boundary checkpoints; the synthetic materialization used here is not presented as an activatable NixOS system closure.

## Pending recovery contract

A pending materialization consists of both **durable identity** and **retained Nix state**.

### Durable identity

The canonical root-owned pending intent contains the identity committed before realization:

- node;
- candidate;
- SystemSpec;
- materialization operation;
- immutable source root;
- installable attribute;
- exact derivation path;
- exact expected output path.

Recovery accepts only the materialization operation id. It never accepts replacement candidate, SystemSpec, source, attribute, derivation or output from the caller.

### Nix retention

Before the pending intent becomes visible, root retains the exact resolved `.drv` through a Nix GC root. The ordering is intentionally fail-safe: a crash before the intent write can leave an inert orphan GC root, but cannot leave a visible pending operation whose committed derivation closure has already disappeared.

The immutable source referenced by the pending identity must also remain available from a trusted retention boundary. In the KVM proof it is retained as an explicit system dependency. A production implementation may use a dedicated source GC root or an already trusted upstream root, but it must not depend on an unrooted caller-owned cache entry.

After successful admission, the pending derivation GC root is released. A missing or mismatched retention root fails closed.

### Store persistence

`runNixOSTest` normally places the writable `/nix/store` overlay on tmpfs, which discards dynamically evaluated and built paths at reboot. That behavior is useful for disposable tests but cannot validate installed-machine recovery semantics.

The recovery proof therefore uses a writable Nix store backed by the VM disk (`virtualisation.writableStore = true` and `virtualisation.writableStoreUseTmpfs = false`). This preserves dynamically created derivations and outputs across the two real reboots and models the persistence expected from an installed host.

### Resume revalidation

`resume(operation)`:

1. reloads the canonical root-owned pending intent;
2. requires the exact pending derivation retention root;
3. refreshes the Nix flake archive from the persisted immutable source;
4. re-resolves the derivation using only the persisted source and attribute;
5. requires both the resolved `.drv` and expected output to equal the values committed at `begin`;
6. returns the already committed `<drv>^out` target.

Re-resolution is verification, not identity recomputation: any mismatch is terminal for that recovery attempt and no substituted identity is accepted.

## Validated recovery states

The KVM proof validates:

1. **pending before realization**: after a real reboot, `resume(operation)` recovers and revalidates exactly the committed derivation/output/target;
2. **pending after realization**: an ordinary user realizes the exact recovered `<drv>^out`; after a second real reboot, the same pending identity and built output remain available and `resume(operation)` revalidates them again;
3. **completion**: root independently verifies the precommitted output, creates the canonical admission and moves the intent from pending to completed;
4. **completed**: `resume(operation)` can no longer reopen the operation;
5. **missing/corrupt/mismatched prerequisite**: recovery fails closed instead of substituting another source, derivation or output.

This checkpoint intentionally does not add cancellation or expiry. Those behaviors need their own explicit product/lifecycle use cases and cleanup semantics rather than being introduced speculatively.

## Request publication composition

After successful completion, the materialization authority itself writes the canonical root-owned admission. No fixture and no unprivileged process writes that admission.

The existing root request publisher then:

- receives only the admitted materialization operation id and bounded preview intent from the D-Bus request;
- derives the live caller from the system-bus sender;
- reloads candidate, SystemSpec, materialization operation and system closure from the authority-produced admission;
- obtains readiness from the root read-only probe and local root-owned safety confirmations;
- performs real polkit authorization;
- derives the exact immutable activation plan;
- publishes a root-owned mode-0600 ready request.

The composition test verifies that the closure, candidate, SystemSpec and materialization operation surfaced by the publisher are exactly those bound upstream by the recovered materialization intent and admission.

An operation with no root materialization admission cannot publish a prepared request.

## Validated proof

The dedicated KVM test proves all of the following in one execution:

1. root commits the exact `.drv`, output and build target before the output exists;
2. no admission or prepared request exists at begin;
3. the pending intent is root-owned and unreadable by the ordinary materializer user;
4. the exact derivation is retained before the pending intent is exposed;
5. a real reboot preserves the pending state and its required Nix objects;
6. `resume(operation)` revalidates and returns exactly the original derivation/output/target without caller-supplied identity fields;
7. the normal user realizes only that exact `<drv>^out` target;
8. a second real reboot, after realization but before completion, preserves the materialized output and allows the same exact recovery;
9. root completion admits only the precommitted output and closes the pending state;
10. the admission is root-owned mode 0600 and unreadable by the normal user;
11. completed materialization cannot be reopened through `resume`;
12. an authorized non-root D-Bus caller prepares preview from that exact admission through real polkit;
13. the resulting root-owned prepared request preserves the upstream candidate, SystemSpec, materialization operation and closure;
14. an operation without a root materialization admission cannot publish another prepared request.

The test runs with network substituters disabled and does not rely on caller-reported output paths.

## Safety boundary

This checkpoint still exposes no persistent `switch` or `boot`, no mutable-source `nixos-rebuild`, no caller-selected closure/program/argv, no caller-reported materialization output, and no physical-node execution.

The materialization `begin` transport remains test-shaped. This ADR validates durable provenance, Nix lifecycle retention, restart recovery and downstream publication; it does not yet define the final trusted control-plane record/API that supplies candidate/SystemSpec/source identity to `begin`.

The GC-root lifecycle is currently exercised by the VM integration harness. Before production enablement, that behavior must move into the installed root service/authority implementation with orphan-root reconciliation after crashes.

## Consequence

The Linux Pilot now has a validated continuous provenance path from a root-predicted derivation through non-root realization and two restart boundaries into root admission and a real polkit-authorized prepared request.

The next checkpoint is the production trusted-input boundary for `begin`: candidate, SystemSpec, materialization operation, immutable source and installable identity must come from an already trusted root/control-plane record rather than arbitrary D-Bus text. That checkpoint should also move pending GC-root management and orphan reconciliation out of the VM harness and into the production-shaped authority service before physical hardware is reconsidered.
