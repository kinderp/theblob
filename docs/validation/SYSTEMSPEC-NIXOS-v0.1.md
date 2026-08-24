# SystemSpec / NixOS backend v0.1 validation checkpoint

This checkpoint validates the first Linux Pilot system-construction seam.

CI must prove that:

- the backend-neutral `SystemSpec` domain model compiles on the Rust 1.85 core MSRV;
- valid specs pass deterministic validation;
- malformed hostnames are rejected;
- duplicate semantic features/priorities are rejected;
- the minimal NixOS backend deterministically translates supported features;
- translation produces an inspectable semantic-to-Nix trace;
- unsupported semantic features are explicit errors and are never silently ignored;
- unsupported channels are explicit errors;
- the NixOS backend remains a materializer outside the semantic model;
- the checkpoint remains green on top of the CI-validated Phase 2B WASIp2 explicit-grant runtime.

This checkpoint does **not** yet execute `nixos-rebuild`. The next materialization slice will place the emitted module inside a reproducible NixOS flake/reference image and exercise build/test/build-vm operations.
