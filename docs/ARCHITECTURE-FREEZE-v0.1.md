# Architecture Freeze v0.1 — Core semantic boundaries

**Status:** accepted baseline for the first implementation.

This freeze does **not** freeze implementation technologies such as NixOS, Z3, Slint, Hyprland, Smithay, WASM or OCI. It freezes only the semantic responsibilities that MVP-0 should preserve unless an ADR explicitly changes them.

## 1. Purpose of the freeze

The Blob has accumulated several powerful ideas from Plan 9, Inferno, Plan B and the project brainstorming. Before writing the trusted core, we need to prevent important concepts from collapsing into one another.

The core rule is:

```text
meaning/state != presentation != implementation != placement != authority
```

A Task must not become a process. A Workspace must not become an application. A Capability must not become a package. A Surface must not become a window. An LLM must not become an authority boundary.

## 2. Canonical flow

```text
Human / authorized agent
        |
     Intent
        |
        +--------------------+
        |                    |
        v                    v
      Goal              existing Context
        |                    |
        +---------+----------+
                  |
                  v
              Workspace
                  |
                  v
                Task
                  |
                  v
         RequirementGraph
                  |
                  v
        deterministic resolution
                  |
                  v
             BindingPlan
                  |
                  v
             BindingLease
                  |
                  v
     ephemeral Capability execution
                  |
                  v
        verified Task outcome
                  |
        persistent state updates
                  |
                  v
        Temporal/Causal evidence
```

In parallel:

```text
Events -> Alfred -> Situation
                    |
          +---------+----------+
          |                    |
          v                    v
      Task/Planner        System Technician
                               |
                     ImprovementProposal
                               |
                        Adaptive System
```

## 3. Personal World

The **Personal World** is the user's persistent logical computing environment.

It owns or references:

- identity and delegated authority;
- preferences and semantic policy;
- Workspaces and Workspace Recipes;
- Goals and Tasks;
- Knowledge Objects and relations;
- semantic/context memory;
- trusted Fabric membership metadata;
- causal references to important transitions.

It does **not** own:

- ephemeral Capsule instances;
- process IDs;
- node-local caches;
- compositor/window IDs;
- one machine's package database as its canonical state.

**Lifetime:** user/personal-environment lifetime.

## 4. Intent

An **Intent** is an expressed desired outcome from a human or authorized agent.

It may create or modify a Goal/Task, but it is not itself executable authority.

Examples:

```text
"translate this document locally"
"make this laptop use less battery"
"continue Pollicino"
```

**Lifetime:** event/request lifetime, with provenance retained where causally important.

## 5. Goal

A **Goal** is a persistent desired outcome that may survive individual Tasks, devices and Workspaces.

A Goal may:

- have success criteria and deadlines;
- span multiple Workspaces;
- generate/reprioritize Tasks;
- accumulate evidence and progress.

A Goal does not directly bind implementations or execute effects.

**Lifetime:** potentially long-lived.

## 6. Workspace

A **Workspace** is a relatively persistent, user-recognizable semantic environment for a domain/project/activity.

It owns or references:

- persistent context;
- relevant Knowledge Object Views/Projections;
- Task set;
- Experience Grammar;
- selected Experience Profiles by device/context;
- baseline/pinned Capability **requirements**, not implementations;
- Workspace-level policies/preferences;
- Surface state references.

It does **not** own:

- Knowledge Object data as application-private state;
- Capsule binaries;
- one compositor's workspace number;
- one renderer implementation;
- a specific machine.

A Workspace is the semantic successor to a Plan 9 private namespace, extended to context, capabilities, UI grammar, policies and work state.

**Lifetime:** days to years.

## 7. Workspace Recipe

A **Workspace Recipe** is a versioned blueprint that can instantiate/configure a Workspace.

It may define:

- required semantic roles;
- Experience Grammar defaults;
- baseline Capability requirements;
- policy defaults;
- compatible Experience Profiles;
- benchmark/quality expectations;
- migration/version rules.

Recipes may be Ready, AI Designed or Expert-derived; they are forkable, mergeable and publishable.

A Recipe is **not** a running Workspace.

## 8. Experience Grammar

The **Experience Grammar** is the stable interaction language of a Workspace: roles, navigation, persistent interaction patterns, command model, shortcut/gesture semantics and placement preferences.

It should be stable enough for muscle memory and versioned when it changes.

It is semantic, not pixel-level.

## 9. Experience Profile

