# ADR-0018 — Linux-first, mainstream-compatible, local-first pilot

**Status:** Accepted

## Context

The Blob draws heavily from Plan 9, Inferno, Plan B and related research systems, but those systems also show the risks of ecosystem incompatibility, latency-blind distribution, always-connected assumptions and abstractions that are elegant but costly for users.

The project needs an incremental path that demonstrates user value before attempting a complete distributed personal environment.

## Decision

The first product objective is a **Linux-first deep-control pilot**.

The goal is to combine:

- Arch-like transparency and freedom;
- Gentoo-like build/specialization potential;
- mainstream desktop ease of use;
- an always-available System Technician that explains, diagnoses, proposes, tests and rolls back changes.

Linux remains compatible with existing applications and hardware. The Blob does not initially require a new kernel, compositor or application ecosystem.

After the Linux pilot reaches useful product quality, heterogeneous Personal World support will be added incrementally:

1. second Linux/Ubuntu node;
2. macOS hosted node;
3. Windows hosted node;
4. Android mobile node;
5. Garmin wearable companion Surface/sensor integration.

Each platform must add measurable user value before the next is attempted.

## Architectural principles

- **mainstream-first:** integrate existing OS/application ecosystems rather than requiring migration;
- **local-first:** interactive state and operation remain useful without network/cloud;
- **latency-aware:** semantic location transparency does not imply ignoring physical locality;
- **logical centralization, physical distribution:** one coherent Personal World with distributed execution/storage as useful;
- **expert escape hatch:** AI/defaults simplify the system without taking away inspectability/control;
- **empirical specialization:** kernel/build/runtime changes require measurable benefit when practical;
- **AI is not authority:** deterministic policy/verifiers authorize privileged actions;
- **incremental heterogeneous Fabric:** devices enter one at a time.

## Consequences

Positive:

- early product value is testable on one machine;
- The Blob avoids requiring a new application ecosystem;
- Linux gives the Adaptive System full depth when required;
- hosted macOS/Windows/Android participation remains compatible with the Personal World vision;
- complexity is introduced only after earlier layers prove useful.

Trade-offs:

- the first pilot will not initially demonstrate the full multi-device vision;
- hosted nodes expose less system-control depth than the Linux reference node;
- some abstractions may be intentionally postponed despite being architecturally attractive.

## Related documents

- `../FAILURE-LESSONS.md`
- `../PILOT-ROADMAP.md`
- `../ROADMAP.md`
- `../SYSTEM-TECHNICIAN.md`
