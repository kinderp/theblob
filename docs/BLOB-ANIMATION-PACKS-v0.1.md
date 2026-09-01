# Blob Animation Packs v0.1

Status: P0 contract / future-store foundation

## Goal

A Blob character has one semantic personality and many possible renderers and animation packs.

The system decides **what the Blob is doing**. An animation pack decides **how that already-approved action looks**.

```text
Workspace / system evidence
        ↓
BlobBehaviorContext
        ↓
BlobAction
        ↓
validated Animation Pack
        ↓
3D / Pixel / ASCII renderer
```

This separation is required so AI-generated or community-created content can be installed without becoming executable system authority.

## P0 semantic actions

The first stable action vocabulary is:

- `idle`
- `blink`
- `look`
- `grin`
- `wave`
- `busy`
- `warning`
- `sleep`

Later versions may add actions such as `walk`, `run`, `eat`, `burp`, `grow`, `shrink`, `poke`, `chase`, `hug`, `play_fight`, `read`, `write`, and `repair`.

Adding an action to the vocabulary is a runtime/API change. Publishing new media for an existing action is only a content-pack change.

## Behavior resolution

P0 uses deterministic priority for real state over decorative state:

```text
problem      → warning
active task  → busy
selected     → wave
hovered      → look
long idle    → sleep
otherwise    → idle
```

Renderers may add harmless micro-motion such as occasional blinking while idle.

The AI may later *propose* higher-level actions, but it does not write frames, coordinates, or renderer state directly into the Shell runtime.

## Pack properties

A pack is data-only. The current Rust contract models:

- schema version
- pack id and semantic version
- target renderer: `Soft3d`, `Pixel`, or `Ascii`
- creator
- license
- compatible character family
- available clips
- content digest
- whether AI was used
- optional generation provenance

Each clip declares:

- semantic `BlobAction`
- frame count
- frames per second
- loop policy
- whether it may be interrupted

P0 caps declared animation rate at 30 fps. The default design target is much lower: short clips around 4–8 fps and long idle periods with no continuous redraw.

## Safe fallback

Every valid pack must contain `idle`.

If the runtime requests an optional action that the installed pack does not provide, the renderer falls back to `idle` rather than executing unknown content or failing the Shell.

```text
requested: sleep
pack has sleep? yes → sleep
                no  → idle
```

This lets old packs remain usable when the semantic action vocabulary grows.

## Store model

A future Blob Store can distribute animation packs independently from applications, Workspaces, and system capabilities.

Suggested trust channels:

1. **Built-in / verified** — shipped or reviewed with The Blob.
2. **Signed community** — community creator identity plus immutable content digest/signature.
3. **Local / private** — user-generated pack that never needs publication.
4. **Experimental** — explicitly opted-in packs that may target newer action APIs.

The store should display at least:

- preview for every supplied action
- creator and license
- AI-generated / AI-assisted disclosure
- compatible Blob character family and renderer
- size and expected animation cost
- content digest/signature state
- reports/moderation status
- supported action list

## AI creation workflow

The intended user flow is:

```text
"Make Romeo celebrate a successful build by
 jumping twice and falling over laughing"
        ↓
AI proposes storyboard / frames
        ↓
user previews and edits
        ↓
pack compiler maps the result to BlobAction or
an approved extension action
        ↓
validator checks dimensions, timing, manifest,
resource limits and content digest
        ↓
local install
        ↓
optional sign + publish to Blob Store
```

The generated media may come from an image/video/3D model pipeline, but the installed result remains a bounded content package.

## Security boundary

Animation packs must not contain arbitrary executable hooks.

A pack cannot:

- run shell commands
- access the network
- read Workspace content
- request credentials
- mutate SystemSpec
- grant itself capabilities
- change placement or Fabric policy
- synthesize privileged intents

A pack receives a bounded semantic action and rendering context. It returns visual/audio presentation only.

Future sound effects, particles, or movement beyond the Blob habitat should have explicit declarative resource and behavior limits.

## User sovereignty

The user controls animation intensity globally and per Workspace.

Expected modes:

```text
calm    — almost static; state only
normal  — idle personality and contextual reactions
lively  — more spontaneous social actions
chaos   — playful autonomous interactions when allowed
focus   — suppress non-essential behavior
```

AI-generated behavior always remains subordinate to focus mode, accessibility preferences, power policy, and user overrides.

## Renderer independence

The same semantic action can be rendered by unrelated visual technologies:

```text
BlobAction::Sleep
  ├─ Soft3d pack → pre-rendered sleeping monster frames
  ├─ Pixel pack  → arcade sprite animation
  └─ ASCII pack  → character-art sleep frames
```

This is why actions belong above the renderer and packs belong below the semantic behavior layer.

## P0 implementation boundary

Implemented now:

- semantic action enum
- deterministic behavior resolver
- animation-pack manifest contract
- manifest validation
- required idle fallback
- renderer-kind identity
- AI provenance field

Next implementation slice:

1. feed resolved action into the Slint Shell;
2. add 3D state assets for `blink`, `look`, `grin`, `wave`, `busy`, `warning`, `sleep`;
3. add a tiny clip scheduler with event-driven timers;
4. expose animation intensity (`calm/normal`) in demo settings;
5. keep Pixel and ASCII on `idle` fallback until their packs are designed.
