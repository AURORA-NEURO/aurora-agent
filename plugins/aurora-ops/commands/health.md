---
description: One-pass operational evidence check of the aurora-agent control plane (snapshot, gates, recovery) with honest posture reporting
argument-hint: "[gateway base URL, default http://127.0.0.1:8787] (token via AURORA_GATEWAY_TOKEN or ask)"
---

Run a one-pass operations health check against the aurora-agent gateway.

1. Gateway base: `$ARGUMENTS` or `http://127.0.0.1:8787`. Bearer token: the
   `AURORA_GATEWAY_TOKEN` environment variable, or ask the operator. If the
   gateway is not running, say so and offer the launch line from
   docs/HTTP_API.md instead of starting services unprompted.
2. Fetch, with curl: `/v1/operations/snapshot?after=0&limit=15`,
   `/v1/operations/gates?after=0&limit=40`, `/v1/recovery`.
3. Report: mission totals and event metrics; per-group gate states (expect
   `insufficient_evidence` until evaluator evidence exists — that is a
   posture, not a defect); `readiness_claimed` verbatim wherever it appears;
   persistence checkpoint presence and integrity; `automatic_resume` and the
   `does_not_restore` lists; refused-event counts; and the system's own
   `guarantees` / `non_claims` / `operator_actions` arrays verbatim.
4. Never summarize the posture as "healthy" or "ready" — summarize it as what
   the evidence shows, name what was not inspected (bounded event pages), and
   surface the gate policy's decision rule if dispatch questions come up.
