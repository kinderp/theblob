# `blob-surface-app`

Renderer-neutral application contract between user-facing Surfaces and The Blob's semantic/authority layers.

## Purpose

A renderer must not translate a click directly into a Nix command, privileged helper invocation or model-selected shell action. It emits a typed intent. This crate owns the first small reducer for those intents.

Current intents:

- navigate to `Now`, current `Workspace`, `History` or `Fabric`;
- open/close `Inspector` with deterministic return navigation;
- ask the Technician to `ExplainCurrent`, `TeachCurrent` or `PrepareNextStep`.

## Authority invariant

`TechnicianIntent::PrepareNextStep` means only that the user requested preparation. Recording that intent is **not** authorization to mutate the system and this crate has no executor, D-Bus, package-manager or NixOS dependency.

A later application adapter may translate a preparation intent into a semantic proposal/request, but it must still cross the existing deterministic policy, verification and authorization boundaries before any privileged execution occurs.

## Renderer relationship

```text
Slint / future renderer
        |
        v
   SurfaceIntent
        |
        v
SurfaceApplication
        |
        v
   SurfaceEffect
```

The same intent contract can later be reused by desktop, mobile, voice and car Surfaces without reusing their visual toolkit.
