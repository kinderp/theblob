# ADR-0021: SystemSpec is backend-neutral; NixOS is a materializer

**Status:** Accepted

## Context

The Linux Pilot needs declarative, reproducible system construction and safe candidate generations. NixOS is an excellent first backend, but making Nix expressions the native user/system model would couple The Blob's semantics to one substrate and expose backend complexity directly to AI/non-expert users.

## Decision

The Blob owns a typed backend-neutral `SystemSpec` in `blob-core`.

Ready, AI Designed and Expert flows all produce the same semantic `SystemSpec`/`SemanticBuildProfile` model.

NixOS lives behind a deterministic translation backend. The backend:

- validates backend support;
- emits inspectable NixOS module text;
- records a semantic-to-Nix translation trace;
- fails explicitly for unsupported features/channels;
- never silently ignores a semantic request.

Raw Nix is not accepted from an LLM as canonical managed state.

## Consequences

- `NixOS != The Blob` remains enforceable in code;
- future substrates can implement the same semantic contract;
- the System Technician can explain what a semantic choice became on NixOS;
- Expert mode remains inspectable without making every user learn Nix;
- unsupported semantic requests fail early instead of producing partial systems;
- cross-backend portability claims can be based on structured fields rather than free-form configuration.

## Initial supported NixOS translations

- architecture and hostname;
- distribution-default/latest-supported kernel policy;
- Bluetooth;
- Podman containers;
- Flatpak;
- Hyprland;
- PipeWire;
- printing;
- OpenSSH.

The backend initially supports only the Stable base channel until reproducible input/channel semantics are specified.
