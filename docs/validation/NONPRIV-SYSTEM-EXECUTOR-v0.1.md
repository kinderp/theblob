# Non-privileged system executor v0.1 validation checkpoint

CI must prove:

- the executor is part of the Rust 1.85 core workspace;
- `Materialize` and `BuildIsolatedVm` plans pass the executor boundary;
- preview/test activation plans are rejected;
- forged program/argument/derivation forms are rejected;
- `--impure` is rejected;
- Nix store paths are parsed into structured result artifacts;
- the reference `Materialize` operation is planned by `NixOsBackend`, executed by `NonPrivilegedNixExecutor`, and performs a real pinned NixOS build;
- the real result reports at least one `/nix/store/...` artifact;
- all existing core, Slint, WASIp2 and NixOS evaluation gates remain green.

No live host activation is allowed by this checkpoint.
