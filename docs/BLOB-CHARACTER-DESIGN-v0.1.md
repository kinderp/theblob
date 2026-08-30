# Blob Character Design v0.1

Status: canonical visual direction for the 3D Blob renderer.

## Core identity

The four Workspace characters are small living blobs: cute enough to feel familiar, mischievous enough to feel like little monsters, and visibly made of unstable organic goo.

The reference is **Blob biology + gremlin attitude**, not fantasy-gremlin anatomy.

## Non-negotiable rules

- No horns.
- No devil tails, demon wings, rigid fantasy spikes, or other demonic anatomy.
- Bodies are amorphous, gelatinous, viscous and deformable.
- Silhouettes should feel slightly irregular and capable of flowing, stretching or collapsing.
- Expressions may be sly, annoyed, smug, curious or mischievous; avoid permanently innocent/kawaii faces.
- Small teeth are allowed when they read as playful monster traits, not horror gore.
- Role accessories remain instantly readable and are not part of the creature anatomy.

## Canonical roles

- Romeo / DEV: yellow-orange goo, construction helmet with `</>`, energetic, cocky, impatient.
- Docs / DOCS: blue goo, large glasses and reading material, clever, observant, slightly smug.
- System / SYSTEM: green goo, wrench/tool, stern, practical, easily irritated by faults.
- Notes / NOTES: purple goo, pencil/notebook, creative, distractible, mischievous.

## Motion language

Movement should prefer Blob-like deformation over ordinary humanoid locomotion.

Primary vocabulary:

- squash
- stretch
- ooze
- melt
- inflate / deflate
- stick / peel
- absorb / release
- puddle / reform
- wobble
- bounce

A future `walk_to` action may therefore render as a short ooze, hop, stretch or slide depending on the active animation pack.

## Expression language

"Cattivello" comes from expression and behavior, not horns.

Good cues:

- asymmetric grin
- narrowed or side-looking eyes
- raised soft brow/gel ridge
- tiny tooth
- tongue-out teasing
- annoyed compression
- smug lean

Avoid:

- generic happy mascot smile in every state
- permanent wide-eyed innocence
- demonic silhouettes
- realistic horror anatomy

## Renderer contract

`BlobAction` describes intent/state. The active animation pack decides the visual realization.

The character identity must survive renderer changes:

- Soft3D: viscous pre-rendered or renderer-native goo motion.
- Pixel: arcade interpretation of the same personality.
- ASCII: terminal interpretation of the same personality.

## P0 3D state coverage

Currently registered:

- idle
- look
- wave
- busy
- warning
- sleep

`blink` and `grin` remain intentionally unregistered until dedicated facial assets exist; the pack contract falls back to `idle` rather than pretending a clip exists.

## Future AI-generated assets

AI generation should preserve the canonical character identity and this document's anatomy rules. Generated states must be previewed and approved before inclusion in the built-in pack or publication to a community store.
