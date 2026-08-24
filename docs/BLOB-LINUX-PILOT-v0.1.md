# The Blob Linux Pilot v0.1

**Status:** accepted product/engineering specification draft derived from the distribution audit.

## 1. Product thesis

The first practical objective of The Blob is narrower than the full Personal World vision:

> Deliver one Linux system with the freedom and inspectability of Arch, the specialization potential of Gentoo, and the onboarding/recovery expectations of mainstream Windows/macOS users.

The Linux pilot must remain useful even if multi-device Fabric, mobile Surfaces and advanced AI routing do not yet exist.

## 2. User promise

A non-expert should be able to obtain a reliable, hardware-aware Linux machine without learning package management, kernel configuration, bootloader recovery or build flags.

An expert must be able to inspect, override and replace the generated choices without leaving The Blob's versioned/causal model.

```text
Ready       -> safe curated defaults
AI Designed -> Technician builds a profile from goals/hardware/preferences
Expert      -> explicit components, channels, build features and system overrides
```

All three modes produce the same underlying semantic `SystemSpec`/profile model.

## 3. Reference substrate

### Pilot full-control node

- Linux kernel;
- NixOS as the first declarative system-materialization backend;
- systemd as the first service substrate;
- Wayland desktop environment;
- Hyprland initially supported as one Experience Profile/integration target, not as a Blob semantic dependency;
- Slint used for Blob Native Surfaces where appropriate;
- filesystem checkpoint adapter chosen to support safe rollback experiments, initially favoring Btrfs on the reference installation.

### Architectural rule

`NixOS != The Blob`.

The user interacts with Blob concepts; Nix is a backend.

## 4. Software layers for the pilot

The pilot intentionally avoids one universal package-manager abstraction.

### 4.1 Base/System layer

Managed through `SystemSpec -> Nix/NixOS`.

Contains:

- kernel/kernel parameters;
- hardware/driver selection;
- boot/recovery configuration;
- system services;
- core runtimes;
- Blob node services;
- trusted base dependencies.

### 4.2 Legacy desktop applications

Normal Linux applications remain first-class compatibility citizens.

Initial preference:

- Flatpak for isolated desktop software where it provides a good fit;
- native Nix packages when host integration or packaging makes that preferable;
- no requirement that an application be rewritten as a Blob Capability.

The user should not need to understand which backend was used unless they open Expert/inspection views.

### 4.3 Development environments

Prefer project/Workspace-local environments rather than polluting the base system. Candidate mechanisms include Nix development environments and containers, chosen behind Workspace semantics.

### 4.4 Blob Capabilities

Executed through Capsule runtimes:

- WebAssembly Component runtime first;
- OCI backend later;
- native/remote backends only behind the same Capability/Binding contract.

Capability state is disposable; user/Workspace state is not.

## 5. Semantic Build Profile

Gentoo-like specialization is exposed through a typed semantic profile rather than raw USE/compiler flags.

Example:

```text
profile: development-balanced

intent:
  development: high
  containers: enabled
  ai: local-optional
  gaming: disabled
  battery: balanced
  legacy-x11: minimal
  bluetooth: enabled

priorities:
  reliability > latency > energy > build-time
```

The backend may derive:

- package feature choices;
- dependencies;
- build variants;
- compiler target/features;
- services;
- kernel modules/configuration;
- runtime/Capsule availability.

### Expert Overlay

Expert mode can override derived values, but overrides are versioned Blob inputs, not silent out-of-band mutation.

The UI must be able to expose:

```text
semantic choice
 -> generated Blob/SystemSpec field
 -> backend Nix expression/module
 -> resulting package/build/kernel/service choice
```

## 6. Binary-first, source-when-valuable

The pilot rejects source compilation as ideology.

Resolution order:

```text
requested build identity
        |
matching trusted binary/cache artifact?
      /   \
    yes    no
     |      |
   reuse   reproducible build
             |
        cache artifact
```

Hardware/workload-specific builds are justified when they:

