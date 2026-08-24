# Concept Model

## Personal World
Persistent environment owned by the user. Contains identity, preferences, Knowledge Objects, Workspaces, task state, goals, semantic memory, policy and causal history.

## Compute Fabric
Set of nodes available to the Personal World. A node advertises hardware, software and sensor/actuator capabilities, trust level, location constraints, cost, power state and availability.

## Surface
A device-specific projection of a Workspace or Task. The same Workspace can render as a full desktop environment, a mobile review surface or a watch notification/action surface. Surface state should be typed and inspectable so that agents operate on semantic UI elements rather than pixels whenever possible.

## Workspace
A relatively persistent interactive environment and **semantic namespace** for a domain of work: Web, Development, Writing, Research, Media, Communication, etc. It composes relevant Knowledge Objects/Views, persistent context, experience grammar, baseline capabilities, dynamic capability requirements, policies, Tasks and Surface state. It owns experience-level layout and preferences, not user data or executable implementations.

## Workspace Recipe
Versioned blueprint for building a Workspace.

### Onboarding modes
- **Ready** — curated, tested Workspace with good defaults.
- **AI Designed** — AI Workspace Architect composes a Workspace from user goals, hardware and preferences, with measurable trade-offs.
- **Expert** — user selects components, policies, layouts, implementations and optimization priorities directly.

Recipes are forkable, mergeable, benchmarkable and publishable to a future Workspace Registry.

## Capability
Abstract typed ability, independent of implementation and location. Examples: `web.render`, `document.translate`, `image.enhance`, `git.diff`, `model.inference`.

## Capability Capsule
Concrete implementation of a Capability. May be WASM/WASI, OCI container, microVM, native component, remote service or hardware capability. Usually acquired/cached/materialized on demand and released afterwards.

## Capability Graph
Graph of available capabilities, implementations, type compatibility, dependencies, trust constraints, cost, privacy, latency and placement options.

## Capability Resolver
Chooses an implementation and execution node based on policy and task requirements.

## Capability Requirement
Structured, typed request produced by a Task/Planner for an abstract Capability. It expresses required input/output types plus constraints such as privacy, trust, latency, quality, cost, energy, offline behavior and side-effect limits. It does not name one implementation.

## Constraint Solver
Deterministic resolver that checks type compatibility, converter paths, policy, authority, trust, effects, resource availability and placement constraints. AI may help construct or rank requirements/valid alternatives; it does not bypass solver validity.

## Adapter / Converter Capability
First-class graph edge transforming one typed representation/object into another. Adapters have the same metadata as other capabilities: trust, quality, cost, effects, permissions and placement. A future AI may synthesize candidate adapters, but only behind sandboxing and deterministic validation.

## Capability Binding
A concrete, time-bounded association between an abstract Capability Requirement, one Capsule/implementation and one or more Fabric nodes. Bindings should be created late and may be safely re-resolved when context or resources change.

## Task
Concrete activity with state, inputs, desired result, working set, agents, temporary capabilities and outputs.

## Intent
Desired outcome expressed by a person or authorized agent.

## Goal
Persistent higher-level objective that can span time, Tasks and multiple Workspaces.

## Context
Current cognitive and operational state relevant to a Task/Goal/Workspace.

## Event
Raw or normalized signal from kernel, filesystem, device, service, sensor, user or capability.

## Situation
Semantic interpretation of one or more events over time, e.g. “the user is leaving while a long-running task is unfinished”.

## Alfred / Situation Engine
Event-driven nervous system. Its v0.2 pipeline intentionally separates deterministic event normalization/correlation, AI-assisted semantic Situation interpretation, deterministic policy/authorization, and subsequent plan/capability execution. AI inference does not itself grant authority.

## System Technician
Persistent AI-assisted system-engineering role that helps the user understand, diagnose, maintain and reshape The Blob. It is not a privileged authority by itself. It observes structured system state and Situations, explains problems, proposes improvements, produces candidate Workspace/SystemSpec changes, and delegates all privileged actions through deterministic policy, verification, simulation and causal history.

The System Technician should behave like an always-available expert administrator while remaining understandable to non-experts. It may be reactive (answer a user request) or proactive (respond to Situations, degradation, regressions, security advisories or relevant upstream improvements).

## Improvement Watch
Proactive maintenance function associated with the System Technician. It correlates the user's actual hardware, workloads, known problems and causal history with trusted external technical information such as official kernel/driver/project documentation, release notes, security advisories and package metadata. It should suppress irrelevant update noise and surface an improvement when it can explain why it matters to this Personal World.

A proposal should include provenance/direct official references where available, applicability evidence, expected benefit, risks, compatibility notes, proposed test/rollback path and required user authorization. Discovery does not itself grant permission to mutate the system.

## AI Broker
Model-agnostic routing layer used by the System Technician and other cognitive services. It selects among a small resident model, a stronger local model, a model available elsewhere in the Personal Compute Fabric, or an explicitly permitted cloud provider according to privacy policy, task quality requirements, latency, monetary cost, energy and hardware availability.

The user experiences one Technician even if the underlying model changes. Model selection is an implementation binding, not an identity boundary. Cloud reasoning is never an authority boundary and receives only policy-approved Projections/minimal context.

## Knowledge Object
Persistent user-owned object with stable identity, content/data, metadata, semantics, provenance, relations, transformations and history.

## Representation
Materialized view/artifact of an object: PDF, DOCX, HTML, audio, thumbnail, mobile surface, print representation, etc.

## View
Saved query/projection over Knowledge Objects. Replaces many hierarchical folder use cases without removing exportable directories/files.

## Temporal/Causal Graph
Version DAG spanning relevant system state, Workspace Recipes, policy, Knowledge Objects and autonomous actions, including causal metadata.

## Adaptive System
Mutable layer: system configuration, kernel parameters, kernel builds/modules, drivers, runtime, services and performance profiles.

## Constitutional Core
Minimal trusted base guaranteeing identity, authorization, trusted boot/recovery, policy enforcement, verification and rollback. Adaptive components must not be able to disable it through normal operation.

## Requirement Graph
General deterministic resolution problem for a Task. It contains typed roles, relations, hard constraints, soft preferences, optimization objectives, required effects and authority requirements. A scalar Capability Requirement is a one-role specialization. Roles are resolved jointly so individually valid selections cannot form an invalid overall plan.

## Binding Plan
Complete valid solution to a Requirement Graph: concrete Capsule implementations, Fabric placements, adapter paths, data routes, delegated grants, expected effects, ranking/score and a structured Resolution Trace explaining why candidates were selected or rejected.

## Binding Lease
Scoped commitment to a Binding Plan or subset of its bindings. Records validity conditions, delegated authority and safe re-resolution boundaries. Prevents “late binding” from meaning arbitrary mid-effect migration.

## Projection
Typed semantic selection over a Knowledge Object or resource. Examples include a document section, code symbol, image region or privacy-reduced text view. Capabilities should prefer narrow authorized Projections to whole-object access.

## Compatibility / Introspection Projection
Simple generic view of a richer native system entity for legacy/general-purpose tooling. Candidate mechanisms include virtual file trees, structured CLI/text, JSON/CBOR, 9P-like bridges and POSIX exports. It is an adapter, not the native semantic model.
