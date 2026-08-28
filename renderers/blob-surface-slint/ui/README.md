# Blob Native Design System

The Blob UI is not a dashboard for the architecture. It is a Surface over the user's Personal World.

## First principles

1. **Show the world, not the machinery.** Primary navigation is `Now`, `Workspaces`, `History`, `Fabric`; internal concepts such as RequirementGraph, BindingPlan, derivations and permits belong in Inspector.
2. **Now before inventory.** The first screen answers “what matters right now?” rather than listing everything the system knows.
3. **Progressive disclosure.** Normal mode shows intent, state, risk and outcome. `Why?` explains the decision. Inspector exposes exact technical evidence.
4. **Technician is a surface role, not an app.** It remains context-aware and can shrink, expand or become voice-first depending on the device.
5. **Semantic state only.** Screens render state produced by the core/application layer. Avoid fake UI-only task state that cannot be traced to system evidence.
6. **Calm by default.** Prefer whitespace, one dominant activity and a small number of meaningful actions over grids of status cards.
7. **Continuity is visible.** Activities may expose their current endpoint and available handoff without turning device topology into the primary interface.
8. **Safety should be legible.** Temporary/test/VM state must be understandable without forcing the user to read a security log.
9. **Expert depth is unlimited, normal density is not.** Complexity is not removed; it moves behind explicit inspection.
10. **Renderer is replaceable.** Slint implements Blob Native desktop today; the design grammar and typed Surface state must not depend on Slint-specific semantics.

## Initial tokens

`design/tokens.slint` is the single source for the first visual vocabulary:

- canvas/surface hierarchy;
- text hierarchy;
- accent and semantic state colors;
- spacing scale;
- radius scale;
- shell dimensions.

Do not introduce one-off colors or spacing in screens unless a missing semantic token has first been identified.

## Initial primitives

The first component set is intentionally small:

- `BlobCard` — one bounded semantic object;
- `BlobNavItem` — primary world navigation;
- `BlobStatus` — short low-noise state;
- `BlobAction` — explicit user action.

A new primitive should be added only when at least two screens need the same semantic interaction pattern.

## Demo v0.1 screen grammar

The first product demo should stabilize four surfaces before adding breadth:

1. **Now** — active Workspace/activity and relevant system context;
2. **Workspace** — the user's work, composed from capabilities rather than app chrome;
3. **Technician / System Change** — Explain → Teach → Prepare → Test, with visible safety state;
4. **Inspector** — Situation, Binding, Verifier, Nix/materialization evidence and causal trace.

The existing diagnostics-style MVP UI is not discarded; its concepts migrate into Inspector.
