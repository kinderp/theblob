# The Blob Shell v0.1

**Status:** accepted canonical GUI/interaction direction.

This document freezes the user-facing model discussed for the first Blob Shell. It is intentionally broader than the first runnable demo but narrower than the complete Personal World architecture.

## 1. Product statement

> You do not open The Blob. You are already inside it.

The Blob Shell is not a dashboard application, settings app, launcher, or chatbot attached to a desktop. It is the semantic shell through which the user experiences Workspaces, Surfaces, capabilities and the Personal Fabric.

The shell should feel terminal-first, tiling-friendly, alive and playful, while remaining understandable to someone coming from a conventional desktop.

## 2. Core object model

```text
Workspace
    |
    +-- Surface
    |      |
    |      +-- SurfaceInstance -> SurfaceHost -> renderer/platform
    |
    +-- Tasks / capability requirements
    +-- objects / context / history
    +-- FabricView
```

### Workspace

A persistent human context: project, activity or domain of work. It is not a window, process or application.

Examples: `Romeo`, `Raiatea`, `Docs`, `System`, `Notes`.

A Workspace may continue to exist with no visible SurfaceInstance.

### Surface

A persistent typed interactive role inside a Workspace, such as `Editor`, `Docs`, `Terminal`, `Tests`, `SystemHealth`, `GitStatus` or `Notes`.

A Surface owns semantic interaction state relevant to its role. It does not own executable implementations and it is not inherently tied to one device.

### SurfaceInstance

A concrete presentation of a Surface on one SurfaceHost/device/context.

A Surface may have zero, one or many simultaneous SurfaceInstances. Instances may use different Experience Profiles and may keep a mix of shared Surface state and instance-local presentation state.

`Projection` keeps its existing Blob meaning: a typed semantic selection over an object/resource. It is **not** used as the name for a concrete Surface presentation.

### SurfaceHost

The replaceable host that realizes SurfaceInstances on a platform. Slint is the first Blob-native host/renderer path; future hosts may target Wayland/Hyprland, macOS native UI, Android, web or a Blob compositor.

## 3. Blob avatars

A Workspace may be represented in compact form by a **Blob avatar**.

The avatar is not an application icon. It is the compact identity/presence of the Workspace.

The visual identity should be playful and differentiated by role. Examples:

- Dev/Romeo: friendly Blob with a yellow work helmet;
- Docs: Blob with glasses/book-like cues;
- System: technician/mechanic cues;
- Notes: writing/pencil cues.

The avatar may expose a tiny amount of state such as health, task count or device presence, but must not become a miniature dashboard.

## 4. Blob mode and Tile mode

A Workspace can move between at least two shell presentations.

### Blob mode

Compact identity and status. Optimized for overview and switching.

### Tile mode

The Workspace unfolds into a composition of its current relevant Surfaces.

Example:

```text
Romeo

+------------------+------------------+
| Editor           | Docs             |
+------------------+------------------+
| Terminal         | Tests / Git      |
+------------------+------------------+
```

Blob -> Tile does not launch an application. It changes the shell presentation of an existing Workspace and creates/focuses SurfaceInstances as required.

## 5. Layout model

The shell is a **semantic compositor**. It decides which Workspace/Surfaces should be present and their logical composition; it is not required to be the platform compositor.

The SurfaceHost/Layout Engine owns concrete move, resize, tile, split, stack, collapse and focus behavior.

The user has final control. Automatic layout is assistance, not authority.

Layouts are persistent Workspace/Experience state and should eventually support:

- drag and resize;
- tiling and snapping;
- collapsing a Workspace back into its Blob;
- saved layout variants;
- device/context-specific arrangements;
- AI-proposed arrangements that remain user-editable.

A resize may select a different Experience Profile (`compact`, `standard`, `rich`) rather than merely scaling the same pixels.

## 6. Global shell grammar

The first Blob-native shell uses a semi-graphical CLI/TUI visual language.

### Persistent top bar

Compact global state, for example:

```text
blob@os | 1 2 3 4 | fabric:1 | CPU 12% | MEM 34% | NET | BAT 84% | 09:21
```

It may expose Workspace focus/presence, Fabric status and essential machine state without becoming a monitoring dashboard.

### Global prompt

A prompt is always quickly reachable and may remain visible at the bottom in the Blob-native profile:

```text
>_
```

It is not a chat box. It is the universal semantic command/intent surface.

The prompt receives the current InteractionContext implicitly: Workspace, focused Surface, SemanticSelection, current device/Fabric state and relevant task context.

Examples:

```text
> build
> why?
> move to notebook
> summarize
> enable bluetooth
```

GUI actions, prompt requests, shortcuts and drag/drop must compile into the same structured Intent model rather than separate ad-hoc command paths.

## 7. SemanticSelection and contextual actions

Every meaningful object shown by a Surface should be addressable semantically when practical.

Examples include:

- Document/ObjectRef;
- SourceLocation;
- Task;
- Capability;
- Workspace;
- Surface;
- Fabric node/resource;
- Git object;
- system setting/state.

Selection is semantic, not merely pixels/text.

The actions offered by the shell derive from object type + Workspace context + available capabilities + policy. The application that happens to render an object does not own the action vocabulary.

Examples:

```text
SourceLocation -> Open / Explain / Test
Document       -> Read / Summarize / Translate / Move
Tests Surface  -> Run / Logs / Move execution / Explain
Fabric node    -> Inspect / Use / Wake / Trust details
```

Deterministic recognition and typed objects come before AI interpretation. AI may resolve ambiguity or compose intents but does not replace structured validation.

## 8. Applications inside Surfaces

