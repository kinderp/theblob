# Blob Characters v0.1

## Purpose

Blob characters are persistent inhabitants of the Shell, not application icons and not the authority layer of the system.

A character has one semantic identity and may be presented through multiple renderer profiles without changing Workspace, Surface, capability or execution semantics.

## Canonical character identities

| Character | Workspace role | Signature accessory | Character direction |
| --- | --- | --- | --- |
| Romeo | DEV | construction helmet / build tools | energetic, cocky, builder gremlin |
| Docs | DOCS | large glasses / reading object | clever, observant, slightly smug gremlin |
| System | SYSTEM | wrench / gear | stern, technical, pragmatic gremlin |
| Notes | NOTES | pencil / notebook | creative, mischievous, distracted gremlin |

Shared species direction: **tiny monsters / gremlins that are a little mean-looking but still small, cute and friendly to the user**.

## Renderer profiles

The same character contract currently supports three presentations:

- `soft3d` — soft volumetric / pre-render-friendly character presentation; P0 default.
- `pixel` — low-frame-rate arcade/pixel presentation.
- `ascii` — terminal-native ASCII character presentation.

Renderer choice is Experience state only. It must not mutate Workspace state or system authority.

## Shared presentation contract

Each renderer receives the same minimum inputs:

- character name;
- Workspace role;
- semantic status string;
- signature accessory;
- identity colour;
- focus state;
- activation callback.

The Shell may switch renderer live while preserving the same Workspace and Surface composition.

## Character state model

Future character behaviour should be expressed as renderer-neutral state rather than direct drawing commands.

Candidate state:

```text
CharacterState
  character_id
  mood
  activity
  energy
  position
  scale
  facing
  held_object
  target
  focus_mode
```

Candidate semantic actions:

```text
idle
blink
look
wave
work
warn
sleep
wake
walk_to
run_to
hop
hide
poke
hug
play_fight
chase
eat
drink
burp
yawn
grow
shrink
squash
stretch
```

P0 intentionally implements only a tiny visual subset such as idle, blink and gesture.

## AI and deterministic behaviour runtime

AI should not emit renderer coordinates, frame numbers or raw animation instructions.

AI may propose a semantic character intent such as:

```text
actor: romeo
action: poke
target: system
```

A deterministic Blob Behavior Runtime should then:

1. validate that the action exists;
2. verify current UX/focus policy;
3. choose a legal path / interaction zone;
4. schedule character state transitions;
5. ask the active renderer to materialize those transitions;
6. permit interruption or cancellation by the user or system.

This keeps autonomous character behaviour separate from execution authority.

## Experience policy

Characters may be lively, mischievous and occasionally chaotic, but must never make the information UI chaotic.

Future activity profiles may include:

- `calm`
- `normal`
- `lively`
- `chaos`

Focus mode should suppress nonessential autonomous behaviour.

Movement should occur in declared character habitats / free zones and should not obscure important code, documents, controls or warnings.

Reduced-motion mode must remain possible regardless of renderer.

## Resource policy

The renderer is free to choose an efficient materialization strategy:

- Soft-3D can use pre-rendered transparent assets / short frame sequences instead of real-time 3D.
- Pixel uses discrete low-frame-rate sprite-like state changes.
- ASCII uses text replacement and low-frequency state changes.

The semantic behaviour model must not require a fixed frame rate.

## P0 rule

The three renderer profiles are an Experience experiment. They must not delay or complicate the semantic Shell architecture. `soft3d` remains the default until visual and performance measurements justify another default.
