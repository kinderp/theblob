# Phase 1 bootstrap validation checkpoint

This checkpoint exists to validate the first Rust vertical-slice bootstrap through the pull-request CI path before graphical work begins.

Included on `main` before this checkpoint:

- Architecture Freeze v0.1;
- Development Workspace v0.1;
- Event/Situation Contract v0.1;
- `blob-core` semantic domain model;
- deterministic `blob-alfred` source-change correlation;
- deterministic `blob-resolver` + independent BindingVerifier;
- MVP-only `blob-executor` LocalProcessCapsule;
- append-only `blob-history` causal log;
- `blob-mvp` end-to-end source-change -> test.run -> causal-record harness.

The purpose of this branch is CI validation only. No new architecture is introduced here.
