# System Candidate Operations v0.1 validation checkpoint

CI must prove:

- canonical action -> effect/authority mapping in `blob-core`;
- forged action policy is rejected;
- NixOS planning uses structured program + argv rather than shell strings;
- `Materialize` uses direct non-activating Nix build semantics;
- `BuildIsolatedVm` targets `system.build.vm`;
- `PreviewActivation` is classified as `PreviewHooks` + administrator authority;
- `TestActivation` is classified as temporary live mutation + administrator authority;
- invalid NixOS configuration names are rejected;
- no v0.1 action can generate persistent `switch` or `boot` activation;
- existing WASIp2, Slint, NixOS evaluation and Linux VM boot smoke checks remain green.

No privileged host activation is performed by this checkpoint.
