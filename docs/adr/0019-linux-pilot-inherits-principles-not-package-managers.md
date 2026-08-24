# ADR-0019 — Linux Pilot inherits principles, not package managers

**Status:** Accepted

## Context

The Blob Linux pilot intentionally studies Arch, Gentoo, NixOS, Fedora Atomic, openSUSE, Ubuntu, Debian, Slackware, Alpine and Void. Each contains mechanisms worth learning from, but directly combining their package managers, release models and administration interfaces would create an incoherent product.

The user-facing goal is one system model with Ready, AI Designed and Expert paths.

## Decision

The Blob will inherit **design principles and proven behaviors**, not expose a collection of distro-native package-management models.

The initial reference implementation uses:

- NixOS/Nix as the first declarative full-system backend;
- systemd as the first Linux service substrate;
- Nix generations plus a filesystem-checkpoint adapter for rollback/recovery experiments;
- normal Linux application compatibility, with Flatpak/native packaging used pragmatically;
- Blob Capsule runtimes for native Blob Capabilities;
- semantic build profiles inspired by Gentoo USE/profile concepts;
- Stable/Testing/Edge policy inspired by Debian-style promotion rather than by one repository layout;
- upstream-first and full inspectability inspired by Arch/Slackware;
- mainstream onboarding/hardware UX inspired by Ubuntu;
- deployment/rollback safety inspired by Fedora Atomic/openSUSE;
- small recovery/trusted components and drift detection inspired by Alpine/Void-style simplicity.

## Core invariant

```text
one Blob semantic model
        |
replaceable Linux mechanisms underneath
```

Nix, Flatpak, Btrfs, systemd, Wasmtime and Hyprland are implementation choices, not product abstractions.

## Expert mode

Expert mode exposes real generated system state and supports versioned overrides. It must not create a second untracked configuration universe.

Out-of-band manual changes are permitted but should be detectable as drift and importable into a versioned Expert Overlay where practical.

## Consequences

### Positive

- one coherent UX instead of multiple package-manager mental models;
- reuse of proven Linux ecosystem mechanisms;
- compatibility with ordinary Linux applications;
- substrate choices remain replaceable;
- deep customization remains available without burdening Ready users;
- distribution research can continue without forcing migrations of the semantic core.

### Negative

- adapter/backend work is required;
- some distro-native features will initially be approximated rather than reproduced exactly;
- semantic build features require a mapping layer to Nix/package-specific options;
- rollback may use multiple internal mechanisms that must be presented as one user model.

## Rejected alternatives

### Use Arch directly as the product

Rejected because manual configuration and rolling-release maintenance would remain part of the normal user burden.

### Use Gentoo directly as the product

Rejected because compilation/USE-flag expertise and build cost would remain too visible.

### Expose NixOS directly as the product model

Rejected because the Nix language and Nix-specific concepts should not be prerequisites for ordinary users.

### Invent a new package manager immediately

Rejected as unnecessary scope. The Blob should prove semantic/workflow value before replacing mature package/storage systems.

## References

See:

- `../research/DISTRO-AUDIT.md`
- `../BLOB-LINUX-PILOT-v0.1.md`
- `../PILOT-ROADMAP.md`
- `../FAILURE-LESSONS.md`
