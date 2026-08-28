# `blob-surface-app`

Renderer-neutral application contract between user-facing Surfaces and The Blob's semantic/authority layers.

## Purpose

A renderer must not translate a click directly into a Nix command, privileged helper invocation or model-selected shell action. It emits a typed intent. This crate owns the first small reducer for those intents and the first deterministic projection of already-structured evidence into the Technician Surface.

Current intents:

- navigate to `Now`, current `Workspace`, `History` or `Fabric`;
- open/close `Inspector` with deterministic return navigation;
- ask the Technician to `ExplainCurrent`, `TeachCurrent` or `PrepareNextStep`.

## Evidence-backed Technician projection

`TechnicianEvidenceContext` consumes only existing backend-neutral semantic evidence:

- `Situation`;
- `Task`;
- `BindingPlan` + independent verifier notes;
- causal records.

It can render three deterministic views without any model call:

- **Explain** — what happened, which capability implementation/node was selected, verifier evidence and causal steps;
- **Teach** — the conceptual chain `Situation -> Task -> Capability -> Binding -> Verifier -> Result` using the exact current evidence;
- **Prepare preview** — what would need to happen next, while refusing to infer a system change from a successful task without a user-stated desired outcome.

The projection rejects a BindingPlan that does not belong to the Task's RequirementGraph.

## Authority invariant

`TechnicianIntent::PrepareNextStep` means only that the user requested preparation. Recording that intent is **not** authorization to mutate the system and this crate has no executor, D-Bus, package-manager or NixOS dependency.

The deterministic Technician projection is also read-only. `Prepare` carries `TechnicianAutonomy::Prepare` as the autonomy level that would be needed by a later proposal, not as granted execution authority.

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
        +----> TechnicianEvidenceContext
        |           |
        |           v
        |     TechnicianProjection
        v
   SurfaceEffect
```

The same intent/projection contract can later be reused by desktop, mobile, voice and car Surfaces without reusing their visual toolkit.
