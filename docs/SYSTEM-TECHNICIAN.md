# System Technician v0.1

**Status:** architectural concept for The Blob.

## Purpose

The System Technician is the always-available system-engineering role of The Blob. Its purpose is to preserve the freedom and deep optimization potential associated with systems such as Arch and Gentoo while removing the requirement that every user personally master package internals, kernel configuration, driver debugging, build flags and system administration.

The Technician should be able to teach an expert what it is doing and make a non-expert safe.

## Responsibilities

- explain system state in human terms;
- diagnose faults and performance/energy regressions using evidence;
- help design Ready, AI Designed and Expert Workspaces;
- produce candidate SystemSpec changes, including package/build options and eventually kernel/driver changes;
- benchmark alternatives instead of assuming that a tweak is beneficial;
- correlate Alfred Situations with local causal history;
- discover relevant upstream improvements/security fixes;
- provide provenance and direct official documentation/release-note references;
- prepare isolated candidate generations/branches;
- request authorization according to policy;
- verify outcomes after activation;
- commit causal evidence or rollback.

## Proactive loop

```text
Alfred events / periodic trusted-source refresh
                 |
                 v
             Situation
                 |
                 v
local evidence + external provenance
                 |
                 v
      applicability analysis
                 |
                 v
        ImprovementProposal
                 |
      policy / authority gate
          /             \
      explain          prepare
                         |
                  isolated candidate
                         |
                  test / benchmark
                         |
                   user/policy gate
                         |
                      activate
                         |
                       verify
                    /        \
                 commit     rollback
```

## Trusted-source discipline

The Technician may search the network, but it should distinguish evidence classes. Primary/official sources are preferred for privileged changes. Community discussion may be useful diagnostic evidence but should not silently become an execution authority.

Every external claim that materially motivates a system change should be traceable to provenance captured in the proposal/causal record.

## Update relevance

The Blob should not recreate the noisy “updates available” model. A new release becomes interesting when it changes a constraint or improves an observed/declared objective of the Personal World. Examples:

- kernel release fixes a battery regression observed on the user's hardware;
- GPU driver fixes a crash visible in causal history;
- new compiler materially improves the active Development Workspace;
- security advisory intersects an installed/exposed capability;
- new implementation satisfies the same Capability with lower cost/energy;
- upstream option enables removing a workaround currently carried by the system.

## AI Broker

The Technician is model-agnostic. `reason.system.*` requirements may bind to:

1. a small resident local model;
2. a stronger local model when hardware permits;
3. a stronger model on another trusted Fabric node;
4. an explicitly allowed cloud provider.

Model routing is itself a Capability binding problem. The logical Technician identity, memory, policy and authority remain local to The Blob.

## Safety invariants

- AI proposes; deterministic policy/verifiers authorize.
- Network discovery never directly mutates the system.
- Official documentation links/provenance accompany important update proposals.
- The system can prepare/test more autonomously than it activates.
- Privileged changes should be branchable, benchmarkable and rollback-capable.
- Cloud models receive minimal authorized Projections, not ambient system access.
- AI unavailability must not prevent boot, recovery or ordinary deterministic operation.

## Example: battery-related kernel improvement

The Technician observes increased discharge on a laptop and correlates it with hardware/driver/kernel telemetry. During Improvement Watch it finds an official upstream kernel change relevant to that exact path. It does not simply announce “new kernel available”. It explains:

- what local symptom it is trying to improve;
- why the upstream change appears applicable;
- which official source supports the claim;
- what will change;
- compatibility risks;
- how it will test battery/runtime before and after;
- how to roll back;
- whether the current policy allows preparation or requires approval before any step.

The user can choose a guided mode in which the Technician teaches each step, or permit preparation/testing and approve only final activation.
