# blob-nix-nixos-materialization-begin

Production-shaped root boundary for starting one NixOS materialization from a trusted candidate manifest.

The public caller selects only a `manifest_id`. Candidate/SystemSpec/source/installable identity is loaded from a root-owned canonical manifest; the materialization operation id and timestamp are generated inside the root boundary. The module also owns the pending derivation GC-root lifecycle used by the materialization admission authority.

This crate intentionally does not define who is allowed to *produce* trusted candidate manifests. That upstream admission boundary is separate and must derive/verify manifests from validated control-plane state rather than arbitrary caller strings.
