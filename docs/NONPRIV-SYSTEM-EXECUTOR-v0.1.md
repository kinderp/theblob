# Non-Privileged System Executor v0.1

**Status:** Linux Pilot implementation checkpoint.

## Purpose

The first system executor deliberately implements only **materialization operations that cannot alter the running system configuration**.

It consumes a previously generated/validated NixOS `NixCommandPlan` and performs another restrictive boundary check before spawning a process.

## Accepted operations

```text
Materialize
BuildIsolatedVm
```

Both must have:

```text
effect_class = MaterializationOnly
authority    = User
program      = nix
argv shape   = build --no-link --print-out-paths <expected derivation selector>
```

## Rejected operations

The executor rejects:

- `PreviewActivation`;
- `TestActivation`;
- administrator-authority plans;
- any non-materialization effect class;
- programs other than `nix`;
- unexpected argument shapes;
- `--impure`, `--expr`, `--command`;
- `switch`, `boot`, `test`, `dry-activate` arguments;
- derivation selectors inconsistent with the semantic action.

The executor does not invoke a shell.

## Structured result

Execution returns:

- operation/candidate/SystemSpec IDs;
- semantic action/effect class;
- success/failure;
- exit code;
- stdout/stderr;
- duration;
- extracted immutable `/nix/store/...` output paths.

`evidence_lines()` exposes normalized evidence suitable for later inclusion in Temporal/Causal history.

## Real CI proof

The NixOS evaluation job constructs the reference `SystemCandidateOperation`, asks the NixOS backend for a plan, and passes that plan to `NonPrivilegedNixExecutor`.

The executor must successfully materialize the pinned reference NixOS `system.build.toplevel` and report at least one Nix store path.

This closes the loop:

```text
SystemSpec
 -> NixOS module
 -> SystemCandidateOperation(Materialize)
 -> NixCommandPlan
 -> NonPrivilegedNixExecutor
 -> real Nix build
 -> structured SystemOperationResult
 -> immutable Nix store artifact
```

## Next boundary

After this checkpoint is green, connect `SystemOperationResult` to the Causal Graph. Privileged `PreviewActivation` and `TestActivation` remain unimplemented in this executor and will require a dedicated physical NixOS test node and fresh authority check.
