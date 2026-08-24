# Failure Lessons — How The Blob Avoids Becoming Another Beautiful Research Island

**Status:** accepted project guidance.

The Blob explicitly studies the successes and failures of UNIX descendants and distributed/pervasive systems such as Plan 9, Inferno, Plan B, Omero and Octopus. The purpose is not nostalgia: it is to avoid repeating failures caused by incompatibility, excessive abstraction, network assumptions, latency blindness and research-driven complexity.

## Core lessons

### 1. Compatibility is a feature

A new abstraction is not valuable if adopting it requires abandoning the software, hardware and workflows users already depend on.

The Blob therefore supports mainstream substrates and legacy software as first-class citizens. Linux is the first deep-control substrate, but macOS, Windows and Android may participate as hosted nodes.

**Rule:** do not require the world to rewrite applications before The Blob becomes useful.

### 2. Internal sophistication must not become user complexity

Concepts such as `RequirementGraph`, `BindingLease`, `Projection`, `ConstraintIR` and Capsule runtime are architectural machinery. They should not become mandatory user vocabulary.

The normal user expresses goals and receives understandable explanations. Expert mode may expose progressively deeper layers.

### 3. Local-first, not distributed-for-distribution's-sake

A distributed Personal World must not become slower, less reliable or less coherent merely because remote execution is possible.

The Blob uses **semantic location transparency with physical locality awareness**:

- interactive work strongly prefers local execution;
- large compute may move to a better Fabric node;
- placement accounts for latency, bandwidth, jitter, energy, cost, data size and trust;
- distributed state is introduced only when it has measurable value.

### 4. Connected most of the time, not always connected

A Personal World must remain useful offline.

Workspaces should retain a useful local working set, Task state, required Knowledge Objects/Projections, cached Capsules and deterministic system functions. Reconnection should reconcile state through explicit version/causal mechanisms rather than assume uninterrupted network availability.

### 5. One coherent world per user

Plan B showed that fully peer-to-peer resource selection can produce inconsistent user experiences. The Blob therefore aims for **logical centralization with physical distribution**.

The Personal World has one coherent logical state and policy model even if copies, computation and capabilities are physically distributed and can continue temporarily offline.

### 6. New abstractions must earn their existence

A new kernel, compositor, filesystem, Capability abstraction or AI mechanism is not justified merely because it is elegant.

Each new layer should demonstrate measurable value against a simpler baseline.

Examples:

- keep Hyprland until a Blob compositor enables proven semantic behavior unavailable otherwise;
- keep normal files/export paths where they are useful even if Knowledge Objects are richer internally;
- modify/build a custom kernel only when a controlled experiment proves a meaningful benefit.

### 7. Mainstream systems should be integrated

The Blob is not initially a replacement for every operating system.

The strategy is:

```text
Linux reference/deep-control node
        +
macOS hosted node
        +
Windows hosted node
        +
Android hosted node
        +
wearable companion surfaces
```

The long-term architecture may deepen integration per platform, but usefulness must not depend on replacing every substrate.

### 8. AI must not become the new source of unpredictability

AI can interpret, diagnose, plan, explain and synthesize candidates. It does not automatically authorize privileged actions.

```text
AI proposal
   -> deterministic policy
   -> independent verification
   -> simulation/test where possible
   -> scoped authority
   -> execute
   -> verify outcome
   -> causal record / rollback
```

AI failure must not prevent boot, recovery, deterministic resolution, ordinary Workspace operation or data access.

### 9. Cloud is optional, not the Personal World

Cloud providers may supply reasoning or compute capabilities under policy, but cloud availability must not define whether the user's computer exists.

A cloud model never receives ambient root authority. It receives minimum authorized Projections and returns proposals/results through normal Capability boundaries.

### 10. Legacy software is an asset

Existing applications can initially appear as Legacy Surfaces and/or Capability Providers. This is not an architectural embarrassment: it is how The Blob gains immediate usefulness while native Workspace/Capability experiences mature.

## Anti-second-system-effect questions

Before accepting a major design change, ask:

1. What concrete user problem does this solve?
2. Can the same value be obtained by adapting an existing substrate?
3. Does this require users or developers to abandon something useful?
4. What is the latency/offline failure mode?
5. Can it be measured against a simpler baseline?
6. Does the AI add understanding, or merely hide complexity behind nondeterminism?
7. Can the change be versioned, explained and rolled back?

## Project invariant

> The Blob is allowed to be internally sophisticated only when that sophistication makes the user's computing environment simpler, more powerful, more understandable or more resilient.
