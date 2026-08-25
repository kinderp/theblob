# ADR-0024 — Physical live activation requires readiness proof

**Status:** Accepted for Linux Pilot v0.1

## Context

The Blob can already build and boot isolated NixOS candidates and materialize real system closures without changing a live machine. `PreviewActivation` and especially `TestActivation` cross a new safety boundary because backend hooks or the running system may be affected.

Host-administrator authority alone is not sufficient evidence that a physical machine is safe to use as an experimental node.

## Decision

Before any physical preview/test activation executor is implemented, The Blob will model and deterministically validate:

1. a static `PhysicalTestNodeProfile` describing the permitted test envelope; and
2. a fresh `PhysicalTestNodeReadiness` observation describing the actual machine state.

A live test activation is eligible only if readiness proves, at minimum for the NixOS pilot:

- correct enrolled node identity;
- trusted enrollment;
- expected architecture and substrate;
- adequate storage and healthy storage state;
- external power when required;
- confirmed local-console recovery;
- known current boot generation;
- explicit rollback reference.

The profile/readiness check is separate from authority policy. Passing it never grants administrator rights and never authorizes persistent `boot`/`switch` activation.

## Rationale

This follows the Linux Pilot rule that a change should become progressively more powerful only after the previous stage is measurable and reversible:

```text
translate
-> evaluate
-> materialize
-> isolated VM
-> explain
-> prove physical readiness
-> preview
-> temporary test activation
-> persistent activation (future ADR)
```

It also preserves multi-substrate architecture: the semantic concept of readiness can later be implemented differently on macOS/Windows/Linux-hosted nodes without teaching the System Technician backend-specific shell commands.

## Consequences

- unsafe state is represented by explicit violations rather than an AI judgement;
- the first physical-node test is intentionally conservative;
- some operations may be blocked even when the user has root/admin authority;
- readiness observations can become causal evidence and Technician explanations;
- a future persistent activation path requires its own authority/rollback ADR.

## Rejected alternatives

### Administrator approval is enough

Rejected. Authorization says *who may request an action*; readiness says *whether the current machine state is safe enough to attempt it*.

### Let `nixos-rebuild test` provide all safety

Rejected. The backend cannot know whether physical console recovery, power, trust and rollback evidence meet The Blob policy.

### Ask the AI Technician whether the machine looks safe

Rejected. LLM output is useful for explanation but is not an authority/readiness boundary.
