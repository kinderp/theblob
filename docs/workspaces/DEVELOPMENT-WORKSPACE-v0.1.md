# Development Workspace v0.1

**Status:** first concrete Workspace specification for MVP-0 and subsequent phases.

The Development Workspace is intentionally the first real Workspace because it exercises most of The Blob's architecture in a compact domain: persistent context, source objects, terminal/build/test capabilities, Alfred events, System Technician diagnosis, ephemeral Capsules, AI-assisted work, causal history and multi-device Surfaces.

## 1. Workspace identity

```text
kind: workspace.development
version: 0.1
```

A Development Workspace may correspond to one project, repository, course/lab or broader development context.

It is not an IDE installation.

## 2. Persistent Workspace state

The Workspace persists/references:

- project/repository identities;
- relevant Knowledge Objects and Views;
- current branch/context references;
- Task set and checkpoints;
- Experience Grammar;
- selected Experience Profiles per device/platform;
- baseline Capability requirements;
- project/toolchain preferences;
- policy/privacy/authority overlays;
- terminal/task history references where retention policy allows;
- System Technician project-specific observations/proposals;
- causal links to important changes.

It does not persist Capsule process state as the source of truth.

## 3. Experience Grammar v0.1

The default semantic roles are:

```text
ObjectNavigator
PrimaryEditor
TaskNavigator
TerminalOrExecutionView
TechnicianPanel
StatusArea
ContextualPanel
```

The grammar says **what roles exist and how the user moves among them**, not how many pixels each panel occupies.

Default interaction rules:

- primary content remains visually stable while contextual panels appear/disappear;
- Task status is always reachable without leaving the Workspace;
- System Technician may surface evidence/proposals contextually but cannot steal focus for non-urgent suggestions;
- execution output is tied to Tasks/Bindings, not to anonymous terminal windows;
- keyboard-first navigation is available even when the Experience Profile is platform-native;
- legacy tools may appear as `LegacySurface` providers.

## 4. Ready mode

Ready mode answers:

> "Give me a development environment that works well now."

### Ready / Balanced recipe

Baseline semantic roles:

- source/object navigation;
- editor;
- terminal/execution panel;
- Git/status integration;
- Task view;
- System Technician panel;
- command palette/search;
- diagnostics.

Baseline Capability **requirements**:

```text
text.edit
source.navigate
terminal.session
git.status
git.diff
git.commit
test.run
build.run
diagnostics.read
```

Language/toolchain-specific capabilities are **not** all installed permanently. They are resolved when project context requires them.

Example for a Python project:

```text
python.runtime
python.language-analysis
python.format
pytest.run
```

Example for Rust:

```text
rust.compiler
rust.language-analysis
cargo.build
cargo.test
```

The Ready recipe can choose curated, tested implementations but must keep contracts abstract enough to replace implementations later.

### Ready experience variants

Examples:

```text
Development / Blob Native Balanced
Development / Hyprland Keyboard-first
Development / macOS Native
```

The same Workspace/Task state survives switching Experience Profile.

## 5. AI Designed mode

AI Designed mode answers:

> "Build the development environment around how I actually work."

The Workspace Architect/System Technician may inspect authorized facts such as:

- CPU/GPU/RAM/display topology;
- keyboard/mouse/trackpad/input devices;
- OS/substrate capabilities;
- active languages/toolchains;
- project size/build characteristics;
- user interaction preferences;
- historical Workspace usage;
- performance/battery objectives;
- privacy and network policy.

It produces a **WorkspaceDesignProposal**, never an unreviewed arbitrary configuration.

The proposal should contain:

```text
selected Experience Grammar/Profile
baseline capability requirements
candidate implementations where pinning is justified
resource/memory estimate
startup/warm-state estimate
performance trade-offs
battery/energy trade-offs
privacy/security implications
legacy integrations
what remains JIT/ephemeral
rollback/reference Recipe version
```

Example user request:

> "I use Rust and Python, have an ultrawide monitor, live in the terminal, want the AI on the right, maximum keyboard control, fast builds and little visual clutter."

Possible derived proposal:

```text
ExperienceProfile: hyprland-keyboard-first
ObjectNavigator: collapsible/semantic
PrimaryEditor: central
TechnicianPanel: contextual-right
ExecutionView: persistent-bottom
GitDiff: fast-access contextual
animations: reduced
build placement preference: workstation/server
battery policy: balanced unless benchmark Task
```

The AI may benchmark alternatives where measurable rather than claiming an optimization without evidence.

## 6. Expert mode

Expert mode answers:

> "Expose the composition and let me control it."

The expert user may control:

### Workspace composition

- semantic roles/components;
- default/pinned Capability requirements;
- policy overlays;
- task/event rules;
- Experience Grammar;
- Experience Profiles by device/context.

