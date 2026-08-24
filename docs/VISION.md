# Vision

## Thesis

The operating system should stop treating applications, files and individual devices as its primary user-facing abstractions. It should instead manage a **Personal World**: persistent identity, context, knowledge, goals, Workspaces and policy, projected onto a changing fabric of devices and capabilities.

## Paradigm shifts

1. **Device → Fabric**  
   PC, phone, watch, server, cloud and IoT nodes are resources of one Personal World.

2. **Application → Workspace + Capability**  
   The stable user experience is a Workspace; executable software is acquired as composable capabilities, often ephemerally.

3. **Command → Intent**  
   Users can express outcomes; the system plans, composes capabilities and exposes the plan when useful.

4. **Event → Situation**  
   Events are correlated temporally and semantically so the OS can understand what is happening rather than merely react to low-level signals.

5. **File → Knowledge Object**  
   Persistent information has identity, semantics, relations, provenance and history. PDF/DOCX/JPEG/etc. are representations or exported artifacts.

6. **Configuration → Evolution**  
   The system can create experimental branches of its own configuration, kernel or drivers, benchmark them, verify effects, then merge or discard them.

7. **Snapshot → Causal History**  
   State changes include why, who/what triggered them, evidence, expected effects, actual effects and rollback information.

## Principle

**Code is replaceable; user state, identity, meaning and history belong to the Personal World.**
