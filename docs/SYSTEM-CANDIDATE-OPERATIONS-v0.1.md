# System Candidate Operations v0.1

**Status:** first bounded execution-planning contract for the Linux Pilot.

## 1. Purpose

The Blob must distinguish **describing a candidate system** from **doing something with that candidate**.

`SystemSpec` describes desired state. `SystemCandidateOperation` describes one permitted lifecycle action over a candidate. A substrate backend translates that semantic action into an inspectable execution plan.

```text
SystemSpec
   |
candidate materialization
   |
SystemCandidateOperation
   |
canonical effect/authority validation
   |
NixOS operation planning
   |
program + argv + expected effects + rollback semantics
   |
future executor / authority gate
```

No AI-generated shell string is part of this contract.

## 2. v0.1 semantic actions

### `Materialize`

Build the immutable system candidate without activating it on the live host.

Effect class: `MaterializationOnly`.
Authority class: `User`.

The NixOS backend maps this to a direct `nix build` of `system.build.toplevel`.

### `BuildIsolatedVm`

Build an isolated QEMU representation of the candidate.

Effect class: `MaterializationOnly`.
Authority class: `User`.

The NixOS backend maps this to a direct `nix build` of `system.build.vm`.

### `PreviewActivation`

Ask the substrate to calculate/show what live activation would do.

Effect class: `PreviewHooks`.
Authority class: `HostAdministrator`.

For NixOS this maps to `nixos-rebuild dry-activate`. It is intentionally **not** classified as a pure build: NixOS activation snippets can explicitly support dry activation and may execute in this mode.

### `TestActivation`

Temporarily activate the candidate on the running host without making it the boot-default configuration.

Effect class: `TemporaryLiveActivation`.
Authority class: `HostAdministrator`.

For NixOS this maps to `nixos-rebuild test`.

## 3. Persistent activation is absent by construction

v0.1 contains no semantic action for:

- `nixos-rebuild switch`;
- `nixos-rebuild boot`;
- boot-default mutation;
- permanent live activation.

This is intentional. Persistent activation will require a separate architecture/authority decision after candidate preparation, VM testing, causal evidence and rollback semantics are proven on a dedicated Linux test node.

## 4. Canonical policy validation

Effect and authority fields are derived from the semantic action, but serialized/in-memory objects are still considered untrusted input at execution boundaries.

Therefore `SystemCandidateOperation::validate_policy()` recomputes the canonical policy and rejects mismatches.

Example attack/error:

```text
action       = TestActivation
effect_class = MaterializationOnly   # forged/incorrect
authority    = User                  # forged/incorrect
```

The core rejects the operation and the NixOS backend refuses to plan it.

## 5. NixOS command plan

The backend produces a structured `NixCommandPlan`:

```text
operation id
candidate id
SystemSpec id
action
effect class
authority class
program
argv[]
expected effects[]
rollback semantics
```

`program` and `argv` remain separate. The backend never emits a single shell command string for execution.

Initial mappings:

```text
Materialize
  -> nix build --no-link --print-out-paths <flake>#nixosConfigurations.<host>.config.system.build.toplevel

BuildIsolatedVm
  -> nix build --no-link --print-out-paths <flake>#nixosConfigurations.<host>.config.system.build.vm

PreviewActivation
  -> nixos-rebuild dry-activate --flake <flake>#<host>

TestActivation
  -> nixos-rebuild test --flake <flake>#<host>
```

Configuration names are validated before planning. Unsupported/free-form action strings do not exist in the API.

## 6. Executor boundary

This contract plans operations; it does not yet execute privileged host activation.

The future executor must:

1. accept only a validated structured plan;
2. check the current authority grant/policy again immediately before execution;
3. capture stdout/stderr/exit status and relevant Nix store/generation identifiers;
4. produce structured evidence;
5. append causal records;
6. refuse stale/expired candidate authority;
7. never silently escalate `PreviewActivation` into `TestActivation` or persistent activation.

## 7. Rollback semantics

- `Materialize`: no live rollback; candidate may be garbage-collected later.
- `BuildIsolatedVm`: discard VM state/artifact reference; host unchanged.
- `PreviewActivation`: no configuration switch, but dry hooks are captured as evidence.
- `TestActivation`: previous boot-default configuration remains authoritative; reboot returns to it, and a more explicit rollback reference should be captured before execution.

## 8. Next step

After CI validates v0.1 planning, implement a **non-privileged executor first** for `Materialize` and `BuildIsolatedVm`, returning structured `SystemOperationResult` and causal evidence.

Only then add privileged host-side execution for `PreviewActivation` and `TestActivation` on a dedicated NixOS test node.
