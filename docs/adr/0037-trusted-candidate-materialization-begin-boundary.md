# ADR-0037: Begin materialization from a trusted candidate manifest id

## Status

Accepted for validation in a disposable NixOS VM. Physical-node execution remains disabled.

## Context

ADR-0035 bound materialization admission to a root-predicted derivation/output. ADR-0036 then proved that a pending materialization can survive reboot and feed the exact resulting admission into the root prepared-request publisher.

One upstream trust gap remained: the validation harness for `begin` still accepted candidate id, SystemSpec id, immutable source and installable attribute as separate command-line inputs. A production D-Bus caller must not be allowed to nominate those security-sensitive fields independently.

Moving those same strings into a different D-Bus method would not improve the trust boundary.

## Decision

Introduce a root-owned `TrustedMaterializationCandidate` manifest and a production-shaped root begin coordinator.

The public materialization begin request accepts only one opaque `manifest_id`.

Root loads the corresponding canonical manifest from a mode-0700 trusted-candidate directory. Each manifest is a root-owned mode-0600 ordinary file and contains:

- manifest id;
- candidate id;
- SystemSpec id;
- canonical immutable flake root;
- bounded installable attribute;
- provenance lines describing the upstream admission context.

The caller does not supply candidate, SystemSpec, source, installable, derivation, output, materialization operation id or timestamp.

Root then:

1. validates the trusted manifest file and canonical serialization;
2. canonicalizes and validates the immutable store source;
3. independently resolves the exact derivation and expected output;
4. generates a fresh materialization operation id internally;
5. retains the exact derivation through a root-owned Nix GC root;
6. creates the durable pending materialization intent through the ADR-0035 authority;
7. requires the persisted derivation/output identity to match the independently resolved identity byte-for-byte.

Completion is also owned by the coordinator: the pending derivation GC root must still match, ADR-0035 independently verifies the realized output, and the GC root is released only after the admission has been created successfully.

## Public IPC shape

The VM proof exposes only:

```text
Begin(manifest_id: string) -> (operation_id, evidence)
```

The system bus unique sender name is derived from the D-Bus message header for audit evidence. Materialization is still a `User`-authority, non-live effect, so this checkpoint does not add a host-administrator polkit grant merely to request realization of an already trusted candidate.

A caller attempting to append another D-Bus argument for source, closure or similar identity data is rejected by the D-Bus signature before the root coordinator is invoked.

## Required proof

The disposable KVM test must prove:

1. the trusted candidate directory is root-owned mode 0700;
2. a valid manifest is root-owned mode 0600 and unreadable by an ordinary user;
3. a missing manifest fails before pending intent or GC-root creation;
4. a manifest with unsafe file mode fails before pending intent or GC-root creation;
5. the public D-Bus method accepts exactly one input string;
6. the root-generated operation id is not supplied by the caller;
7. candidate, SystemSpec, source and installable returned in evidence match the trusted manifest;
8. the pending intent is root-owned mode 0600 and unreadable by the ordinary user;
9. exactly one GC root retains the exact root-selected derivation;
10. the ordinary user can realize only the returned `<drv>^out` target;
11. root completion admits exactly that output and releases the pending GC root.

## Trust boundary intentionally not solved here

This ADR does **not** claim that any process may create a trusted candidate manifest.

The manifest producer is the next upstream boundary. It must eventually derive or verify candidate/SystemSpec/source identity from deterministic SystemSpec translation and control-plane history rather than copying arbitrary caller fields into a root-owned file.

Until that producer is defined, the KVM fixture stages manifests as root solely to prove the consumer boundary.

## Safety boundary

This checkpoint still does not permit:

- caller-selected materialization operation ids;
- caller-selected source, installable, derivation or output paths at `Begin`;
- caller-selected privileged closure/program/argv;
- persistent NixOS `switch` or `boot`;
- mutable-source `nixos-rebuild`;
- physical-node execution.

## Consequence

The production-shaped materialization API can now be designed around stable object identity instead of a bag of privileged strings. This also improves modularity: upstream planners/Technician/control-plane components may evolve independently as long as they produce the same trusted candidate manifest contract, while the root materialization module remains a narrow reusable capability.

The next checkpoint should define and prove the trusted manifest producer from validated SystemSpec/candidate translation and causal history, then compose that producer with this begin boundary without introducing an alternate path for arbitrary native configuration.
