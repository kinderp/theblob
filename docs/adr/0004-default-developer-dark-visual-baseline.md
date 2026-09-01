# ADR 0004 — Default Developer-Dark Visual Baseline

**Status:** Accepted  
**Date:** 2026-08-29

## Context

The Blob Shell is intended to become highly personalizable, including themes, typography and Workspace-specific Experience Profiles. Building that customization machinery before the P0 experience is validated would, however, turn theme infrastructure into part of the demo critical path.

The first dark prototype also used highly saturated neon accents and static Workspace avatars. The result communicated novelty but was visually closer to a product dashboard than to a calm environment suitable for long programming and system-administration sessions.

The product therefore needs a deliberate **default appearance** even though that appearance will later be replaceable.

## Decision

The P0 default visual profile is **Developer Dark**.

It is a baseline, not a theme engine and not a permanent restriction.

### Typography

- use the platform's generic `monospace` family as the Shell default;
- prefer normal/medium weights for information and reserve heavier weight for hierarchy;
- avoid shipping a decorative or branded font solely for Shell identity;
- keep information density similar to a familiar editor/terminal rather than a consumer dashboard.

This intentionally allows the host to resolve `monospace` to a familiar local coding face while keeping the Shell portable.

### Colour and contrast

- very dark grey/blue canvas rather than pure black;
- off-white primary text rather than pure white;
- medium-contrast muted secondary text;
- restrained blue/purple/green/amber accents following familiar developer-tool conventions;
- Workspace colours distinguish identity but should not dominate large portions of the screen;
- no persistent neon glow as the primary visual language.

### Shape language

The information-bearing Shell remains simple, compact and rectilinear:

- small corner radii;
- light one-pixel borders;
- thin semantic accent markers;
- little ornamental chrome.

Workspace **Blob avatars are the deliberate exception**. They may be soft, organic and characterful so the system has an identity without making every panel playful.

### Motion

Blob avatars should feel alive, not decorative-animation-heavy:

- slow idle movement;
- subtle acknowledgement on hover/focus;
- eventual state/emotion changes tied to real Workspace activity;
- no constant high-frequency motion in information surfaces.

Motion must never be required to understand system state.

## Personalization later

A future theme/Experience system may replace colours, typography, density, Blob appearance and motion preferences. The default profile remains just one supported recipe.

P0 explicitly does **not** implement:

- a theme chooser;
- downloadable themes;
- per-Workspace theme editing;
- custom font packaging;
- an appearance DSL.

Those features must not delay validation of the core Shell interaction model.

## Consequences

- The demo has one coherent appearance that can be judged as a product.
- Long-session readability is prioritized over visual novelty.
- The distinctive character of The Blob is concentrated in Workspace avatars and interaction grammar instead of neon decoration.
- Later personalization can be introduced without changing Workspace/Surface/SurfaceInstance semantics.
