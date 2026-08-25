# ADR-0029 — Physical activation requires VM mechanism proof first

**Status:** Accepted for Linux Pilot v0.1

## Context

The Blob has deterministic policies, physical-node readiness, scoped single-use authorization, immutable candidate identity and an activation gate. Those safeguards are necessary but do not prove that NixOS itself behaves as assumed when `switch-to-configuration dry-activate|test` runs against a candidate closure.

The next step crosses from static validation into real system activation semantics.

## Decision

The Linux Pilot will prove the exact activation mechanism in an isolated NixOS VM before implementing or testing a privileged physical-node executor.

The VM integration test must exercise an actual immutable system closure and prove:

- `dry-activate` preserves the baseline running configuration;
- `test` temporarily switches to the candidate;
- no persistent boot-default action is requested;
- reboot restores the baseline configuration.

A successful unit/integration test of Rust planning alone is insufficient to waive this requirement.

## Rationale

The project deliberately advances privilege in small experimentally verified steps:

```text
semantic model
-> immutable build
-> isolated boot
-> readiness
-> scoped authorization
-> immutable activation planning
-> real activation in disposable VM
-> only then physical privileged helper
```

This is consistent with the Linux Pilot's inherited openSUSE/NixOS rollback philosophy and the project's failure-lesson rule that new abstractions must demonstrate measurable behavior rather than rely on elegance.

## Test isolation

The first proof uses `pkgs.testers.runNixOSTest` with QEMU/KVM. A NixOS specialisation supplies a second immutable system closure while inheriting the test instrumentation from the baseline.

The specialisation is a testing mechanism, not a new product abstraction and not the source of truth for production `SystemSpec` candidates.

## Failure policy

If the VM cannot prove temporary activation/reboot recovery reliably, physical activation work stops. The expected NixOS semantics or the Blob activation contract must be corrected before moving forward.

## Persistent activation

This ADR provides no justification for `switch` or `boot`. Persistent activation remains separately gated and requires a future ADR even after physical `test` activation becomes safe.
