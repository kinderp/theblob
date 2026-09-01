# Blob Shell Demo v0.1 — Priority Plan

**Status:** accepted near-term product priority.  
**Goal:** produce a recognizably Blob-native experience early; do not wait for the complete Linux pilot or Fabric architecture.

## 1. Demo thesis

The first useful demo must answer one question:

> Does The Blob already feel like a new way to interact with a computer rather than a dashboard application?

The demo is successful if a user can launch it on the current macOS host, immediately understand that Workspaces are the primary objects, unfold one into useful Surfaces, invoke a real semantic system action and see The Blob explain what is happening.

It is **not** required to prove the complete Personal Fabric.

## 2. Priority rule

```text
P0 = required to make the first demo feel like The Blob
P1 = deepen the demo without changing its identity
P2 = prove the first real Fabric interaction
P3 = full pilot/future architecture
```

No P1/P2/P3 item may block P0 unless it exposes a fundamental architectural contradiction.

# P0 — Demo-critical

## P0.1 Replace the dashboard shell

Remove the current application/dashboard mental model from the primary experience:

- no permanent left application-like navigation rail;
- no permanent Technician sidebar;
- no `Now / Workspaces / History / Fabric` app-page hierarchy as the dominant visual grammar.

Create the Blob-native shell:

- CLI/TUI-style top bar;
- large central world/space;
- bottom global prompt;
- keyboard-first focus/navigation.

**Acceptance:** on launch it reads as an operating environment, not as a settings/dashboard application.

## P0.2 Four Workspace Blob avatars

Initial visible Workspaces:

- Dev/Romeo — friendly Blob with yellow work helmet;
- Docs — friendly Blob with glasses;
- System — technician/mechanic identity;
- Notes — writing identity.

The avatars are Workspace identities, not app icons.

**Acceptance:** focus/click/keyboard navigation works and each avatar has a distinct recognizable role.

## P0.3 Blob mode -> Tile mode

At least one Workspace (preferably Dev/Romeo) must visibly unfold into a Surface composition.

Initial roles can be small but semantically named:

```text
Editor | Docs
-------+------
Terminal | Tests
```

The first implementation does not need a complete editor/browser/terminal product. It must demonstrate that these are Surface roles belonging to one Workspace.

**Acceptance:** expand/collapse works without navigating to an app-style page.

## P0.4 Basic SurfaceInstance model

Introduce the minimum renderer-neutral data model required to avoid hard-coding the demo into Slint:

- Workspace identity;
- Surface identity/role;
- SurfaceInstance identity;
- current local SurfaceHost placement;
- layout slot/state sufficient for the demo.

Do **not** build remote placement yet.

**Acceptance:** Slint renders the model rather than defining Workspace semantics itself.

## P0.5 One real semantic system action

Reuse the existing System Workspace Bluetooth path.

From the System Workspace/Surface:

```text
Bluetooth OFF
 -> request
 -> semantic proposal OFF -> ON
```

The UI must continue to avoid faking `ON` until the real effect has crossed the backend/authority path.

For the first macOS visual demo, materialization/activation may remain unavailable; the semantic proposal is still real and inspectable.

**Acceptance:** GUI action calls `blob-system-workspace` rather than toggling local UI state.

## P0.6 Contextual Technician

Replace the persistent right panel with an overlay/hint interaction.

Minimum interactions:

```text
Why?
Explain
Show details
```

Use existing evidence-backed Technician projections where possible.

**Acceptance:** the Technician feels like intelligence of the computer rather than a chat application.

## P0.7 Global prompt shell

The prompt must exist visually and accept at least a tiny controlled set of structured commands/intents, for example:

```text
> system
> romeo
> bluetooth
> why
```

Natural-language AI is **not required** for P0. A deterministic command mapping is preferable to a fake LLM experience.

**Acceptance:** prompt and graphical action converge on the same navigation/semantic intent layer where applicable.

## P0.8 Essential shortcuts

Minimum target grammar:

