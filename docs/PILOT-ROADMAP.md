# Product Pilot Roadmap

**Status:** accepted product strategy overlay.

This roadmap describes what a user should be able to *experience*. It complements the engineering roadmap in `ROADMAP.md`, which tracks internal implementation phases.

## Pilot A — Linux: Arch/Gentoo power, Windows/macOS simplicity

The first product goal is not multi-device distribution. It is to prove that The Blob can make a Linux machine deeply configurable without requiring the user to be a Linux expert.

### Target experience

A user should be able to say:

> Make this machine fast, quiet and optimized for development, but explain what you change and let me open the hood whenever I want.

The Blob should provide:

- simple onboarding comparable to mainstream desktop systems;
- Ready / AI Designed / Expert paths;
- a persistent System Technician that diagnoses and explains;
- declarative, versioned system configuration;
- reproducible build profiles inspired by Gentoo-style specialization;
- ability to choose/remove compile-time and runtime features when useful;
- hardware-aware optimization;
- curated/default choices for non-experts;
- full inspectability for experts;
- experiment branches, benchmark before/after, causal history and rollback;
- Improvement Watch for relevant kernel, driver, compiler and package improvements;
- official provenance/documentation attached to privileged proposals;
- legacy Linux applications available throughout the transition.

### Principle

```text
Freedom of Arch
+ specialization of Gentoo
+ usability of mainstream desktops
+ an always-available System Technician
```

The user should never be forced into Expert mode to obtain a reliable system, but Expert mode must never be removed or artificially restricted.

### Pilot A exit criteria

The Linux pilot is successful when a real user can:

1. bootstrap a supported Linux node;
2. choose Ready, AI Designed or Expert configuration;
3. create/use at least one real Workspace;
4. install/materialize capabilities without traditional application-management complexity for the demonstrated path;
5. ask the Technician to explain current system configuration;
6. diagnose one real regression/problem from events and causal history;
7. propose and test one system-level improvement;
8. compare before/after measurements;
9. activate or reject the candidate;
10. roll back confidently;
11. continue using normal Linux applications.

## Pilot B — Personal World across real heterogeneous devices

Once the Linux experience is useful by itself, expand the same Personal World gradually. Do **not** attempt all devices simultaneously.

### Node 1 — Linux reference node

Use the primary Linux pilot as the first authoritative/deep-control node.

It provides:

- Personal World state;
- Workspace/Task state;
- Alfred events;
- Capability runtime;
- System Technician;
- causal history;
- first Surface.

### Node 2 — second Linux/Ubuntu node

Add the Dell XPS Ubuntu machine as the first heterogeneous-but-Linux hosted node before crossing OS boundaries.

Prove:

- identity/trust enrollment;
- node capability advertisement;
- shared Task state;
- placement decisions;
- cross-node execution where useful;
- no requirement to reinstall the host OS immediately.

### Node 3 — macOS hosted node

Add the Mac while retaining macOS.

Initial goals:

- join Personal World;
- native macOS Experience Profile;
- Workspace continuity;
- local Blob runtime/services where permitted;
- native/legacy macOS applications as Legacy Surfaces/Capability Providers;
- optional Linux Capsule execution through an isolated Linux VM/runtime when required.

Deep kernel/driver mutation is not required on macOS hosted mode.

### Node 4 — Windows hosted node

Add the ThinkPad X390 while retaining Windows.

Initial goals:

- Rust-based Blob node agent/service;
- identity/trust enrollment;
- Workspace/Task continuity;
- native Windows Surface/notifications where useful;
- legacy Windows applications as surfaces/providers;
- optional WSL2/VM/container execution for Linux-oriented Capsules rather than requiring a Windows-native implementation for everything.

Windows support should prove that The Blob is not simply a Linux distribution with remote clients.

### Node 5 — Android phone

Add the Android smartphone as the first always-carried device.

Initial goals:

- Personal World identity and encrypted sync;
- mobile Surface for an existing Workspace/Task;
- Alfred events appropriate to explicit policy;
- notification/action surface;
- selected phone hardware capabilities such as camera, network, location or sensors only through explicit permissions;
- Resident AI if hardware permits, otherwise Fabric/cloud routing according to policy.

### Node 6 — Garmin wearable companion

Treat the Garmin watch **first as a constrained companion Surface/sensor provider**, not as a full general-purpose Blob node.

Initial path:

```text
Garmin watch
   -> Connect IQ companion experience / supported device APIs
   -> Android phone bridge
   -> Personal World
```

First goals:

- Task status;
- notifications;
- approve/deny a narrow safe action;
- expose selected sensor/status data when the platform and user permissions allow;
- preserve the same Task/Workspace identity as desktop/mobile Surfaces.

A deeper Garmin integration can be evaluated later. The architecture must not assume arbitrary daemon/container execution on the watch.

## Pilot B target experience

A single Task should be able to move through the user's real environment:

```text
Linux desktop
   -> continue/review on Mac
   -> status/action on Android
   -> tiny notification/action on Garmin
   -> execute suitable work on another Linux/Windows-hosted Fabric node
```

The user should perceive one Personal World rather than a collection of synchronized apps.

## Sequencing rule

Every new node type must add one measurable user capability and preserve previous behavior before the next platform is started.

Do not build five incomplete clients at once.

## Product success criterion

The Blob succeeds when adding a new device feels less like installing and configuring another computer and more like attaching a new organ to an existing personal computing environment.
