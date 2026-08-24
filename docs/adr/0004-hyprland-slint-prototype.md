# ADR-0004: Slint UI with Hyprland as prototype compositor

**Status:** provisional

## Decision
Use Slint for early native Workspace/System surfaces. Use Hyprland/Wayland during prototyping to avoid spending early project effort on compositor engineering.

Design Workspace and Surface abstractions so they do not depend on Hyprland's window/workspace semantics. Evaluate a Rust/Smithay compositor later.
