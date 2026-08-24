# Graphics and Experience Model v0.1

**Status:** accepted design direction.

## Goal

The Blob must feel like one computer across devices without forcing every device to look identical. The logical experience is portable; the renderer may be Blob-native or platform-native.

## Model

```text
Personal World
     |
Workspace
     |
Task / Context
     |
Experience Grammar
     |
Surface Model
     |
Experience Profile
     |
Renderer
     |
Platform graphics/windowing stack
```

### Experience Grammar

Stable interaction semantics for a Workspace: navigation placement, main content roles, contextual panels, command model, keyboard/gesture policy, density and persistent interaction preferences.

### Surface Model

Typed semantic description of what should be presented on the current device/context. It contains roles such as `Editor`, `ObjectNavigator`, `TechnicianPanel`, `TaskStatus`, `Timeline`, `ComparisonPanel` rather than pixel coordinates.

### Experience Profile

Determines how that semantic Surface is presented on a platform/device.

Examples:

```text
Development Workspace
    + macos-native
    + MacBook

Development Workspace
    + hyprland
    + Linux desktop

Writing Workspace
    + blob-native
    + either platform
```

## Renderer families

### Blob Native

Initial renderer: **Slint**.

Purpose: coherent Blob visual identity across supported platforms, especially for Blob Shell, Workspace Builder, System Technician, Object Browser, Timeline, settings, command palette and native Blob components.

### macOS Native

Potential renderer: SwiftUI/AppKit plus macOS-native windowing and platform integrations.

Goals:

- native menu/shortcut conventions;
- native gestures and window management;
- notifications, drag-and-drop and system integration;
- preserve Blob semantics/state underneath.

### Linux / Hyprland

Initial Linux stack:

```text
Blob Workspace/Surface Engine
        |
Blob UI components (Slint where appropriate)
        |
Hyprland integration
        |
Wayland
        |
Mesa / DRM / KMS / Linux drivers
```

Hyprland is a first-class **Experience Profile/integration target**, not a permanent architectural dependency.

Longer-term option:

```text
Blob Surface Engine
       |
Blob Shell
       |
Blob compositor (Rust + Smithay)
       |
Wayland / DRM / KMS
```

This becomes worthwhile only if semantic Surface management requires compositor-level control that Hyprland cannot provide cleanly.

### Android Native

A future renderer may use Android-native UI APIs for phone/tablet surfaces while consuming the same semantic Surface Model.

## Stable experience vs generative adaptation

The Blob does not regenerate the whole UI arbitrarily. Muscle memory and predictability are system requirements.

```text
stable Experience Grammar
          +
versioned Experience Profile
          +
contextual temporary Surfaces
```

AI may propose or instantiate contextual panels and layouts inside declared schema boundaries, but stable shortcuts/layout grammar change only through explicit/authorized Workspace evolution.

## Ready / AI Designed / Expert

Experience Profiles participate in all three Workspace construction modes.

### Ready

Choose a curated combination such as:

- Development / Blob Native Balanced
- Development / Hyprland Keyboard-first
- Development / macOS Native

### AI Designed

The Workspace Architect derives a profile from hardware, displays, input devices, habits and user priorities, then previews measurable trade-offs before committing it.

### Expert

Users may directly control compositor/window model, renderer, layout grammar, animation policy, keyboard/gesture conventions, density, rendering priorities, Surface placement and legacy-window behavior.

## Legacy applications

Existing applications remain supported.

On Linux, Wayland/XWayland clients can appear as `LegacySurface` elements in a Blob Workspace. On macOS, native applications such as Xcode or Safari can be associated with a Blob Workspace as native/legacy Surface providers.

The Blob must not require the software ecosystem to be rewritten before the new interaction model is useful.

## Core invariant

```text
same Workspace semantics
+ same Personal World state
+ same Tasks/Capabilities
!= same pixels
```

The Blob synchronizes meaning and state, not screenshots.