### Implementation preferences

- preferred editor implementation;
- language server implementation/version constraints;
- compiler/build profile;
- WASM/OCI/native runtime preference;
- isolation level;
- local/remote placement constraints;
- network/data residency rules;
- cache/pinning/eviction behavior.

### Deep system optimization

Through the System Technician/Adaptive System, Expert mode may request:

- compiler flags;
- feature/build flags;
- CPU-specific builds;
- LTO/PGO experiments;
- service/runtime changes;
- power profiles;
- kernel parameters/modules;
- eventually kernel/driver variants.

These remain experiment branches with validation/benchmark/rollback semantics; Expert mode does not disable the Constitutional Core.

## 7. Capability lifecycle

The Development Workspace distinguishes three classes.

### Baseline requirements

Capabilities required for the Workspace to remain usable, e.g. editing/navigation/task status.

They may be warm/pinned, but the Workspace still references contracts rather than executable identity.

### Contextual requirements

Capabilities acquired when project context needs them:

```text
open Python project -> python analysis/runtime capabilities
open Rust project   -> Rust capabilities
open Kubernetes config -> cluster inspection capabilities
```

### Task-ephemeral requirements

Materialized for one Task and then released/evicted according to policy:

```text
profiling
one-off document conversion
special benchmark tool
security scanner
migration utility
```

## 8. First MVP-0 Task

Canonical scenario:

```text
User edits source file
       |
filesystem/repository event
       |
Alfred normalizes event
       |
candidate Situation: relevant source changed
       |
Task requests test.run
       |
RequirementGraph(one capability role initially)
       |
deterministic candidate derivation
       |
selected implementation/node
       |
independent BindingVerifier
       |
short BindingLease
       |
ephemeral test Capsule execution
       |
structured TestResult
       |
Task state update
       |
Surface update
       |
Temporal/Causal record
```

The MVP must prove that destroying the test Capsule does not destroy the Workspace, Task or test-result history.

## 9. System Technician in the Workspace

The Technician is not merely a chat sidebar.

It can consume project/system Situations such as:

```text
build became slower after toolchain change
repeated compiler crash
new upstream compiler fixes active issue
battery/thermal regression during builds
test runtime dominated by one resource bottleneck
```

It may propose:

- changing a Capability implementation;
- changing build profile;
- moving build execution to a Fabric node;
- updating a compiler/toolchain;
- adjusting system/runtime configuration;
- creating a benchmark branch;
- linking official upstream documentation/release notes.

All privileged/system changes follow the System Technician safety model.

## 10. Cross-device Surfaces

### Desktop / ultrawide

```text
Objects | Primary Editor | Technician
        |                |
        +----------------+
Execution / Tests / Task timeline
```

### Laptop

```text
Objects | Primary Editor
        |
Execution drawer
Technician contextual drawer
```

### Phone

Primary roles:

- Task status;
- code/diff review;
- Technician explanation;
- approve/reject safe proposed actions;
- build/test progress.

### Watch

Primary roles:

- completion/failure status;
- urgent warning;
- one constrained approval/action where policy explicitly permits.

No mobile Surface changes the identity of the Workspace or Task.

## 11. Legacy development tools

Existing IDEs/editors remain supported.

Examples:

```text
JetBrains / VS Code / Xcode / terminal app
        -> LegacySurface or Capability provider
        -> associated with Development Workspace
```

The Blob may progressively expose semantic adapters around existing tools, but adoption is not blocked on rewriting them.

## 12. State ownership test

The following experiment must succeed:

1. create Development Workspace;
2. perform Task with a selected test implementation;
3. destroy/evict the Capsule;
4. move to another compatible node;
5. reconstruct required implementations;
6. continue the same Task/Workspace with preserved semantic state.

If this fails because state was trapped in an implementation, the architecture has regressed toward applications.

## 13. Recipe evolution

Workspace Recipes are versioned:

```text
dev-recipe/v1
      |
      +-- v2: remove unused minimap role
      |
      +-- v3: add fast-access Git comparison role
```

AI Designed changes are proposals with causal evidence. Expert changes are still versioned. Ready recipes can receive upstream updates and be merged/forked rather than overwriting user customization.

## 14. Success criteria for v0.1

The specification is successful if the implementation can demonstrate:

- Workspace identity independent from editor/runtime implementation;
- Ready, AI Designed and Expert as composition modes, not product editions;
- Experience Profile independence from Workspace semantics;
- baseline vs contextual vs ephemeral Capability lifetimes;
- one Task resolved through RequirementGraph/BindingLease;
- Alfred event -> Situation -> Task reaction;
- Capsule destruction without state loss;
- System Technician explanation/proposal without direct authority;
- Temporal/Causal evidence for the meaningful transition.
