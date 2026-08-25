# ADR-0036: Compose recovered materialization admission into prepared request publication

## Status

Accepted for validation in a disposable NixOS VM. Physical-node execution remains disabled.

## Context

ADR-0035 proves that root commits to an exact derivation and expected output before an ordinary user materializes it, and that root later admits only that precommitted output after independent Nix verification. ADR-0034 proves that a root-owned admission can be converted into an exact prepared activation request without caller control over closure, program, argv or readiness evidence.

Those boundaries were validated separately. The remaining integration gap is whether a real admission produced by the derivation-bound authority is exactly what the request publisher consumes, including when the machine restarts while materialization is still pending.

## Decision

Add one disposable KVM composition proof covering:

`root begin -> durable pending intent -> reboot -> root resume -> non-root realization -> reboot -> root complete -> root admission -> D-Bus/polkit prepare -> root-owned prepared request`

The proof deliberately stops at prepared-request publication. Activation/execution is already covered by ADR-0033 and ADR-0034; the synthetic materialization used here is not presented as an activatable NixOS closure.

## Pending recovery semantics

A pending materialization intent is durable root-owned state.

Recovery accepts only the materialization operation id. Root reloads the existing canonical pending intent and returns the exact persisted derivation, expected output and `<drv>^out` build target. Recovery does not accept or recompute candidate, SystemSpec, source, installable attribute, derivation or output from caller input.

The allowed recovery states are:

1. **pending before realization**: `resume(operation)` returns the already committed target; completion still fails if the expected output has not been realized;
2. **pending after realization**: after restart, `resume(operation)` returns the same committed target and `complete(operation)` independently verifies the already realized expected output;
3. **completed**: the pending record is gone and `resume(operation)` fails closed; completion cannot be reopened as pending work;
4. **missing/corrupt prerequisite**: recovery fails closed. This checkpoint does not substitute another derivation/output or implicitly recompute a new materialization identity.

This checkpoint does not yet add cancellation, expiry or automatic Nix GC-root management for long-lived pending work. Those mechanisms should be added only when a concrete lifecycle requirement justifies them; their absence must never permit identity substitution.

## Request publication composition

After successful completion, the materialization authority writes the canonical root-owned admission. No test fixture or unprivileged process writes an admission.

The existing root request publisher then:

- receives only the admitted materialization operation id and bounded preview intent from the D-Bus request;
- derives the live caller from the system-bus sender;
- reloads candidate, SystemSpec, materialization operation and system closure from the admission produced by the materialization authority;
- obtains readiness from the root read-only probe and local root-owned safety confirmations;
- performs real polkit authorization;
- derives the exact immutable activation plan;
- publishes a root-owned mode-0600 ready request.

The composition test asserts that the closure, candidate, SystemSpec and materialization operation surfaced by the publisher are exactly those bound upstream by the recovered materialization intent/admission.

## Required proof

The KVM test must prove:

1. root begins materialization before the output exists and persists the exact `.drv`, output and build target;
2. no admission or prepared request exists at begin;
3. reboot preserves the pending intent;
4. root `resume` using only the operation id returns byte-for-byte the same derivation, output and target;
5. the normal materializer user cannot read the pending root intent;
6. that user realizes the exact recovered `<drv>^out` target;
7. a second reboot, after realization but before completion, still permits recovery of the same target;
8. root completion admits only the precommitted output and moves pending to completed;
9. the resulting admission is root-owned mode 0600 and unreadable by the normal user;
10. completed materialization cannot be reopened through `resume`;
11. an authorized non-root D-Bus caller can prepare preview from that exact admission through real polkit;
12. the root-owned prepared request contains the same candidate, SystemSpec, materialization operation and closure established upstream;
13. an operation without a root materialization admission cannot publish a prepared request.

## Safety boundary

This checkpoint still exposes no persistent `switch` or `boot`, no mutable-source `nixos-rebuild`, no caller-selected closure/program/argv, no caller-reported materialization output, and no physical-node execution.

The materialization `begin` transport remains test-shaped: this ADR proves durable provenance and downstream composition, not yet the final trusted control-plane record/API that supplies candidate/SystemSpec/source identity to `begin`.

## Consequence

If green, the Linux Pilot has a continuous provenance chain from a root-predicted derivation through non-root realization and root admission into a real polkit-authorized prepared request, including restart recovery on both sides of realization.

The next checkpoint should decide the production trusted-input boundary for `begin` and whether concrete pending-work lifecycle use cases require cancellation, expiry and/or explicit Nix GC-root retention before physical hardware is reconsidered.
