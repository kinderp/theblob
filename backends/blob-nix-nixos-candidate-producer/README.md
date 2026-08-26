# `blob-nix-nixos-candidate-producer`

This module is the trusted producer upstream of the materialization-begin boundary.

Its public input is a **canonical backend-neutral `SystemSpec`**, not native Nix, a store path, a candidate id, a manifest id, or a materialization operation id.

The root producer:

1. parses a strict versioned `SystemSpec` representation;
2. runs `SystemSpec::validate()`;
3. re-runs the deterministic `NixOsBackend` translation;
4. builds an immutable candidate source from trusted service-owned nixpkgs and base-module inputs plus the generated module;
5. generates candidate, manifest and causal identifiers internally;
6. writes a root-owned trusted materialization manifest and a root-owned causal receipt;
7. retains the immutable candidate source with a root-owned Nix GC root until a later manifest-lifecycle checkpoint retires it.

The module deliberately has no raw-Nix or shell escape hatch. AI, the System Technician, Ready/AI Designed/Expert UX, or another control-plane component may propose semantic `SystemSpec` values, but they cannot nominate the native source consumed by root.

The current Linux Pilot still treats the node-specific base module (filesystem/boot scaffolding) and pinned nixpkgs source as trusted service configuration. General node-profile derivation and manifest retirement/quota/orphan cleanup are later checkpoints.
