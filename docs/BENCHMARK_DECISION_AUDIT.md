# Benchmark decision audit

`benchmark_decision_audit` is the review-facing bridge between the native trajectory IR and a
candidate benchmark decision cell. It composes the existing `bioprism-benchcompiler` kernels; it
does not invent a second action model or silently upgrade a proposal into an executable test.

## Request

The required `trace` is a serialized `bioprism-trace::Trace`. It contains a producer-supplied
`trace_id`, typed events, opaque payloads, optional `caused_by` links, visible-variable names, and a
producer-supplied `succeeded` flag. An optional `reference` trace supplies observed comparison
evidence for causal ranking. It is not replayed and it is not treated as a measured intervention.

`decision_step` is optional. Without it, the endpoint selects the `first_causal_step` only when
causal analysis is willing to localize a decision. With it, the endpoint checks that the requested
step exists and is a `choice` or `action`; an observation, result, missing step, or other
environment event is refused rather than replaced with a nearby action.

The optional `actions` array accepts serialized `CandidateAction` values. Each action records a
label, optional semantic property, provenance, feasibility, and `strong` review flag. The
provenance values are the hindsight firewall:

- `recorded_alternative` is an alternative present at the recorded decision;
- `visible_at_decision_time` and `peer_trajectory` must cite a step at or before the decision;
- `tool_schema` and `architecture_policy` are declared sources;
- `from_future` remains useful for validating coverage but is never returned as agent-visible;
- a future step falsely labelled as visible is a fail-closed refusal.

The endpoint always starts with `CandidateActionSet::reconstruct`, which only imports the action or
tool and alternatives recorded on the selected event. It then applies caller-supplied actions
through `CandidateActionSet::add`, so provenance checks happen in the domain kernel rather than in
SDK glue.

## Returned layers

The response has schema `bioprism-mcp/benchmark-decision-audit/0.1` and keeps four layers apart.

1. `analysis` is the causal result: textual divergence, ancestry, ranked decision-bearing
   candidates, transparent necessity/counterfactual/irreversibility/simplicity components, and a
   typed verdict. `analysis_omitted` reports truncation.
2. `decision` identifies the selected step and whether it aligns with the causal step. It returns
   the reconstructed action rows plus separate `visible_to_agent`, `validation_only`, and
   `acceptable` projections. `coverage` reports total, visible, validation-only, feasible, strong,
   plausible-wrong-alternative, and adequacy counts. `action_counts` and `omitted` make bounded
   transport loss explicit.
3. `failure_card` applies the attribution ordering from section 06.06. An unsatisfiable constraint
   routes to `task_defect`; an evaluator dispute routes to `evaluator`; otherwise the causal
   verdict may produce agent, environment, or undetermined blame. `claims` with no citations stay
   in `hypotheses`, while cited claims reach `findings`. `evidence_ratio` is a derived projection,
   not a confidence estimate.
4. `trace_digest` and optional `reference_digest` bind the review material to the exact input
   documents. They are identity witnesses, not provenance for an external dataset.

`max_items` bounds each returned list independently (1–1000, default 100). The trace and reference
are bounded at 100,000 events, candidate actions at 10,000, constraint records at 10,000, and
assertions at 10,000; the combined request is bounded at 20 MB.

## Refusal semantics

Refusals are structured with `ok: false`, a `stage`, `refusal`, `fail_closed: true`, and guarantees.
Important refusals include:

- `benchmark_causal_analysis`: empty or non-decision-bearing trajectory;
- `decision_selection`: no localized decision and no explicit valid step;
- `decision_reconstruction`: selected step is not an actual decision-bearing event;
- `hindsight_firewall`: a candidate claims decision-time visibility from a later step.

An environment divergence is never relocated to the nearest controlled ancestor. A causal ranking
without a reference remains structurally informative but is not promoted to an agent blame
assignment. The endpoint does not replay tools, fork an architecture, minimize state, synthesize or
approve an oracle, grade an agent, or publish a benchmark pack.

## SDKs

Python exposes `BenchmarkDecisionAuditArgs`, `BenchmarkDecisionAuditReport`,
`BenchmarkDecisionCoverageReport`, `BenchmarkFailureCardReport`, and
`benchmark_decision_audit_report(...)` through sync MCP, async MCP, sync HTTP, async HTTP, and both
workspace facades. TypeScript exposes `BenchmarkDecisionAuditArgs`,
`BenchmarkDecisionAuditResult`, and `client.benchmarkDecisionAudit(...)`.
