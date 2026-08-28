# `blob-nix-nixos-system-workspace`

Composition adapter between the backend-neutral System Workspace proposal model and the NixOS candidate-preparation boundary.

The adapter is intentionally **read-only**. It can:

- translate the baseline and proposed `SystemSpec` values with `NixOsBackend`;
- expose the semantic diff separately from the native generated module;
- render the proposed canonical `SystemSpec` accepted by the trusted root candidate producer;
- expose deterministic semantic-to-Nix translation evidence.

It cannot:

- execute `nix` or `nixos-rebuild`;
- call D-Bus;
- create root manifests;
- authorize or activate a candidate.

For the first product demo, `bluetooth` is a real supported semantic feature. The System Workspace sees only:

```text
feature:bluetooth: disabled -> enabled
```

The adapter/backend may explain that this becomes `hardware.bluetooth.enable = true`, but native Nix never becomes the authority input emitted by the renderer or the System Workspace.