Applications remain fully supported but cease to be the primary unit of user context.

A Surface may be implemented at different integration levels:

1. **Legacy window** — the complete application is managed only as a shell placement unit.
2. **Hosted Surface** — an application/window is associated with one semantic Surface role.
3. **Semantic adapter** — the application exposes useful Blob capabilities/state through an adapter.
4. **Blob-native Surface** — the application may not be visible at all; the Surface directly exposes the relevant capability/content.

Examples:

```text
Editor Surface -> Zed / Neovim / Blob-native implementation
Web Surface    -> Firefox adapter / full Firefox / native reader
3D Surface     -> full Blender when its complete UI is appropriate
Git Surface    -> Blob-native status/actions backed by git
```

The user can always choose the concrete application/implementation when technically possible.

## 9. Technician interaction

The System Technician is intelligence of the system, not a permanent chatbot panel.

Default manifestations are contextual hints, explanations, proposals and short-lived Surfaces:

```text
Bluetooth failed. I found the cause.
[Fix it] [Show me]
```

or:

```text
Battery 8%. I can reduce power use.
[Do it] [Why?]
```

A full conversational Surface can appear when requested, but it is not the visual center of the OS.

The Technician remains non-authoritative: suggestions and explanations compile into semantic proposals and cross the normal verification/authority boundary.

## 10. Fabric: two distinct jobs

The Fabric has two related but distinct responsibilities.

### Placement

Where SurfaceInstances, tasks, capability executions and data are placed.

### Federation

How resources from several nodes are presented as one personal computer: files/objects, clipboard, storage, compute, displays, audio, cameras, services and other CapabilityOffers.

The normal user model is:

> the computer is the Personal Fabric, not necessarily the physical device in front of the user.

A Workspace may nevertheless use only one local node; distribution is never required for its own sake.

## 11. Workspace distribution modes

A Workspace is logically independent from a device but should use the simplest placement satisfying the user's intent.

Natural modes are:

- **Local** — interaction, Surfaces and execution remain on the current node;
- **Handoff** — interaction presence and appropriate SurfaceInstances move to another node while remote work may remain where it is;
- **Distributed** — selected SurfaceInstances/tasks/capabilities/data use different nodes because there is a concrete benefit or the user explicitly requests it.

Distribution is a means, not a goal.

## 12. Placement dimensions

The following are distinct and independently inspectable:

- **Interaction presence** — the device the user is primarily using now;
- **Surface placement** — where each SurfaceInstance is visible;
- **Execution placement** — where tasks/capability implementations run;
- **Data placement** — where objects/content are stored, cached or replicated.

Example:

```text
Romeo

interaction: MacBook
Editor SurfaceInstance: MacBook
Docs SurfaceInstance: Tablet
Tests execution: Desktop
Containers: Home Server
working-set cache: MacBook
```

The system must never make important placement invisible. A Presence/Placement view must make it easy to inspect and override.

## 13. Fabric drag and drop

Dragging an object onto a Fabric device is semantic. The visible gesture may be identical while the Intent differs by object type.

Examples:

```text
Workspace Blob -> Notebook
  make Notebook the interaction point / open suitable SurfaceInstances

Docs Surface -> Tablet
  create/move a Docs SurfaceInstance

Tests -> Desktop
  place test.run execution on Desktop

report.pdf -> Desktop
  make the Document available there (stream/cache/replicate/move according to policy)
```

During drag only valid targets should be emphasized and the intended action must be previewed. Ambiguous operations offer choices such as `Open here`, `Move here`, `Mirror`, `Companion`.

## 14. Unified and device Fabric views

The shell should eventually expose multiple views of the same Fabric:

- **Unified** — aggregate resources: compute, GPU, storage, displays, clipboard, services;
- **Devices** — PC/notebook/phone/watch/server and their state;
- **Placement** — where current SurfaceInstances/tasks/data actually live.

The normal experience remains simple; detailed placement is progressive disclosure.

## 15. User sovereignty

The user has the final word.

The Blob may recommend safer/faster/lower-energy placement, applications, layouts or system configuration, but recommendation is not prohibition.

Only real constraints (technical impossibility, unavailable capability, explicit security/authority boundary, externally imposed policy) block an operation, and the reason must be explainable.

Autonomy is delegated by domain and is revocable. `automatic` means the user granted bounded authority, not that The Blob owns the decision.

## 16. System changes remain semantic

A graphical toggle is an Intent, not a direct mutation.

```text
Bluetooth OFF
    -> user intent
    -> System Workspace proposal
    -> SystemSpec
    -> backend translation
    -> candidate/materialization
    -> verification
    -> authority
    -> observed real state ON
```

Until the effect is real, the UI must not fake the final state.

## 17. History and state

Workspace and Surface state are persistent independently from window/process lifetime. The shell should eventually support semantic checkpoints, causal history and selective restore, but history is not required for the first visual demo.

`WorkspaceSnapshot != runtime checkpoint` and `Object identity != object placement` remain architectural requirements for later work.

## 18. Non-goals for the first Shell implementation

The first runnable Blob Shell does **not** require:

- a custom Wayland compositor;
- full multi-node Fabric;
- live process/VM migration;
- a distributed object store;
- universal application adapters;
- complete semantic clipboard;
- full history/time travel;
- AI-generated UI;
- mobile/watch renderers.

These are compatible with this model but must not delay the first useful demo.

## 19. North-star invariants

```text
Workspace != application
Workspace != window
Workspace != device
Surface != application
Surface != SurfaceInstance
SurfaceInstance placement != execution placement
Object identity != object location
Fabric != mandatory distribution
AI intent != execution authority
recommended != required
```
