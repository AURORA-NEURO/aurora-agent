# Benchmark oracle review

`benchmark_oracle_review` is the transport boundary for the benchmark compiler's oracle gate.
It makes the most important type distinction in benchmark authoring visible to callers:
`ProposedOracle` is a synthesis artifact, while `ReviewedOracle` is the only artifact allowed to
grade a response or package a `DecisionCell`.

## Why the boundary is typed

An oracle is not merely a label such as “exact” or “model judge.” It is a contract describing the
decision point, accepted verdict vocabulary, required witnesses, observable inputs, blind spots,
and attacks attempted against the grader. If that contract can be serialized and reintroduced as
trusted review state, a caller can claim that a model-generated grader was reviewed without any
human gate. The Rust kernel therefore serializes reviewed output for audit but does not deserialize
it back into a `ReviewedOracle`.

The endpoint accepts a serialized proposal and a named `reviewer`. It calls the kernel's
`ProposedOracle::review` path and returns either a reviewed projection or a structured,
fail-closed refusal. Review refuses when:

- the reviewer is missing or blank;
- the acceptable verdict set is empty;
- the proposal names no blind spot or gap in what it can observe;
- an exploit was scored as a pass without fulfilling task intent; or
- a statistical tolerance or model judge has no deterministic companion.

The endpoint does not allow a caller to submit a serialized `ReviewedOracle` in place of this
gate. The returned `review_digest` binds the proposal and reviewer, and `synthesis_order` keeps
the compiler's strongest-first ladder visible: exact state predicate, execution test, property
relation, trajectory constraint, statistical tolerance, and model judge.

## Optional grading

The optional `grade` object contains an observed `verdict`, a set/list of `witnesses`, and a
boolean `closure_complete`. Grading is set-valued and preserves four distinct outcomes:

- `passed`: the verdict is acceptable, required witnesses are present, and protected closure is
  complete;
- `wrong_verdict`: the observed verdict is outside the proposal's accepted set;
- `missing_witnesses`: the verdict is acceptable but required evidence is absent; and
- `closure_incomplete`: the answer may be right while the protected evidence basis is incomplete.

`passed` is only a convenience projection of the typed acceptance outcome. It is never inferred
from the verdict string alone.

## Optional DecisionCell packaging

The optional `cell` object supplies a `cell_id`, a world `InputRef`, and a query `InputRef`.
Packaging is only reached after review succeeds. The kernel copies the reviewed oracle's decision
point, acceptable verdict set, and required witness set into a `DecisionCell`; it does not invent a
single canonical answer or discard the set-valued contract.

Packaging still does not execute the cell, validate a domain world, run an exploit suite, or prove
that the declared oracle is scientifically adequate. Those are separate execution, realism, and
calibration responsibilities. The response explicitly reports these limits so a successful review
cannot be mistaken for benchmark performance or external-world truth.

## Response and SDKs

The response schema is `bioprism-mcp/benchmark-oracle-review/0.1`. A refusal has `ok: false`,
`stage: oracle_review`, `refusal`, and `fail_closed: true`. A success includes the proposal,
serialized reviewed projection, reviewer identity, digest, strength/determinism, optional grade,
optional packaged cell, guarantees, and limitations.

Python exposes `BenchmarkOracleReviewArgs`, `BenchmarkOracleReviewReport`, and
`benchmark_oracle_review_report(...)` through sync/async MCP, HTTP, and workspace facades.
TypeScript exposes `BenchmarkOracleReviewArgs`, `BenchmarkOracleReviewResult`, and
`client.benchmarkOracleReview(...)`.
