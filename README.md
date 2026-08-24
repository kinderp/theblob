# The Blob

**One computer. Every device.**

The Blob is a research and engineering project exploring a personal, adaptive, AI-native computing environment where the user's world is continuous across devices, software is composed from capabilities instead of monolithic applications, work happens in persistent Workspaces, and system evolution is explainable, versioned, testable, and reversible.

The name is a cultural homage to *The Blob* (1958), in the same playful spirit with which Bell Labs named Plan 9 after *Plan 9 from Outer Space*. The metaphor also fits the architecture: one identity that changes shape, expands across devices, acquires capabilities when needed, and remains a single environment.

## Current status

**Phase 0 — Architecture v0.5, vocabulary, systems archaeology, deterministic resolver design and proactive System Technician.**

We are deliberately not implementing the full system yet. The first implementation will be a small vertical slice proving the separation between semantic intent, deterministic resolution, independent verification and ephemeral capability execution.

## Research lineage

The Blob explicitly studies UNIX, Plan 9, Inferno, Plan B, Omero/Octopus, semantic file systems, Exokernel, capability-oriented systems, modern dependency solvers and SMT/constraint solving.

```text
MULTICS
   |
UNIX
   |
PLAN 9
   |
INFERNO
   |
PLAN B / OMERO / OCTOPUS
   |
   v
THE BLOB
```

The goal is not to claim those earlier ideas as new. The research question is what that line of operating-systems work becomes when its primitives also include AI reasoning, agents, semantic memory, heterogeneous personal devices, cloud/GPU resources and safe software synthesis.

Start here:

- [`docs/VISION.md`](docs/VISION.md)
- [`docs/CONCEPTS.md`](docs/CONCEPTS.md)
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- [`docs/ARCHITECTURE-v0.5-DELTA.md`](docs/ARCHITECTURE-v0.5-DELTA.md) — accepted System Technician/AI Broker delta pending consolidated freeze
- [`docs/ARCHAEOLOGY.md`](docs/ARCHAEOLOGY.md)
- [`docs/NAME.md`](docs/NAME.md)

Deep dives:

- [`docs/research/PLAN-B-BOXES.md`](docs/research/PLAN-B-BOXES.md)
- [`docs/research/CONSTRAINT-SOLVER-STUDY.md`](docs/research/CONSTRAINT-SOLVER-STUDY.md)

## Core ideas

- **Personal World** — one persistent personal computing environment across PC, phone, watch, server, cloud and future devices.
- **Compute Fabric** — devices are resources/capability providers of one environment, not isolated computers.
- **Workspace** — persistent, user-recognizable interactive environment for a class of activity.
- **Workspace Recipe** — versioned blueprint with **Ready**, **AI Designed** and **Expert** creation modes.
- **Surface** — device/context-specific projection of a Workspace.
- **Capability** — abstract typed ability such as `document.render.pdf`, `web.render`, `git.diff` or `model.inference`.
- **Capability Capsule** — concrete, usually ephemeral implementation materialized through WASM/WASI, OCI, microVM, native or remote execution.
- **Requirement Graph** — typed roles and relations with policy, hard constraints, preferences, objectives, effects and authority requirements.
- **Constraint IR** — backend-neutral deterministic constraint language owned by The Blob; AI/plugins never emit raw SMT as authority.
- **Binding Plan / Lease** — selected implementations, nodes, adapters, data routes and scoped authority with safe rebinding boundaries.
- **Independent Binding Verifier** — re-checks every concrete selected plan; solver output never grants authority by itself.
- **ResolutionTrace** — explains derivation, rejections, solver evidence, objective ranking, verification and tie-breaks.
- **Task** — concrete activity performed inside or across Workspaces.
- **Intent / Goal** — desired outcome; Goals can persist over time.
- **Alfred / Situation Engine** — event-driven nervous system correlating events into semantic Situations.
- **System Technician** — always-available, local-first, model-agnostic AI-assisted system engineer that diagnoses, explains, proactively watches for relevant upstream improvements, prepares/test changes and never bypasses deterministic policy/verification.
- **AI Broker** — routes reasoning among resident, local, Fabric and optional cloud models according to privacy, quality, latency, cost, energy and hardware constraints.
- **Knowledge Object** — persistent user-owned object, independent from any application.
- **Projection** — least-privilege typed slice of a Knowledge Object.
- **Representation** — derived materialization such as PDF, DOCX, HTML, audio or thumbnail.
- **Temporal/Causal Graph** — Git-like history of state plus reasons, evidence and outcomes.
- **Adaptive System** — kernel, drivers, runtime and configuration may evolve through controlled experiments.
- **Constitutional Core** — trusted recovery, authorization, policy, verification and rollback substrate.

## Resolver architecture

```text
Intent / Situation
       |
       v
RequirementGraph
       |
       v
candidate derivation / recursive closure
       |
       v
CandidateGraph
       |
       v
SMT/MaxSMT backend (Z3 first)
       |
       v
SolverProposal
       |
       v
independent Rust BindingVerifier
       |
       v
BindingPlan + BindingLease
```

Specialized sub-problems remain separate:

```text
versions/dependencies -> PubGrub candidate
future scheduling     -> CP-SAT candidate
recursive graph facts -> Datalog-style engine when justified
```

## Initial technical direction

- Linux kernel and existing driver ecosystem.
- NixOS as first declarative PC/server substrate, not a permanent product constraint.
- Android/GKI-compatible substrate for early phone/watch nodes.
- Rust as dominant implementation language for trusted/core components.
- Z3 as first SMT/MaxSMT resolution backend behind our own Constraint IR.
- Independent Rust verifier outside the solver.
- Language-neutral Capability contracts; WASM/WASI where appropriate plus OCI, microVM, native and remote backends.
- Slint for early native UI surfaces.
- Hyprland as prototype Wayland compositor only; Rust/Smithay later if the Workspace/Surface model requires it.

## First vertical slice

See [`docs/MVP-0.md`](docs/MVP-0.md).

Contracts and roadmap:

- [`docs/CAPABILITY-CONTRACT-v0.1.md`](docs/CAPABILITY-CONTRACT-v0.1.md)
- [`docs/RESOLUTION-CONTRACT-v0.1.md`](docs/RESOLUTION-CONTRACT-v0.1.md)
- [`docs/ROADMAP.md`](docs/ROADMAP.md)
- [`docs/OPEN-QUESTIONS.md`](docs/OPEN-QUESTIONS.md)
- [`docs/SYSTEM-TECHNICIAN.md`](docs/SYSTEM-TECHNICIAN.md)

## Design rule

> AI interprets, reasons and proposes. Deterministic systems verify, authorize and materialize.

The Blob is currently a research architecture. Implementation choices remain deliberately replaceable until the first vertical slice proves which abstractions deserve to harden.
