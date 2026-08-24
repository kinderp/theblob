# Architecture v0.5 delta — System Technician

**Status:** accepted delta over `ARCHITECTURE.md` v0.4. It will be folded into the next consolidated Architecture Freeze.

Architecture v0.5 adds a persistent **System Technician** and a model-agnostic **AI Broker** without changing the v0.4 authority model.

## New flow

```text
Alfred events / trusted-source refresh
            |
            v
        Situation
            |
            v
     System Technician
            |
 local evidence + external provenance
            |
            v
  applicability analysis
            |
            v
   ImprovementProposal
            |
      policy / authority
            |
      prepare / explain
            |
 branch -> build -> simulate/test/benchmark
            |
       activation gate
            |
          verify
        /        \
     commit     rollback
            |
     causal evidence
```

## Improvement Watch

The Technician may proactively consult trusted technical sources, preferring official/upstream kernel, driver, package and project documentation, release notes, hardware-vendor material, signed repository metadata and security advisories.

A new version is not intrinsically an improvement. It is surfaced when external evidence intersects local hardware, workloads, known problems, policies, objectives or causal history.

Every privileged proposal should include:

- trigger/Situation and local evidence;
- external provenance and direct official references where available;
- applicability reasoning;
- proposed changes;
- expected benefits and uncertainty;
- risks and compatibility constraints;
- test/benchmark plan;
- authority requirement;
- rollback reference;
- revalidation/expiration conditions.

## Autonomy

```text
OBSERVE  diagnose/discover
SUGGEST  explain/propose
PREPARE  download/build/test candidate
APPLY    activate only inside explicit policy
FORBID   action class unavailable
```

Preparation may be more autonomous than activation. Kernel, driver and security-boundary changes default conservatively.

## AI Broker

The Technician is one logical role while reasoning capabilities may bind to:

```text
resident local model
        -> stronger local model
        -> trusted Fabric model
        -> optional cloud model
```

Binding respects privacy/data residency, required quality, latency, cost, energy and available hardware. High-end local inference is optional, not a minimum system requirement.

Cloud models receive only policy-approved Projections/minimal context and return proposals; they never receive ambient root authority, reusable credentials or automatic execution rights.

## Invariants

- AI/network evidence proposes; deterministic policy/verifiers authorize.
- Network discovery never directly mutates the system.
- Important proposals expose provenance and official source references.
- Candidate system changes remain branchable, testable, benchmarkable and rollback-capable.
- The Blob must continue boot, data access, Workspace operation, deterministic resolution and recovery when AI is unavailable.

See also `SYSTEM-TECHNICIAN.md`, ADR-0015 and ADR-0016.
