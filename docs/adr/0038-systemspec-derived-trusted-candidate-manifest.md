# ADR-0038: Derive trusted candidate manifests from canonical SystemSpec

## Status

Accepted for validation in a disposable NixOS VM. Physical-node execution remains disabled.

## Context

ADR-0037 narrowed the privileged materialization-begin API to one opaque trusted candidate manifest id. That removed caller control over candidate id, SystemSpec id, immutable source, installable attribute, derivation, output and materialization operation id at `Begin`.

One upstream trust gap remained: the ADR-0037 KVM fixture still staged the trusted manifest as root. Merely adding a privileged API that copied caller-supplied candidate/source fields into that file would move the same trust problem one step earlier.

The Blob already has a backend-neutral `SystemSpec` and a deterministic NixOS translation. `SystemSpec` deliberately contains no raw Nix, shell fragments or arbitrary native configuration. That semantic model is therefore the correct public proposal boundary.

## Decision

Introduce a root-owned SystemSpec candidate producer.

The producer accepts a single canonical, versioned `SystemSpec` representation plus the unique D-Bus sender derived by the root service. It does **not** accept:

- candidate id;
- manifest id;
- Nix source path;
- installable attribute;
- derivation/output path;
- materialization operation id;
- raw Nix or shell text.

Root then:

1. parses the strict canonical SystemSpec format;
2. rejects trailing/unknown/native fields;
3. runs `SystemSpec::validate()`;
4. re-runs `NixOsBackend::translate()` itself;
5. constructs a candidate flake from only trusted service-owned inputs:
   - the pinned nixpkgs store source;
   - the trusted node/base module;
   - the deterministic generated module;
6. copies that candidate source into `/nix/store`;
7. generates candidate id, manifest id and causal-record id internally;
8. derives the only materialization installable from the validated hostname:
   `nixosConfigurations.<hostname>.config.system.build.toplevel`;
9. publishes the canonical root-owned ADR-0037 manifest;
10. publishes a root-owned candidate-manifest receipt containing the canonical SystemSpec and translation evidence;
11. retains the immutable candidate source through a root-owned Nix GC root.

The resulting manifest can then be consumed unchanged by ADR-0037 `Begin(manifest_id)`.

## Canonical SystemSpec transport

The producer uses a versioned text form beginning with:

```text
theblob-system-spec-v1
```

It contains only fields already represented by the semantic `SystemSpec` contract: id, hostname, architecture, base channel, kernel policy, semantic profile, construction mode, priorities, typed feature selections and optional experience profile.

Feature entries are sorted canonically. Unknown fields or alternate serialization are rejected. This means a caller cannot append `raw-nix`, `--impure`, a source path or another native escape hatch while retaining a valid request.

The D-Bus proof exposes:

```text
PrepareCandidate(canonical_system_spec: string)
  -> (manifest_id, evidence)

Begin(manifest_id: string)
  -> (materialization_operation_id, evidence)
```

The root service obtains the requester identity from the D-Bus message header.

## Causal provenance

For each successful production root creates a durable candidate-manifest receipt binding:

- requester system-bus unique name;
- generated manifest id;
- generated candidate id;
- SystemSpec id;
- immutable candidate source;
- complete canonical SystemSpec;
- deterministic semantic-to-Nix translation evidence;
- root-generated causal record id and timestamp.

The producer also returns the equivalent backend-neutral `CausalRecord`. A future persistent causal-log checkpoint should append/link these records rather than treating the receipt directory itself as the final global history implementation.

## Source retention

A manifest must not outlive the Nix source it names. Therefore the producer creates a root-owned GC-root symlink for the exact candidate source before considering publication complete.

This checkpoint intentionally does not yet define manifest retirement, quotas or orphan-GC reconciliation. Until that lifecycle is introduced, a successfully published trusted candidate retains its immutable source.

## Trusted node-specific input

The current Linux Pilot `SystemSpec` does not describe every machine-specific boot/storage property. The reference `base.nix` still contains validation-node filesystem, boot-loader and `system.stateVersion` scaffolding.

That base module is **trusted service/node configuration**, not caller input. ADR-0038 does not pretend it is portable semantic state. A later checkpoint should replace the fixed reference base with a validated node/hardware profile derived from read-only observation and enrollment policy.

## Required proof

The disposable KVM test must prove:

1. the public producer accepts exactly one semantic SystemSpec string;
2. an extra caller-selected source/native argument is rejected by D-Bus;
3. an unknown/trailing native field makes the SystemSpec non-canonical and produces no manifest or source GC root;
4. a valid canonical SystemSpec is parsed and validated by root;
5. root-generated manifest/candidate/causal ids are not supplied by the caller;
6. the immutable source is created under `/nix/store` from trusted nixpkgs/base plus deterministic generated module;
7. `generated.nix` matches the existing deterministic backend translation byte-for-byte;
8. the candidate flake references only the trusted pinned nixpkgs source and `base.nix`/`generated.nix`, with no impure environment escape;
9. manifest and causal receipt are root-owned mode 0600 and unreadable by the ordinary requester;
10. a source GC root retains exactly the generated immutable source;
11. the generated manifest feeds ADR-0037 `Begin(manifest_id)` without changing candidate/SystemSpec/source/installable identity;
12. ADR-0037 still creates only the exact root-selected derivation/output and materialization GC root.

## Safety boundary

This checkpoint still does not permit:

- raw Nix or shell input as canonical system state;
- caller-selected native source/installable/derivation/output;
- caller-selected candidate/manifest/materialization-operation ids;
- persistent NixOS `switch` or `boot`;
- arbitrary mutable-source `nixos-rebuild`;
- physical-node execution.

## Consequence

The Linux Pilot now has a provenance-preserving semantic front door:

```text
Technician / UI / control-plane proposal
              |
              v
       canonical SystemSpec
              |
              v
 root validate + deterministic translate
              |
              v
 trusted immutable candidate manifest
              |
              v
     ADR-0037 Begin(manifest_id)
```

This preserves modularity: the AI Technician or another standalone planner may propose semantic system intent, while the trusted root producer remains deterministic and AI-independent.

After this proof is green, the next work is candidate-manifest/source lifecycle (retirement, quota and orphan reconciliation), persistent causal-log linkage, and replacement of fixed reference-machine base scaffolding with a trusted node-specific profile before physical-hardware enablement is reconsidered.