An **Experience Profile** selects/adapts presentation conventions for a platform/device/Workspace combination.

Examples:

```text
macos-native
hyprland-keyboard-first
blob-native
android-native
```

It can choose renderer/integration strategy, but it cannot change the underlying identity of Tasks, Objects or Capabilities.

## 10. Surface

A **Surface** is a typed semantic projection of a Workspace or Task onto current I/O capabilities and context.

Examples:

```text
EditorSurface
TechnicianPanel
TaskStatus
ComparisonPanel
Timeline
LegacySurface
```

A Surface is **not inherently a window**. A renderer may materialize it as a Wayland surface, macOS native view/window, Android view, watch card or another medium.

Surface state should be inspectable by trusted agents without requiring pixel automation where possible.

**Lifetime:** contextual; may be persistent, session-lived or ephemeral.

## 11. Task

A **Task** is the canonical unit of concrete work.

It owns:

- desired outcome;
- state machine/status;
- typed inputs/outputs;
- working context references;
- requested/allowed effects;
- RequirementGraph(s);
- causal provenance;
- checkpoints.

It may survive migration between devices or replacement of implementations.

It does **not** equal a process/container/agent conversation.

**Lifetime:** seconds to months, depending on task.

## 12. RequirementGraph

A **RequirementGraph** is the deterministic resolution problem derived from a Task, Intent, Situation or system proposal.

It contains:

- typed roles;
- relations between roles;
- policy constraints;
- hard correctness constraints;
- preferences;
- explicit objectives;
- authority/effect requirements.

It requests **what must be satisfied**, not what executable to launch.

## 13. Capability

A **Capability** is an abstract typed ability independent of implementation, packaging and location.

Examples:

```text
document.translate
test.run
web.render
model.inference
kernel.build
```

A Capability contract defines semantic inputs/outputs, effects and relevant metadata.

A Capability does **not** own user state.

## 14. Capability Implementation / Capsule

A **Capability Implementation** is one concrete way to satisfy a Capability contract.

A **Capsule** is a materializable execution/package envelope for such an implementation. It may be:

- WASM/WASI;
- OCI/container;
- microVM;
- native component;
- local AI model;
- remote service adapter;
- device/hardware implementation.

Capsules are usually cacheable/evictable/disposable. Their lifecycle must not define the lifecycle of user state.

## 15. Compute Fabric / Node

The **Compute Fabric** is the set of nodes/resources available to the Personal World.

A **Node** advertises facts such as:

- CPU/GPU/RAM/storage;
- device/sensor capabilities;
- trust level;
- power/network state;
- location/data-residency constraints;
- supported runtimes;
- platform control depth;
- cost and availability.

A node can use a different substrate (NixOS/Linux, macOS, Android/GKI, future systems) while participating in the same Personal World.

## 16. BindingPlan

A **BindingPlan** is a complete concrete solution proposed for a RequirementGraph.

It specifies:

- selected Capability implementations/Capsules;
- Fabric nodes;
- converter/adapter paths;
- data routes;
- delegated grants;
- expected effects;
- objective scores;
- ResolutionTrace.

A solver proposal is not authority until the independent verifier accepts it.

## 17. BindingLease

A **BindingLease** is the scoped, time/condition-bounded commitment to a verified BindingPlan or subset of it.

It defines:

- validity conditions;
- delegated authority;
- safe rebind boundaries;
- selected implementations/nodes;
- expiration/checkpoints.

This prevents late binding from becoming arbitrary mid-effect migration.

## 18. Knowledge Object

A **Knowledge Object** is the primary persistent user-owned data abstraction with stable identity independent from one filename/application.

It may include:

- data/content;
- logical structure;
- semantics/metadata;
- provenance/confidence;
- relations;
- history.

Physical files remain valid storage/export/compatibility representations.

### Projection

A **Projection** is a typed, least-privilege semantic slice of an Object/resource.

### Representation

A **Representation** is a derived materialization such as PDF, DOCX, HTML, audio or thumbnail. It may be rebuilt and invalidated like a build artifact.

### View

A **View** is a saved semantic query/collection over Objects.

## 19. Event

An **Event** is a raw or normalized signal from kernel, filesystem, device, service, user, network, Capsule or external source.

Events are facts/signals, not semantic conclusions.

## 20. Situation

A **Situation** is a structured semantic interpretation of events plus temporal/contextual evidence.

Examples:

```text
battery regression after system change
user leaving while a long-running task is active
GPU driver repeatedly crashing on a particular workload
```

