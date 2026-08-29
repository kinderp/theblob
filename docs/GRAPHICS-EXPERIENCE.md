# Graphics and Experience Model v0.2

**Status:** accepted design direction.

Canonical shell details live in [`BLOB-SHELL-v0.1.md`](./BLOB-SHELL-v0.1.md). The terminology decisions are recorded in ADR 0001–0003 under `docs/adr/`.

## Goal

The Blob must feel like one computer across devices without forcing every device to look identical. The logical experience is portable; concrete SurfaceInstances and renderers may be Blob-native or platform-native.

The user should experience a semantic operating environment rather than a dashboard application.

## Model

```text
Personal World
     |
Workspace
     |
Experience Grammar
     |
Surface(s)
     |
SurfaceInstance(s)
     |
Experience Profile
     |
SurfaceHost / Renderer
     |
Platform graphics/windowing stack
```

### Experience Grammar

Stable interaction semantics for a Workspace: navigation/focus model, Surface roles, contextual actions, command model, keyboard/gesture policy, density rules and persistent interaction preferences.

### Surface

Persistent typed interactive role inside a Workspace. Examples include `Editor`, `ObjectNavigator`, `Docs`, `Terminal`, `Tests`, `SystemHealth`, `TaskStatus`, `Timeline` and `Comparison`.

A Surface is semantic and device-independent. It may exist with no visible instance and may have multiple simultaneous instances.

### SurfaceInstance

Concrete manifestation of a Surface on one SurfaceHost/device/context.

An instance may own local presentation state such as bounds, compact/rich profile and panel expansion while sharing semantic Surface state with other instances.

The term `Projection` is reserved for its existing Blob meaning: a typed semantic selection over an object/resource.

### Experience Profile

Determines how a SurfaceInstance is presented and interacted with on a platform/device/context.

Examples:

```text
Romeo / Editor Surface
    + macos-native
    + MacBook instance

Romeo / Editor Surface
    + hyprland-keyboard-first
    + Linux desktop instance

Docs Surface
    + blob-native-compact
    + tablet instance
```

## Blob Shell as semantic compositor

The Blob Shell owns semantic composition:

- focused Workspace;
- which Surfaces should have instances;
- logical Workspace layout;
- SemanticSelection and contextual action routing;
- prompt/Intent integration;
- Fabric/presence summary.

It does not need to own low-level platform compositing.

```text
Blob Shell
     |
SurfaceHost / Layout Engine
     |
Slint/winit, Hyprland/Wayland, macOS, Android, ...
```

This allows the product interaction model to be proven before building a custom compositor.

## Blob-native shell grammar

The first Blob-native profile targets a semi-graphical CLI/TUI visual language:

- compact persistent top status bar;
- playful Workspace Blob avatars;
- Blob mode <-> Tile mode;
- tiling-friendly Surface composition;
- bottom/global semantic prompt;
- keyboard-first interaction;
- contextual Technician, not a permanent chatbot panel.

The same structured Intent layer should be reachable from GUI actions, shortcuts, prompt requests and drag/drop.

## Renderer / SurfaceHost families

### Blob Native

Initial implementation: **Slint**.

Purpose: coherent Blob visual identity across supported platforms, especially for Blob Shell, Workspace construction, System Technician manifestations, Object Browser, Timeline, settings and native Blob Surfaces.

Slint is a renderer/host choice, not semantic authority.

### macOS Native

Potential SurfaceHost/renderer: SwiftUI/AppKit plus macOS-native windowing and integrations.

Goals:

- native menu/shortcut conventions where useful;
- native gestures and window management;
- notifications, drag/drop and system integration;
- preserve Blob Workspace/Surface semantics underneath.

### Linux / Hyprland

Initial Linux integration target:

```text
Blob semantic Shell / Workspace model
        |
SurfaceHost + Blob UI components
        |
Hyprland integration
        |
Wayland
        |
Mesa / DRM / KMS / Linux drivers
```

Hyprland is a first-class Experience integration target, not a permanent architectural dependency.

Longer-term option:

```text
Blob semantic Shell
       |
Blob SurfaceHost
       |
Blob compositor (Rust + Smithay)
       |
Wayland / DRM / KMS
```

This becomes worthwhile only if semantic Surface management requires compositor-level control that existing integrations cannot provide cleanly. A custom compositor is explicitly not required for the first shell demo.

### Android Native

A future host may use Android-native UI APIs for phone/tablet SurfaceInstances while consuming the same semantic Surface model.

## Stable experience vs generative adaptation

The Blob does not regenerate the whole UI arbitrarily. Muscle memory and predictability are system requirements.

```text
stable Experience Grammar
          +
versioned Experience Profile
          +
contextual temporary SurfaceInstances
```

AI may propose or instantiate contextual panels/layouts inside declared schema boundaries, but stable shortcuts/layout grammar change only through explicit/authorized Workspace evolution.

The user remains free to rearrange and override generated layouts.

## Ready / AI Designed / Expert

Experience Profiles participate in all three Workspace construction modes.

### Ready

Choose a curated combination such as:

- Development / Blob Native Balanced;
- Development / Hyprland Keyboard-first;
- Development / macOS Native.

### AI Designed

The Workspace Architect derives a profile/layout from hardware, displays, input devices, habits and user priorities, then previews relevant trade-offs before adoption.

### Expert

Users may directly control renderer/integration, layout grammar, animation policy, keyboard/gesture conventions, density, rendering priorities, SurfaceInstance placement and legacy-window behavior.

These modes may be mixed by domain; they are not permanent user skill levels.

## Legacy applications

Existing applications remain first-class compatibility citizens.

The shell recognizes increasing levels of integration:

1. **Legacy window** — complete application managed as a placement unit;
2. **Hosted Surface** — application/window associated with a semantic Surface role;
3. **Semantic adapter** — application exposes useful state/capabilities to the Blob;
4. **Blob-native Surface** — role is rendered directly without exposing a traditional application.

On Linux, Wayland/XWayland clients can therefore participate without being rewritten. On macOS, native applications can be associated with Workspaces/Surfaces while keeping their native UI.

The user may request the full traditional application whenever technically possible.

## Multi-device experience

A Workspace is logically independent from a physical device but does not need to be distributed.

SurfaceInstance placement, execution placement, data placement and current interaction presence are separate dimensions. A Workspace may be local, handed off to another interaction device, or selectively distributed when this is useful or explicitly requested.

The Fabric must always make significant placement inspectable and overridable.

## Core invariants

```text
same Workspace semantics
+ same Tasks/Capabilities
+ same persistent Surface roles
!= same SurfaceInstances
!= same pixels
```

and:

```text
Workspace != application
Workspace != window
Surface != SurfaceInstance
SurfaceInstance placement != execution placement
renderer != authority
```

The Blob synchronizes meaning and state, not screenshots, whenever a semantic Surface path is available.
