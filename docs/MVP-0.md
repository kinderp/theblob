# MVP-0 — Deterministic Capability Binding Vertical Slice

**Goal:** prove the architectural separation between Workspace, Task, abstract Capability, ephemeral implementation, deterministic resolution, independent verification and causal recording.

## Scenario

A Development Workspace contains a source tree. Alfred observes a source modification and produces a candidate Situation:

```text
source changed -> project tests may need to run
```

The semantic plane proposes a Task requiring:

```text
capability = test.run
```

The Task does not choose the executable or node.

## Minimal world

```text
Implementations
  test-native-fast
  test-wasm-safe

Nodes
  laptop
  local-server
```

Example facts:

```text
native-fast on laptop:
  latency = 120 ms
  trust = verified
  network = none

wasm-safe on laptop:
  latency = 180 ms
  trust = verified
  sandbox = strong

native-fast on server:
  latency = 90 ms
  requires network = local
```

## RequirementGraph

One capability role is sufficient for MVP-0, but use the graph representation from day one.

```text
Task: run project tests

role test_runner:
  capability = test.run

policy:
  no public network

hard:
  implementation verified

preference:
  stronger sandbox preferred

objective:
  minimize latency
```

## Required components

Rust workspace modules:

```text
world-model
requirement-model
constraint-ir
candidate-deriver
solver-z3
binding-verifier
resolution-trace
alfred-events
workspace-model
```

UI may initially be CLI/text-first. Slint is added after the resolver behavior is stable; MVP-0 success does not depend on visual polish.

## Tests

1. exactly one valid candidate -> select it;
2. multiple valid candidates -> explicit objective profile selects deterministically;
3. fastest candidate violates policy -> reject it;
4. no solution -> produce domain explanation from named constraints;
5. fabricated invalid BindingPlan -> independent verifier rejects it;
6. equal objective score -> stable ID tie-break;
7. simulated solver timeout/unknown -> no new privileged binding;
8. existing valid BindingLease survives a harmless Fabric fact change;
9. lease invalidation at safe checkpoint triggers re-resolution;
10. after execution, runtime can be destroyed while Workspace/Task/result state remains.

## Success criteria

MVP-0 succeeds when it demonstrates:

```text
Workspace != application
Capability != implementation
Task != executable
AI proposal != authority
Solver proposal != authority
Capsule lifetime != user-state lifetime
Surface != Workspace
state history != causal explanation
```

And when the user can ask:

```text
Why did you choose this implementation?
Why was the faster one rejected?
Why is there no valid solution?
```

and receive deterministic domain-level answers.
