# ADR-0005: Causal history is a first-class OS primitive

**Status:** provisional

## Decision
State changes worth preserving must support history with parents plus causal metadata such as WHAT, WHY, WHO/agent, TRIGGER, EVIDENCE, expected effect, actual effect and rollback information.

Nix generations, Btrfs snapshots and Knowledge Object versions are implementation mechanisms beneath one user-facing Temporal/Causal Graph, not separate conceptual histories.