- enable a requested feature unavailable in generic builds;
- remove a meaningful unwanted dependency/feature;
- materially improve a measured objective;
- are required for compatibility/security.

The Technician should be able to benchmark a generic artifact against an optimized candidate before recommending adoption.

## 7. Update/channel model

The Blob exposes a policy layer independent from distro repository names.

Initial channels:

```text
Stable   -> default for Ready
Testing  -> passed automated/compatibility gates, newer
Edge     -> newest/upstream-oriented, explicit risk
```

Channels may be selected per domain/component when dependency validity permits:

```text
base system -> Stable
kernel      -> tested hardware-enablement lane
Mesa        -> Testing
one dev tool-> Edge
```

The resolver/Technician must prevent unsupported partial-state combinations rather than merely warn after breakage.

## 8. Hardware onboarding

Pilot onboarding should eventually provide a mainstream-style hardware report before activation/installation.

Minimum information:

- CPU/architecture/features;
- RAM;
- storage/controller/SMART capability where available;
- GPU(s) and driver path;
- Wi-Fi/Bluetooth;
- audio;
- display/refresh capabilities;
- battery/power devices on laptops;
- virtualization support;
- firmware/UEFI state relevant to the installation;
- known unsupported/degraded hardware.

The Technician explains relevant choices in human terms and exposes primary documentation when a user wants details.

## 9. System change lifecycle

No privileged system recommendation should default to direct live mutation.

```text
trigger / user intent / Improvement Watch
        |
Technician explanation + evidence
        |
candidate SystemSpec branch
        |
materialize/build
        |
static validation
        |
VM/sandbox/test where meaningful
        |
benchmark / regression checks
        |
authorization gate
        |
activate candidate generation
        |
observe actual result
        |
commit or rollback
```

## 10. Rollback and causal history

The pilot presents **one** user-facing time model even if several mechanisms exist underneath.

Potential backend evidence:

- Nix system generations;
- Btrfs/Snapper-style filesystem checkpoints;
- Workspace/Blob object versions;
- causal records.

User-facing model:

```text
main
 |
 A  baseline
 |
 B  development profile
 |\
 | C  battery experiment
 |  \
 |   D  new kernel candidate
 |
 E  accepted state
```

Each meaningful system transition records:

- what changed;
- why;
- trigger/user goal;
- who/which agent proposed it;
- upstream/local evidence;
- predicted benefit;
- tests/benchmarks;
- actual outcome;
- side effects;
- approval;
- rollback target.

## 11. Drift and manual changes

Manual editing is not prohibited.

The Blob distinguishes:

```text
DECLARED    represented in SystemSpec/Expert Overlay
MANAGED     generated by a backend
OBSERVED    actual machine state
DRIFT       observed != expected
```

Expert changes made outside normal tooling should be detected where practical and offered for:

- import into an Expert Overlay;
- intentional ignore scope;
- revert to declared state.

No automatic overwrite without explanation.

## 12. Service model

The initial substrate remains systemd, but Blob must provide a simpler semantic view inspired by runit/Slackware transparency.

For every important service, Technician/Expert views should expose:

- why it is enabled;
- executable/implementation;
- dependency relationship;
- startup state;
- environment/resources;
- logs;
- restart policy;
- package/source owner;
- last relevant configuration change.

The product must not use “systemd is complex” as justification for hiding system state.

## 13. System Technician responsibilities in Pilot v0.1

At minimum the Technician can:

- inventory and explain the current system;
- explain how an active semantic profile maps to actual configuration;
- compare current state to a proposed SystemSpec candidate;
- diagnose at least one real class of regression from Alfred/system evidence;
- propose improvements without self-authorizing them;
- attach official/upstream references to privileged update proposals;
- prepare a candidate generation;
- show risk/test/rollback plan;
- explain outcome after activation.

Local reasoning is preferred. AI unavailability must not prevent deterministic operation/recovery.

## 14. Installation/onboarding modes

### Ready

Question set is intentionally small:

```text
What is this machine mainly for?
Everyday / Development / Gaming / AI / Creative / Server
```

