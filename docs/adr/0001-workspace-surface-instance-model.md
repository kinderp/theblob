# ADR 0001 — Workspace, Surface and SurfaceInstance

**Status:** Accepted  
**Date:** 2026-08-29

## Context

The existing Blob documents correctly separate Workspace semantics from pixels, but earlier wording used `Surface` both for the semantic presentation role and for a device-specific rendering. The GUI design work also temporarily reused the existing term `Projection` for a concrete Surface manifestation, conflicting with `Projection`'s established meaning as a typed semantic selection over an object/resource.

The shell additionally needs to support one semantic interactive role being present on multiple devices at the same time, potentially with different Experience Profiles.

## Decision

Use the following hierarchy:

```text
Workspace
   |
   +-- Surface
          |
          +-- SurfaceInstance
                  |
                  +-- SurfaceHost / renderer / platform
```

### Workspace

Persistent semantic human context. It owns context, tasks, object references, experience/layout preferences and Surface membership. It is independent from processes, applications, windows and physical devices.

### Surface

Persistent typed interactive role inside a Workspace. Examples: `Editor`, `Docs`, `Terminal`, `Tests`, `SystemHealth`, `GitStatus`.

A Surface may exist without currently being visible and may be backed by a Blob-native implementation, hosted legacy application, semantic application adapter or remote capability.

### SurfaceInstance

Concrete manifestation of one Surface on one SurfaceHost/device/context. A Surface may have zero, one or many simultaneous instances.

A SurfaceInstance owns presentation/interaction state that is specific to that manifestation (for example dimensions, density/profile and local panel state) while the parent Surface may own shared semantic state.

### Projection

`Projection` retains its pre-existing Blob meaning: a typed semantic selection over a Knowledge Object/resource. It is not renamed and is not used for Surface placement.

## Consequences

- Closing a SurfaceInstance does not necessarily destroy the Surface or Workspace state.
- The same Surface can appear on MacBook and tablet using different profiles.
- `Mirror`, `adaptive instance` and `companion` behaviors can be expressed without duplicating the Workspace.
- Layout acts on SurfaceInstances; semantic state belongs to the Surface/Workspace as appropriate.
- Rendering/platform integration remains replaceable.
- Existing code may continue using simpler Surface structures during the demo, but new APIs must not deepen the old terminology ambiguity.

## Rejected alternatives

### Surface = device-specific view only

Rejected because it makes persistence and multiple simultaneous device manifestations awkward and pushes device concerns too high in the semantic model.

### Reuse Projection for concrete UI manifestations

Rejected because `Projection` already has a distinct semantic/security meaning in the Blob concept model.

### Workspace = collection of windows/apps

Rejected because applications and windows are implementation/presentation details rather than the durable human context.
