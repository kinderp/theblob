# ADR 0002 — Blob Shell as Semantic Compositor

**Status:** Accepted  
**Date:** 2026-08-29

## Context

The first Slint prototype proved that renderer-neutral intent handling can be connected to a real GUI, but visually it behaved like a control/dashboard application. The desired product experience is instead an operating environment: the user should already be inside The Blob, with Workspaces unfolding into interactive Surfaces and a global CLI/TUI-style interaction grammar.

Building a new Wayland compositor now would delay the product without validating the novel semantic model.

## Decision

The Blob Shell is a **semantic compositor**, not necessarily the low-level platform compositor.

It owns semantic composition decisions:

- active/focused Workspace;
- which Surfaces should be instantiated;
- logical layout/composition;
- SemanticSelection;
- global prompt/Intent routing;
- Workspace/Fabric presence summary;
- contextual Technician manifestations.

Concrete rendering/window/input behavior belongs to a replaceable **SurfaceHost**.

```text
Blob Shell
  semantic composition
        |
SurfaceHost
  concrete instances, tiling, input, rendering
        |
platform graphics/windowing
  Slint/winit, Wayland/Hyprland, macOS, Android, ...
```

Slint remains the first Blob-native SurfaceHost/renderer path.

## Interaction grammar

The Blob-native profile should prefer a semi-graphical CLI/TUI identity:

- compact persistent top bar;
- Blob avatars representing Workspaces;
- Blob mode <-> Tile mode;
- keyboard-first navigation;
- global semantic prompt;
- contextual action text/strips rather than heavy app-style navigation;
- Technician as contextual intelligence rather than a permanent chat sidebar.

GUI actions, keyboard shortcuts, prompt requests, drag/drop and future expert CLI forms must compile into the same structured Intent model.

## Applications

Applications are compatibility and implementation providers, not the primary shell unit.

Supported integration levels are:

1. legacy full application window;
2. hosted Surface;
3. semantic application adapter;
4. Blob-native Surface.

The user may always request the full traditional application when possible.

## Consequences

- The shell can feel like an OS before The Blob owns Wayland/compositor internals.
- Hyprland/Sway/macOS-native/Android hosts remain viable.
- Surface layout semantics are portable across renderer implementations.
- Existing applications can be adopted incrementally rather than rewritten.
- Tiling/drag/resize belong to SurfaceHost/Layout Engine, not each Surface implementation.

## Non-decision

This ADR does not forbid a future Blob-native Wayland compositor. Such a compositor becomes justified only if semantic Surface management requires capabilities that cannot be implemented cleanly through replaceable hosts/integrations.

## Rejected alternatives

### Rebuild the first prototype as a larger dashboard

Rejected because it reinforces the wrong application/control-panel mental model.

### Build a custom compositor first

Rejected because it validates generic window-system engineering before validating The Blob's differentiating Workspace/Surface/Fabric interaction model.

### Make the AI chat surface the center of the shell

Rejected because The Blob should behave as an intelligent computer, not a conventional desktop with a chatbot attached.