Hardware is probed, a curated profile is selected and only important exceptions are surfaced.

### AI Designed

Technician interviews the user about goals and trade-offs, then proposes a profile with explicit consequences.

### Expert

Expose:

- packages/components;
- channels;
- semantic build features and lower-level overrides;
- kernel/modules/parameters;
- services;
- filesystem/storage strategy;
- Experience Profile;
- Capsule/runtime settings;
- generated backend state.

Expert mode must use the same safety/versioning/rollback pipeline rather than bypassing it by default.

## 15. Initial curated profiles

Pilot v0.1 should provide at least:

1. **Balanced Desktop** — conservative, general-purpose default.
2. **Development** — compiler/dev/containers/terminal-oriented, still reliable.
3. **Laptop Battery** — power-conscious profile preserving normal desktop functionality.

A fourth **Minimal/Expert Base** profile is desirable but not required for the first usability proof.

## 16. Measurable Pilot v0.1 demonstrations

The pilot is not successful because configuration files exist. It must demonstrate user-visible loops.

### Demo A — profile construction

User chooses Ready/AI Designed/Expert -> Blob produces a valid SystemSpec -> materializes a bootable candidate.

### Demo B — explainability

User asks “why is this service/package/kernel option present?” -> Technician traces semantic choice to concrete backend state.

### Demo C — safe update

New kernel/package candidate -> branch/generation -> validation -> user sees diff/risk -> activation -> rollback remains available.

### Demo D — Gentoo-like specialization

For at least one real component, semantic feature selection changes the build/dependency graph. A matching binary is reused when available or a reproducible build is generated.

### Demo E — evidence-driven optimization

At least one candidate optimization is benchmarked against baseline; unhelpful change is rejected or helpful change is accepted based on measured result.

### Demo F — normal desktop compatibility

At least one standard GUI application and one ordinary CLI workflow work without being converted into native Blob Capabilities.

## 17. Exit criteria

Pilot v0.1 is complete when all are true:

- one x86_64 UEFI Linux reference machine/VM can be reproducibly materialized;
- Ready/AI Designed/Expert produce valid versioned configuration inputs;
- three curated profiles exist;
- Technician can explain real system state;
- at least one system candidate can be created/tested/activated/rolled back;
- causal history connects proposal -> evidence -> generation -> outcome;
- at least one semantic build-feature proof exists;
- standard Linux applications remain usable;
- Capsule runtime remains isolated from base-system package semantics;
- user can always inspect the generated low-level configuration;
- AI failure does not prevent boot/recovery/manual administration.

## 18. Explicit non-goals for v0.1

Not required yet:

- custom Linux kernel written by The Blob;
- AI-generated kernel patches;
- custom Smithay compositor;
- replacing systemd;
- compiling every package from source;
- proprietary universal package format replacing all ecosystem formats;
- multi-node Compute Fabric;
- macOS/Windows/Android/Garmin nodes;
- full Knowledge Object filesystem replacement;
- autonomous cloud reasoning;
- fully automatic privileged update activation.

## 19. Immediate implementation order

The engineering roadmap can continue underneath this product target:

```text
1. finish deny-by-default Capsule runtime / explicit grants
2. preserve Capability vs system-package separation
3. define Linux SystemSpec v0.1
4. implement NixOS backend for a tiny controlled subset
5. implement generation/diff/rollback facade
6. expose current/generated state to System Technician
7. implement first Ready Development profile
8. add AI Designed translation into the same typed profile model
9. add Expert Overlay
10. perform first real system candidate experiment
11. build onboarding/Surface around proven operations
```

The exact order may adjust from implementation evidence, but no step should violate the distribution-audit principles.

## 20. Success definition

The Blob Linux Pilot should feel like:

> Arch freedom + Gentoo specialization + Nix reproducibility + Atomic/openSUSE recovery + Ubuntu-level approachability + Debian discipline + Slackware inspectability.

The goal is not to expose that complexity. The goal is to make it available safely when the user wants it.
