# Physical Node Preflight Report v0.1

**Status:** Linux Pilot read-only UX contract.

The System Technician turns the deterministic `PhysicalTestNodeProfile + PhysicalTestNodeReadiness` result into a human-facing preflight report **without gaining execution authority**.

The report contains:

- requested `SystemCandidateAction`;
- eligible / blocked state;
- structured readiness evidence;
- exact blocking reasons mapped from core violations;
- platform-probe warnings preserved as uncertainty;
- next steps appropriate to the action;
- explicit safety notes separating readiness from authorization.

Example blocked live test:

```text
Temporary live test activation: BLOCKED

Why:
- external power has not been confirmed;
- no rollback reference is recorded.

What to do:
- connect/confirm external power;
- establish a current boot-generation rollback reference;
- collect a fresh readiness observation.

The Technician cannot bypass this gate.
```

Example successful materialization preflight:

```text
Materialize candidate: READY

The node is trusted, matches the profile and has enough healthy storage.
Power/console/boot rollback are not required because materialization does not change the live system.
```

A successful `PreviewActivation` or `TestActivation` preflight means only that the machine satisfies safety prerequisites. Host-administrator authorization remains separately required. Persistent `boot`/`switch` activation remains outside the v0.1 model.
