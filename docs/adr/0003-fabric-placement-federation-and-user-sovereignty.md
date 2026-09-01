# ADR 0003 — Fabric Placement, Federation and User Sovereignty

**Status:** Accepted  
**Date:** 2026-08-29

## Context

The Blob Fabric must support two different user needs:

1. place Workspace Surfaces/tasks/capabilities on appropriate devices; and
2. let several personal devices/resources behave like one computer for operations such as clipboard, object/file access, compute and device use.

Treating every Workspace as permanently distributed would add complexity without value. Treating every physical device as an isolated computer would defeat the Personal Fabric model.

The user must retain final control over placement and automation.

## Decision

### Fabric has two jobs

**Placement** decides where interaction, SurfaceInstances, execution and data live.

**Federation** makes authorized resources from multiple nodes available through one Personal/Workspace Fabric view.

### Workspace independence from device

A Workspace belongs logically to the Personal World/Fabric, not to one physical device. This does not imply mandatory distribution.

Natural operating modes:

- `Local` — remain on one node;
- `Handoff` — change the user's interaction point and appropriate SurfaceInstances while leaving useful execution/data in place;
- `Distributed` — use multiple nodes only when explicitly requested or concretely beneficial.

### Placement dimensions are distinct

```text
Interaction presence
SurfaceInstance placement
Execution placement
Data placement
```

These dimensions must be independently observable and overridable.

### Workspace FabricView

Each Workspace may have a different logical view of authorized Fabric resources. A Workspace FabricView is conceptually the intersection of:

```text
available
∩ compatible
∩ authorized
∩ policy-visible
```

### Drag/drop is semantic

Dragging a Workspace, Surface, Task/capability execution or object onto a device may produce different Intents. The shell must preview the operation and avoid ambiguous silent behavior.

Examples:

- Workspace -> notebook: handoff/open suitable SurfaceInstances;
- Docs Surface -> tablet: create/move a SurfaceInstance;
- Tests -> desktop: place `test.run` there;
- Document -> desktop: make the object available there using an appropriate data-placement strategy.

### User sovereignty

Recommendations do not become prohibitions.

The Blob may recommend a placement based on performance, power, availability, privacy, trust or latency. The user can override it when technically and policy-wise possible.

Autonomy is explicitly delegated, scoped and revocable.

## Consequences

- Fabric UX can expose `Unified`, `Devices` and `Placement` views without changing the underlying model.
- A Workspace may be simple/local by default and become distributed only when useful.
- `Continue here` can move interaction without needlessly migrating build jobs/containers.
- Clipboard/file/object federation becomes a normal Fabric resource problem rather than a collection of unrelated special features.
- Placement history can be explained by the Technician.

## Rejected alternatives

### Always distribute Workspaces

Rejected because distribution is a cost and failure surface, not a goal.

### Workspace belongs to one machine and can only be moved wholesale

Rejected because interaction, display, compute and data frequently benefit from independent placement.

### Automatic optimizer has final authority

Rejected because The Blob is user-sovereign; optimization is advisory or operates only within explicitly delegated bounds.
