# SystemSpec v0.1

**Status:** first executable semantic contract for the Linux Pilot.

## 1. Purpose

`SystemSpec` is The Blob's backend-neutral description of the desired system state. It exists so Ready, AI Designed and Expert users manipulate the same semantic model while NixOS remains a replaceable materialization backend.

```text
user goals / Technician / Expert choices
                |
                v
         SemanticBuildProfile
                |
                v
            SystemSpec
                |
        validate deterministically
                |
                v
        backend translation
       /         |          \
    NixOS      future      hosted
              backend      substrate
```

`SystemSpec` must not contain raw Nix, shell fragments or arbitrary AI-generated configuration text.

## 2. v0.1 fields

The first Rust domain model contains:

- stable `SystemSpecId`;
- hostname;
- architecture (`x86_64` / `aarch64`);
- base channel (`Stable`, `Testing`, `Edge`);
- kernel policy (`DistributionDefault`, `LatestSupported`);
- `SemanticBuildProfile`;
- optional `ExperienceProfileId`.

The profile contains:

- `Ready`, `AiDesigned` or `Expert` construction mode;
- ordered optimization priorities;
- typed feature selections (`Enabled` / `Disabled`).

Initial priorities:

```text
Reliability
Security
Latency
Energy
Memory
BuildTime
```

## 3. Semantic features

Features are stable IDs rather than Nix option names. Initial NixOS backend mappings include:

```text
bluetooth  -> hardware.bluetooth.enable
containers -> virtualisation.podman.enable
flatpak    -> services.flatpak.enable
hyprland   -> programs.hyprland.enable
pipewire   -> services.pipewire.enable
printing   -> services.printing.enable
ssh        -> services.openssh.enable
```

This list is intentionally small. New semantic features require an explicit contract/mapping instead of silently passing through arbitrary backend configuration.

## 4. Validation

The trusted core currently rejects:

- invalid hostnames;
- duplicate feature declarations;
- duplicate optimization priorities.

Validation happens before backend translation.

Backend-specific support is then checked separately. A valid semantic feature that a backend cannot implement produces an explicit backend error.

## 5. NixOS backend v0.1

The first backend emits a readable NixOS module and a translation trace.

Example semantic input:

```text
hostname = blob-pilot
architecture = x86_64
channel = Stable
kernel = LatestSupported
bluetooth = enabled
pipewire = enabled
printing = disabled
hyprland = enabled
```

Produces conceptually:

```nix
{ pkgs, ... }:
{
  networking.hostName = "blob-pilot";
  nixpkgs.hostPlatform = "x86_64-linux";
  boot.kernelPackages = pkgs.linuxPackages_latest;
  hardware.bluetooth.enable = true;
  programs.hyprland.enable = true;
  services.pipewire.enable = true;
  services.printing.enable = false;
}
```

The translation trace records semantic source -> backend target for inspectability and System Technician explanations.

## 6. v0.1 channel constraint

The minimal NixOS backend initially materializes only the `Stable` base channel. `Testing`/`Edge` are valid semantic concepts but the backend returns `UnsupportedChannel` until we define reproducible flake/input promotion semantics.

This prevents a placeholder implementation from pretending to support channel mixing.

## 7. Expert escape hatch

Expert mode does **not** mean raw Nix is the canonical state. Expert choices should become structured `SystemSpec` fields/overrides whenever they affect The Blob-managed system.

A future explicit backend escape hatch may allow advanced native configuration, but it must be:

- clearly marked as backend-specific;
- represented in causal history;
- inspectable;
- excluded from claims of cross-backend portability.

## 8. Next Linux Pilot steps

After the v0.1 translation is CI-validated:

1. materialize the generated module through a reproducible NixOS flake/reference image;
2. expose `build`, `test`, `build-vm`, `dry-activate`, `boot` and `switch` as controlled backend operations;
3. connect candidate generations to Temporal/Causal history;
4. add before/after system benchmark evidence;
5. let the System Technician explain the translation and propose one safe candidate change.