```text
Cmd+K       focus/open global prompt
Cmd+1..4    focus Workspace
Cmd+T       Blob <-> Tile for focused Workspace
?           explain/help focused semantic object/context
Esc         close overlay / return focus
```

Exact platform bindings may be adjusted to avoid macOS conflicts.

# P0 explicit non-goals

Do not delay the first demo for:

- remote Fabric nodes;
- real cross-device drag/drop;
- semantic clipboard;
- file federation;
- custom Wayland compositor;
- Hyprland integration;
- AI-generated layouts;
- application adapters;
- full embedded editor/terminal/browser;
- mobile/watch clients;
- history/time travel;
- credential delegation;
- object/content store;
- live NixOS activation from macOS;
- production animation/polish.

# P1 — Make the local shell genuinely usable

P1 starts only after P0 is directly runnable and visually reviewed.

Priorities:

1. move/resize/re-tile SurfaceInstances;
2. persist local Workspace layout;
3. compact/standard/rich Surface experience according to available size;
4. SemanticSelection and contextual action strip;
5. richer Dev Surface backed by the existing development vertical slice;
6. System Surface with proposal progress/status;
7. History as a small causal activity Surface using existing records;
8. one hosted legacy application experiment if it can be integrated without compositor work.

# P2 — First real Fabric proof

The first Fabric demo should be deliberately tiny: **two nodes only**.

Preferred test topology:

```text
MacBook
  Blob Shell / current interaction node

NixOS VM or second Linux node
  Blob node / execution or Surface target
```

Required proof points:

1. both nodes appear as real discovered/configured Fabric devices;
2. Presence/Placement view shows where something lives;
3. one Surface or semantic activity can be opened/placed on the second node;
4. one simple object/clipboard or execution operation crosses nodes;
5. user can explicitly choose/override target;
6. no master secret is copied merely to make the demo work.

Do not attempt general peer-to-peer orchestration before this two-node slice works.

# P3 — Full pilot expansion

Only after P0/P1/P2:

- richer CapabilityOffer/FabricView model;
- unified resource view across devices;
- semantic clipboard and object placement;
- application adapters;
- adaptive phone/watch SurfaceInstances;
- distributed Workspace optimization;
- credentials/delegation;
- causal snapshot/time travel;
- AI-designed layouts;
- native Linux session/Hyprland integration;
- possible future Blob compositor.

## 3. Demo sequence the user should experience

The near-term runnable demo should feel approximately like this:

```text
launch
  |
  v
Blob-native TUI shell
  |
  +-- Dev/Romeo Blob
  +-- Docs Blob
  +-- System Blob
  +-- Notes Blob
  |
select Romeo
  |
Cmd+T / activate
  |
Romeo unfolds into tiles
  |
return/collapse
  |
open System
  |
Bluetooth OFF -> proposal OFF -> ON
  |
Why?
  |
Technician explains semantic/evidence state
  |
Cmd+K
  |
prompt invokes a shell/system intent
```

If this sequence is compelling, the demo has succeeded even though the complete Fabric is still future work.

## 4. Engineering guardrails

- No renderer-owned semantic truth.
- No raw Nix emitted by UI.
- No fake backend state for demo theater.
- No new general framework unless needed by a P0 vertical slice.
- Prefer one real end-to-end interaction over five static mock screens.
- Keep current CI green.
- Keep GUI work layered on the System Workspace branch until the dependency is resolved, then rebase/retarget cleanly.

## 5. Demo completion definition

P0 is complete when:

- the current Mac can run the shell with one command;
- it visually matches the Blob-native CLI/TUI + playful avatar direction;
- the user can navigate/focus Workspaces without an app-style sidebar;
- one Workspace expands/collapses into multiple Surface roles;
- the global prompt is interactive;
- the Technician is contextual;
- Bluetooth produces a real semantic proposal;
- tests/CI protect the renderer-neutral intent boundary.

At that point the demo should be shown and used before adding further architecture.
