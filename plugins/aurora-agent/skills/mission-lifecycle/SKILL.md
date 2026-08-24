---
name: mission-lifecycle
description: Author, preflight, and execute aurora-agent missions — validated DAGs of MCP tool calls. Use when asked to build or run a mission, when a mission is refused with invalid_mission or operations_gate_acceptance_required, when deciding between synchronous agent_mission and async submission, or when wiring evaluator reviews and claim lineage.
---

# Mission lifecycle

A mission is a **validated DAG of MCP tool calls** executed in-process with an
explicit least-authority policy. Refusals are preserved verbatim; a structural
success is never presented as a scientific one.

## Mission shape (the fields that trip people)

```json
{
  "mission_id": "my-mission-1",
  "goal": "compile the reference world",
  "steps": [{
    "id": "compile",
    "domain": "fiber",
    "capability": "context_compile",
    "objective": "compile at L0",
    "tool": "fiber_compile",
    "arguments": { "world": "fixtures/fiber-v0.1/radiogenomic_world.json",
                    "query": "fixtures/fiber-v0.1/leakage_query.json" },
    "depends_on": []
  }],
  "policy": { "execute": true, "allowed_tools": ["fiber_compile"],
               "allow_side_effects": false, "max_steps": 4 }
}
```

- **Every step requires `domain` AND `capability`** (labels retained for
  routing and audit; the server does not infer semantics). Missing either →
  `invalid_mission`.
- `policy.allowed_tools` is an allow-list; a step naming a tool outside it is
  refused: "agent mission refused: step X requests tool Y, which is not
  allow-listed".
- Step `bindings` are validated JSON pointers pulling upstream results into
  dependent arguments; `agent_mission` cannot recurse into itself.

## Three dispatch paths, one kernel

1. **Preflight** (`POST /v1/missions/preflight` on the gateway): validates and
   plans; returns `preflight: true`, **`dispatch: "not_started"`**, the
   digest-bound wave plan, and an `operations_evidence` block. Nothing
   executes. Report `dispatch` and the gate `decision` verbatim.
2. **Synchronous** (`agent_mission` MCP tool): executes immediately, returns
   the full report — `mission_status`, per-step `results[]` (raw JSON-RPC
   envelopes preserved, including refusals), a clock-free `execution_trace`,
   and `claim_lineage` (`claim_status: "unreviewed"`, `readiness_claimed:
   false` until evaluator evidence exists — report these verbatim).
3. **Async** (`POST /v1/missions`): queued with retry/lease semantics — but
   **fail-closed**: without a current operator gate acceptance covering every
   selected domain evidence gate it refuses with
   `operations_gate_acceptance_required`. That is working as designed, not a
   bug. The acceptance is an operator decision recorded via
   `POST /v1/operations/gate-reviews` with
   `{gate_digest, reviewer, rationale, group_ids, accepted_gates}` — it does
   not make the evidence better, and preflight's
   `dispatch_prerequisite: "acceptance_required"` names the gap.

## Honest-reporting rules

- `readiness_claimed: false` appears at many levels; never render a group as
  "ready" while it is false.
- Nested refusals surface as `refused` counts plus preserved envelopes in
  `results[].wire` — show the inner error text verbatim.
- The artifact registry entry for a mission report carries
  `semantic_verification: "not_run"` — content-digest verification is not
  semantic validation; keep the distinction when summarizing.
