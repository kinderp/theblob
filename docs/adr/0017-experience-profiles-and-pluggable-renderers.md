# ADR-0017 — Experience Profiles and pluggable native renderers

**Status:** Accepted

## Context

The Blob must preserve one logical Personal World and Workspace model across heterogeneous substrates while allowing each node to present an experience appropriate to that platform and to the user's preference.

A single mandatory cross-platform visual toolkit would make every platform look the same, but it would also prevent deep integration with native macOS, Android or Linux desktop conventions. Conversely, making Workspaces platform-specific would destroy continuity and portability.

## Decision

The Blob separates:

```text
Workspace / Task semantics
        |
Experience Grammar
        |
Surface Model
        |
Experience Profile
        |
Renderer
        |
Host graphics/windowing substrate
```

An **Experience Profile** is a versioned preference/policy object describing how one Workspace should be presented on a particular class of device or substrate.

Examples:

- `blob-native` — common Blob visual language, initially rendered with Slint;
- `macos-native` — native macOS integration, potentially SwiftUI/AppKit;
- `hyprland` — Linux keyboard/tiling-first integration using Hyprland/Wayland;
- future `gnome-native`, `kde-native`, `android-native`, accessibility-first or task-specific profiles.

Experience Profiles may be selected per device, per Workspace, or by explicit user policy. The system may suggest profiles, but it must not arbitrarily change a user's stable interaction model.

## Renderer policy

Slint is the first official **Blob-native renderer**, not the definition of the Surface model.

On Linux, Hyprland is an initial compositor/integration target and prototype dependency. Wayland remains the Linux display protocol substrate. A future Blob compositor may be implemented in Rust/Smithay if semantic Surface management requires compositor-level control.

On macOS, a native renderer may use macOS-native UI/windowing APIs rather than forcing Slint or Wayland. On Android, a native renderer may use Android-native UI APIs.

## Invariants

- `Workspace != Surface`.
- `Surface Model != renderer implementation`.
- `Experience Profile != Workspace Recipe`.
- A Workspace's persistent state, Tasks, Objects, Capabilities and causal history remain portable across renderers.
- Renderers receive typed semantic Surface descriptions; privileged AI must not gain authority by injecting arbitrary renderer code.
- Native host applications may appear as `LegacySurface` providers inside a Workspace.

## Consequences

The same Development Workspace may look and behave like macOS on a Mac and like a Hyprland environment on Linux while remaining the same logical Workspace.

This increases renderer implementation cost, but avoids coupling The Blob to one GUI toolkit or one compositor and makes platform-native experiences first-class rather than compatibility fallbacks.
