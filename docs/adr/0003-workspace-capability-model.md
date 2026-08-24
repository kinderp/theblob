# ADR-0003: Workspace is persistent; capabilities are composable and usually ephemeral

**Status:** provisional

## Decision
Do not replace “applications” with long-lived containers. Replace the monolithic application model with:

- persistent **Workspace** experience/state;
- versioned **Workspace Recipe**;
- abstract **Capabilities**;
- concrete **Capability Capsules** materialized through the appropriate runtime.

Workspace creation has three first-class modes: **Ready**, **AI Designed**, and **Expert**.

User data/state belongs to the Personal World, not to capability implementations.
