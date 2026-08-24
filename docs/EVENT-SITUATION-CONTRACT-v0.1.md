# Event and Situation Contract v0.1

**Status:** semantic contract for Alfred MVP-0 and future distributed Fabric events.

## 1. Purpose

Alfred is The Blob's nervous system. Its first responsibility is to turn heterogeneous signals into normalized, attributable events and then into structured Situations without confusing observation, interpretation and authority.

```text
raw signal
   |
adapter
   |
Normalized Event
   |
deterministic temporal correlation
   |
Candidate Situation
   |
optional semantic AI interpretation
   |
Structured Situation
   |
consumers: Task/Planner/System Technician/UI
```

A Situation is evidence for action. It is never itself permission to perform an action.

## 2. Normalized Event Envelope

Every normalized event should conceptually expose:

```text
EventEnvelope
  id
  schema_version
  kind
  source
  source_node
  subject_refs
  observed_wall_time
  source_monotonic_time/sequence when available
  received_time
  correlation_keys
  typed payload/attributes
  sensitivity/trust metadata
  provenance
  causal_parent_refs when known
```

### Stable event identity

Adapters should create stable/deduplicatable IDs when the source provides identity. Replayed transport delivery must not silently create duplicate semantic events.

### Time

The contract distinguishes:

- source/observed wall-clock time;
- receive time;
- monotonic/sequence order when available.

Distributed Fabric nodes may have clock skew. Correlation logic must not assume wall-clock timestamps establish total order.

### Source

Source categories include:

```text
kernel
filesystem
device
network
service
capability
user
external-source
```

The concrete adapter/runtime is provenance, not the semantic event identity.

### Subject references

Events should identify semantic subjects where possible:

```text
WorkspaceId
TaskId
KnowledgeObjectId
NodeId
CapabilityId
system component / device resource
```

An adapter may initially know only a platform-specific subject; normalization/resolution can enrich it later without mutating the original evidence.

## 3. Event payloads

The runtime may start with a generic finite attribute map, but event kinds should progressively acquire typed payload schemas.

Examples:

```text
source.modified
  object/path identity
  revision/fingerprint when available

process.completed
  Task/Binding reference
  exit/result summary

battery.telemetry
  node
  charge/discharge metric
  power state

capability.failed
  BindingLease
  implementation
  failure classification
```

Raw unstructured logs are evidence attachments, not substitutes for normalized event fields.

## 4. Provenance and trust

An event records where the claim came from.

Possible trust classes:

```text
local-kernel
trusted-local-service
trusted-fabric-node
signed-upstream-metadata
untrusted-external
user-declared
AI-inferred
```

Trust affects how events may contribute to a Situation or privileged proposal, but it does not convert evidence into authority.

## 5. Correlation

Correlation combines events over a bounded temporal/context window using deterministic rules where practical.

Examples:

```text
source.modified + active Development Workspace
    -> candidate source-change situation

repeated gpu.reset + same workload + same driver generation
    -> candidate gpu-instability situation

battery discharge increase + system revision boundary
    -> candidate battery-regression situation
```

Correlation state must be bounded by window/retention rules and reconstructible enough for debugging where practical.

## 6. Candidate Situation

A Candidate Situation is a deterministic correlation result before semantic interpretation.

It contains:

- candidate kind;
- evidence Event IDs;
- time/window bounds;
- deterministic derived facts;
- rule/version that produced it;
- unresolved semantic questions if any.

A Candidate Situation can be sufficient for simple deterministic automation and does not require an LLM.

## 7. Structured Situation

A Situation is a semantic statement about what appears to be happening.

Conceptual fields:

```text
Situation
  id
  kind
  summary
  evidence_event_ids
  derived_facts
  window
  confidence when interpretation is probabilistic
  deterministic_rule_provenance
  semantic_model_provenance when used
  subject_refs
  freshness/expiry
```

Examples:

```text
"relevant source changed and tests should be reconsidered"
"battery consumption regressed after system generation X"
"user is leaving while Task T is still running"
```

## 8. AI interpretation

AI may:

- classify ambiguous candidate situations;
- summarize evidence;
- connect current evidence with semantic context;
- suggest missing diagnostic observations;
- propose a structured Situation schema instance.

AI may not:

- rewrite raw evidence;
- manufacture event provenance;
- grant execution authority;
- bypass deterministic validation/policy.

If AI interpretation is used, model/provider/projection provenance should be recordable.

## 9. Situation consumers

Situations are published to specialized consumers.

### Task/Planner

May create/reconsider a Task or RequirementGraph.

### System Technician

May diagnose and create an ImprovementProposal.

### Workspace/Surface

May present contextual UI state.

### Policy/rule engine

May perform narrowly pre-authorized deterministic reactions.

Alfred itself should not become the owner of all these downstream responsibilities.

## 10. Distributed delivery semantics

Future Fabric event transport should target:

- idempotent processing;
- duplicate detection;
- explicit source sequence where available;
- toleration of delayed/out-of-order events;
- bounded replay;
- provenance-preserving forwarding;
- local operation when remote nodes are unreachable.

Exactly-once distributed delivery is not required as a primitive if semantic processing is idempotent.

## 11. Retention

Not every raw event deserves permanent retention.

Suggested tiers:

```text
telemetry/noise      -> bounded/aggregated
normalized evidence  -> task/situation retention policy
important Situation  -> persistent/context memory where useful
causal evidence      -> Temporal/Causal Graph reference
```

Sensitive event payloads should be minimized and may be represented by hashes/Projections/references rather than copied into long-lived history.

## 12. MVP-0 event set

MVP-0 needs only a narrow set:

```text
source.modified
task.test-requested
binding.created
capability.started
capability.completed
capability.failed
task.result-updated
```

The first Alfred rule can be:

```text
source.modified
+ source belongs to active Development Workspace
+ test policy says relevant changes should be tested

=> Candidate/Structured Situation:
   development.source-change-requires-test
```

No AI is required for this first Situation.

## 13. MVP-0 invariant

The same normalized Event stream should produce the same deterministic candidate Situation sequence for the same correlation-rule version.

Semantic AI interpretation, when later introduced, is an explicit additional step with provenance rather than hidden inside event normalization.
