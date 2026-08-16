# Oncology Outcome Analysis

`onco_outcome_analyze` creates one subject-level analysis record from a serialized `FollowUp` and
a predeclared `Estimand`. It does not estimate a cohort endpoint. The successful projection is
versioned as:

```text
bioprism-mcp/onco-outcome-analyze/0.1
```

## Estimand binding

The nested `analysis` record carries the exact estimand that produced the numbers:

- `endpoint` distinguishes overall survival, progression-free survival, time to progression, and
  time to treatment failure;
- `population` retains intention-to-treat, per-protocol, or response-evaluable scope;
- `variable` preserves the plain-language target;
- `summary_measure` retains the declared measure, including landmark parameters;
- `intercurrent_event_strategies` preserves the strategy for each intercurrent event; and
- `censoring_assumption` prevents an estimator from silently treating informative censoring as
  non-informative.

The Python SDK parses these fields as `OncoEstimandProjection`. The report refuses an analysis
whose endpoint, population, strategy pairs, or estimand-linked outcome disagree with the
projection metadata.

## Event and censoring are disjoint

`outcome` is a tagged record:

```json
{ "outcome": "censored", "lost_to_follow_up": null }
```

or:

```json
{ "outcome": "event", "kind": "confirmed_progression" }
```

The top-level `event` and `censoring_reason` fields are ergonomic copies, but the typed SDK
requires them to reconcile with the tagged `outcome` and with `analysis.outcome`. A censored record
must have exactly one tagged reason; an event cannot carry one. Loss to follow-up is never promoted
into an event. The SDK normalizes the tagged key into `outcome_record.censoring_reason` while
retaining the raw serde representation.

## Delayed entry and bias evidence

`at_risk_days` starts at risk-set entry. `immortal_time_days` covers the index-to-entry interval
that the subject had to survive in order to enter observation. `left_truncated` is derived from
that interval and cannot contradict it.

`bias_flags` is the complete per-subject set. `informative_bias_flags` is the subset that signals
potentially prognosis-dependent censoring or treatment switching. `bias_count` and
`informative_bias_count` must match the arrays. The top-level `censoring_informative` value is
`null` for an event and otherwise follows the kernel’s reason-specific classification.

The available one-subject flags are `left_truncation`, `informative_loss_to_follow_up`,
`competing_death`, and `treatment_switching`. Landmark-time and follow-up-intensity bias are
cohort-level properties and are intentionally outside this record.

## Python and TypeScript surfaces

Python callers receive `OncoOutcomeReport.analysis_record` as an
`OncoAnalysisRecordProjection`, with typed `OncoEstimandProjection` and
`OncoAnalysisOutcomeProjection` children. The original `analysis` mapping remains available for
forward-compatible fields. TypeScript exposes the same contract through
`OncoOutcomeAnalysisResult`, `OncoOutcomeEstimandResult`, and the discriminated
`OncoOutcomeOutcomeResult` union.

The projection is transport-agnostic across direct MCP structured content, HTTP envelopes, and
JSON text content. It preserves the distinction between a successful domain-level censored result
and a transport failure.

## Scope and limitations

This tool produces a per-subject record for a downstream estimator. It does not fit Kaplan–Meier,
Cox, competing-risk, or cumulative-incidence models; it does not decide treatment; and it cannot
detect cohort-level landmark or follow-up-intensity bias from one record.