A Situation is evidence/input for planning. It does not itself grant authority.

## 21. Alfred / Situation Engine

**Alfred** is the event-driven nervous system.

Frozen responsibility:

```text
receive -> normalize -> correlate -> derive candidate Situation -> semantic interpretation -> publish Situation
```

Alfred may trigger rules and request planning, but it does not own generic planning, system maintenance policy or Capability authorization.

This separation is deliberate.

## 22. System Technician

The **System Technician** is a persistent specialized system-engineering role.

It consumes:

- system-relevant Situations from Alfred;
- explicit user requests;
- system state/telemetry;
- Temporal/Causal history;
- trusted external provenance through Improvement Watch.

It produces:

- diagnosis/explanation;
- ImprovementProposal;
- candidate Workspace/SystemSpec changes;
- test/benchmark plans;
- official-source references/provenance.

It does not directly possess root/authority.

## 23. ImprovementProposal

An **ImprovementProposal** is a structured proposal to change system/workspace/runtime state.

It should contain:

- local trigger/evidence;
- external provenance when used;
- applicability reasoning;
- expected benefit/uncertainty;
- risks;
- proposed changes;
- test/benchmark plan;
- authorization requirement;
- rollback reference;
- expiration/revalidation conditions.

## 24. AI Broker

The **AI Broker** resolves abstract reasoning requirements onto models/providers:

```text
resident local -> stronger local -> trusted Fabric -> optional cloud
```

Routing respects privacy, data residency, required quality, latency, cost, energy and availability.

The AI Broker is not an authority boundary.

## 25. Adaptive System

The **Adaptive System** is the mutable machine/substrate layer represented by declarative SystemSpec-like state where possible.

It includes:

- packages/build features;
- services/runtime;
- power/performance profiles;
- kernel parameters/builds/modules;
- drivers;
- node-specific optimization.

Changes follow branch/build/test/benchmark/activate/verify/commit-or-rollback semantics.

## 26. Constitutional Core

The **Constitutional Core** is the minimal trusted base that adaptive mechanisms cannot normally disable.

It owns/enforces:

- root identity/trust anchors;
- authorization/policy root;
- independent verification;
- recovery/known-good boot;
- rollback capability;
- audit integrity necessary to recover.

The Constitutional Core should make as few policy decisions as practical while preserving safety/recovery.

## 27. Temporal vs Causal state

The Blob distinguishes:

### Temporal state

Enough data to reconstruct an earlier state/version.

### Causal history

Why a meaningful state transition occurred:

```text
what
why
who/agent
trigger/evidence
expected effect
actual effect
side effects
authorization
ResolutionTrace
parents/branch/merge
rollback reference
```

The Temporal/Causal Graph is cross-cutting infrastructure, not one component that owns all system state.

## 28. Ownership matrix

| Concept | Owns persistent user data? | Owns executable implementation? | Owns presentation? | Owns authority? |
|---|---:|---:|---:|---:|
| Personal World | yes/references | no | no | identity/policy references |
| Workspace | context/references | no | grammar/profile refs | policy overlay only |
| Task | task state/refs | no | no | requested effects only |
| Knowledge Object | yes | no | no | no |
| Capability | no | no | no | no |
| Capsule | no | yes | no | no |
| Surface | small UI state | no | semantic presentation | no |
| Renderer | no | implementation | physical rendering | no |
| Alfred | event/situation history refs | no | no | no |
| System Technician | memory/proposals refs | no | may request Surface | no |
| Solver | no | no | no | no |
| BindingVerifier | no | no | no | validates granted authority |
| Constitutional Core | minimal trusted state | trusted implementation | recovery UI only | yes |

## 29. Cross-device invariant

```text
same Personal World
+ same Workspace identity
+ same Task identity
+ same Knowledge Object identity
+ same semantic state

may use

!= same process
!= same Capsule
!= same node
!= same renderer
!= same pixels
```

## 30. AI independence invariant

The Blob must still provide boot, recovery, user data access, deterministic resolution, existing Workspace state and ordinary system operation if every AI model/provider is unavailable.

AI expands understandability, diagnosis, planning and customization. It is not the substrate that keeps the machine alive.

## 31. Freeze rule

MVP-0 code should depend on these semantic boundaries rather than on prototype technologies. A proposed implementation that merges two frozen roles for convenience must document why it remains externally separable or introduce a new ADR changing this freeze.
