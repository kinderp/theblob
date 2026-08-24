# MVP-0 — Deterministic Capability Binding Vertical Slice

**Goal:** prove the architectural separation between Workspace, Task, abstract Capability, ephemeral implementation, deterministic resolution, independent verification and causal recording.

**Current checkpoint:** first headless vertical-slice code is present. Graphics are intentionally next, not prerequisite to the semantic proof.

## Scenario

A Development Workspace contains source state. Alfred observes a normalized source modification and derives:

```text
source.modified
      |
      v
development.source-change-requires-test
```

That Situation creates/reconsiders a Task requiring:

```text
capability = test.run
```

The Task does not choose executable or node.

## Current implementation path

```text
Event
  |
blob-alfred
  |
Situation
  |
Task + one-role RequirementGraph
  |
blob-resolver
  |  deterministic finite candidate ranking
  v
BindingPlan
  |
independent BindingVerifier
  |
BindingLease
  |
blob-executor
  |  MVP-only LocalProcessCapsule
  v
structured ExecutionResult
  |
Task state transition
  |
blob-history
  |
append-only CausalRecord chain
```

The complete chain is wired by `blob-mvp` and covered by an end-to-end fixture test.

## Rust crates

```text
blob-core
  semantic domain model only

blob-alfred
  deterministic event correlation + deduplication

blob-resolver
  MVP finite resolver + independent verifier

blob-executor
  controlled local-process Capsule prototype
  NOT a production sandbox

blob-history
  append-only causal log prototype

blob-mvp
  vertical-slice harness joining the components
```

The crates deliberately do not contain Slint, Hyprland, Nix, Z3 or AI-model dependencies yet.

## RequirementGraph scope

MVP-0 currently supports exactly one Capability role with no Constraint IR expressions. This limitation is explicit rather than silently ignored.

```text
Task: run project tests

role test_runner:
  capability = test.run
```

The finite resolver checks current candidate facts such as:

- implementation trust;
- node trust/online state;
- platform compatibility;
- memory requirement;
- accelerator requirement;
- runtime availability;
- deterministic implementation metrics.

The initial ranking profile is:

```text
1 maximize quality
2 minimize monetary cost
3 minimize expected latency
4 minimize expected energy
5 stable implementation/node ID tie-break
```

Constraint IR, Z3/SMT and richer policy solving remain behind the already-defined architectural boundary and are not faked in MVP-0.

## Independent verification

The resolver cannot authorize itself.

`BindingVerifier` independently rechecks the concrete selected plan against the RequirementGraph and registry facts before a `BindingLease` is created.

A test intentionally tampers with the selected node and verifies rejection.

## Alfred MVP

The first deterministic rule is versioned:

```text
development.source-change-requires-test@v1
```

Properties already represented in code/tests:

- stable Event ID;
- duplicate suppression;
- explicit evidence Event IDs;
- deterministic rule provenance;
- semantic AI provenance remains empty for this rule;
- unrelated Workspace events do not trigger the rule.

See `EVENT-SITUATION-CONTRACT-v0.1.md`.

## Ephemeral execution MVP

The first execution backend is intentionally modest:

```text
LocalProcessCapsule
```

It exists only to prove:

```text
materialize/request
-> execute
-> structured result
-> release/discard implementation state
```

It is explicitly **not** considered a sandbox. WASM/OCI/microVM security isolation begins in Phase 2.

Execution success/failure becomes structured data rather than becoming the identity/state of the Task.

## Causal record MVP

The first causal history is an append-only in-memory log with stable IDs and parent validation.

The vertical slice records:

```text
EventObserved
     |
SituationDerived
     |
TaskTransition
     |
BindingResolved
     |
ExecutionCompleted
```

This is intentionally separate from future temporal/content-addressed storage.

## Tests represented in the current bootstrap

Already encoded conceptually/in Rust tests:

1. Workspace/Task reference Capability semantics, not implementation identity;
2. same Surface can materialize under macOS and Hyprland Experience Profiles;
3. BindingLease defines a safe rebind boundary;
4. ImprovementProposal separates local evidence from external provenance;
5. Alfred source-change rule is deterministic;
6. replayed Event is deduplicated;
7. unrelated Workspace does not trigger the rule;
8. deterministic resolver selects among candidates;
9. offline candidate is rejected;
10. tampered BindingPlan is rejected by independent verifier;
11. unsupported Constraint IR fails explicitly rather than being ignored;
12. local process Capsule returns structured success/failure;
13. causal log rejects unknown parents and duplicate IDs;
14. full source-change -> execution -> causal-record vertical slice is wired.

## Remaining MVP-0 work

### Compilation/CI verification

The GitHub CI workflow compiles/tests the workspace at Rust 1.85. The project should not claim the bootstrap green until the workflow result has been inspected.

### Constraint/policy tests

The MVP resolver still needs a minimal explicit policy/constraint subset or a documented deferral to the first Z3-backed phase. We should not simulate a rich solver with ad-hoc string parsing.

### Capsule-state destruction proof

Add an explicit test that drops/evicts the selected Capsule/catalog entry after execution and demonstrates that Task/Workspace/result/causal state remains intact.

### First Surface

After headless flow verification, add a minimal Development Workspace Surface showing:

- source-change Situation;
- Task status;
- selected implementation/node;
- verification result;
- test output;
- causal explanation.

Slint/Blob Native is the first renderer target. Hyprland is the first Linux integration profile; neither enters `blob-core`.

### System Technician read-only slice

Consume a failed test/build Situation and produce an evidence-backed diagnostic/proposal without privileged mutation.

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

And when the system can answer from structured evidence:

```text
Why did you choose this implementation?
Why was another candidate rejected?
What authorized execution?
Which Event/Situation caused this Task?
What happened after execution?
```
