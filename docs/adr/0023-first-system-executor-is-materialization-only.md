# ADR-0023: First system executor is materialization-only

**Status:** Accepted

## Context

The Blob can now generate, build and boot a NixOS candidate VM and has bounded semantic actions for candidate lifecycle operations. Executing those plans on behalf of the System Technician introduces a new authority boundary.

Implementing privileged activation at the same time as the first executor would combine process execution, privilege escalation, live-system mutation and rollback semantics into one unproven step.

## Decision

The first `blob-system-executor` accepts only NixOS plans whose canonical semantic policy is:

```text
Materialize      / MaterializationOnly / User
BuildIsolatedVm  / MaterializationOnly / User
```

It rejects preview/test activation and any administrator-authority or live-system-changing plan.

The executor additionally validates the concrete backend plan before spawn: executable must be `nix`, argument form must match the expected pure `nix build --no-link --print-out-paths` shape, and known dangerous/free-form arguments are rejected.

No shell is invoked.

## Consequences

- the System Technician can safely prepare real immutable candidates before gaining any host activation capability;
- the executor produces structured evidence/store paths for future causal recording;
- NixOS planner output is treated as untrusted at the executor boundary and checked again;
- privileged `dry-activate`/`test` execution requires a later dedicated executor/authority path on a controlled test node;
- persistent `switch`/`boot` remains unavailable.
