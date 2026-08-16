# Benchmark counterfactual check

`benchmark_counterfactual_check` validates a matched pair of serialized `DecisionCell` values and
grades a candidate's response to one declared intervention. It is the public transport for the
benchmark compiler's 06.09 `pair` and `contrast` kernels.

## Matched-pair contract

The request supplies `source`, `followup`, `intervention`, `expected`, `source_verdict`, and
`followup_verdict`.

`Intervention` names a factor, its typed target, `from` and `to` values, and a set of cell fields it
is allowed to change. The compiler compares exactly these `DecisionCell` fields:

`world`, `query`, `acceptable_verdicts`, `required_witnesses`, and
`require_protected_closure`.

The pair is refused when cell ids collide, the intervention is null, the cells do not differ, the
caller-side realism check would refuse the state, or a field moved without appearing in
`intervention.changes`. The MCP boundary uses `NoRealismReview` explicitly and returns
`realism_reviewed: false`; it does not turn the absence of an environment/domain validator into a
realism claim.

The pair preserves set-valued acceptance contracts. Two cells can accept multiple verdicts and
require multiple witnesses; the counterfactual checker compares the declared verdict response and
does not reduce a cell to one “correct answer.”

## Contrast outcomes

`ExpectedResponse` is either:

- `invariant`: the correct verdict should stay the same;
- `must_change`: the verdict must move and land in the declared `to_verdicts` set.

`outcome` is one of `as_predicted`, `spurious_sensitivity`, `missed_the_change`, or
`wrong_direction`. `satisfied` is a convenience derived directly from that typed outcome, not a
model score. Source/follow-up cell digests bind the result to the exact pair.

## Limits and SDKs

The response schema is `bioprism-mcp/benchmark-counterfactual/0.1`. Refused pairs return
`ok: false`, `stage: matched_pair`, a refusal, `fail_closed: true`, and the allowed field list.
The endpoint does not apply interventions, construct follow-up worlds, run an architecture,
measure a causal effect, or approve a benchmark cell. It validates caller-constructed evidence and
records whether the candidate response agrees with the declared contrast.

Python exposes `BenchmarkCounterfactualCheckArgs`, `BenchmarkCounterfactualCheckReport`, and
`benchmark_counterfactual_check_report(...)` through sync/async MCP, HTTP, and workspace facades.
TypeScript exposes `BenchmarkCounterfactualCheckArgs`, `BenchmarkCounterfactualCheckResult`, and
`client.benchmarkCounterfactualCheck(...)`.
