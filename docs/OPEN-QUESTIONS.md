# Open Questions

These are intentionally unresolved.

1. Public positioning/tagline and whether the project name needs a future trademark/domain review before a public release.
2. Exact boundary between Personal World and node-local state.
3. Consistency model for offline/multi-node state.
4. Capability contract format: WIT-first, own IDL, or typed internal model with multiple ABI adapters.
5. Exact Requirement Graph constraint/optimization formalism: custom typed predicates, SMT, Datalog/logic, or a deliberately small deterministic evaluator; and how hard constraints, preferences and multi-objective ranking compose.
6. How to model Capability effects so policy can reason about them deterministically.
7. First sandbox backend for MVP-0.
8. Persistent store for early Personal World / Temporal Graph.
9. Object identity and content-addressing strategy.
10. Trust/reputation model for future Capability and Workspace registries.
11. How much of AI Designed Workspace creation is deterministic policy vs model-generated proposal.
12. Surface schema: custom typed Rust model, external declarative schema, or hybrid.
13. Constitutional Core threat model and privileged boundary.
14. How system branches combine Nix generations, filesystem snapshots, object history and causal metadata.
15. Fabric architecture: Plan B-like peer-to-peer, Octopus-like central Personal World control plane, or hybrid.
16. Binding Lease semantics and safe boundaries for late re-binding/migration of running Tasks, especially non-idempotent or irreversible effects.
17. When an AI-synthesized adapter becomes cacheable/reusable and what trust state it receives.
18. Whether Workspace composition should expose an explicit namespace/bind model inspired by Plan 9.
19. How human-readable delegated authority maps to low-level Linux/Android security primitives.
20. How much legacy application compatibility belongs in the first usable release.

21. Minimal universal compatibility/introspection bridge: virtual file trees/9P, structured CLI, JSON/CBOR, or multiple projections.
22. Type compatibility model: nominal, structural, semantic ontology, or layered combination.
23. How derived Representation dependency invalidation interacts with offline replicas and Temporal/Causal history.
