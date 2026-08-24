# ADR-0001: Linux + NixOS as initial PC/server substrate

**Status:** provisional

## Decision
Use Linux and the existing driver ecosystem. Use NixOS initially on PC/server nodes because declarative system state, reproducible builds, generations and rollback align with the Adaptive System model.

## Non-goal
The Personal OS is not defined as “NixOS with an AI shell”. Nix is a backend/substrate that may later be replaced or supplemented.
