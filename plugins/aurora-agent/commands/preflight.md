---
description: Preflight an aurora-agent mission — validate, plan, and show gate posture without dispatching anything
argument-hint: "[path to mission JSON, or leave empty to preflight the demo mission]"
---

Preflight a mission against the aurora-agent backend without executing it.

1. If `$ARGUMENTS` names a JSON file, read it as the mission request. Otherwise
   use the demo mission from the mission-lifecycle skill (fiber_compile on the
   reference fixtures; remember every step needs `domain` and `capability`).
2. Preferred transport: the `agent_mission` MCP tool is EXECUTION — do NOT use
   it here. Preflight goes through the HTTP gateway:
   `POST /v1/missions/preflight` on a running `bioprism-api` (launch line is
   in the aurora-agent docs/HTTP_API.md). If no gateway is running, say so and
   offer the launch command rather than executing anything.
3. Report verbatim: `dispatch` (expect `"not_started"`), the plan digest, the
   wave decomposition, `operations_evidence.decision`,
   `acceptance_required/present/valid`, `dispatch_prerequisite`, each group's
   `gate_state` and missing gates, and `readiness_claimed`. A complete
   evidence set still requires human review — never present preflight success
   as authorization to dispatch.
