# Benchmark compiler pipeline

`benchmark_compile` exposes the assembled `bioprism-benchcompiler` pipeline at the MCP boundary.
It is the non-executing path from an observed failing trajectory to an unreviewed benchmark oracle
proposal:

```text
failing Trace + optional reference
          ↓
causal divergence and decision boundaries
          ↓
caller-observed hierarchical minimization
          ↓
ProposedOracle synthesis
          ↓
decomposed confidence and provenance
```

The output stops before human review. `benchmark_oracle_review` is a separate gate and is the only
transport path that can turn the proposal into a reviewed oracle, grade an observed verdict, or
package a reviewed `DecisionCell`.

For callers that want the complete authoring transaction, `benchmark_compile_review` accepts the
same compiler inputs plus `reviewer`, world/query `InputRef` values, and an optional grade. It
delegates to the assembled compiler and then to `Compilation::approve`; it cannot bypass the
proposal-to-reviewed-oracle type gate. Compiler refusals remain at `stage: benchmark_compile`,
while missing review, weak/exploited oracle, or absent causal cell refusals are reported at
`stage: oracle_review`.

## Inputs

`trace` is a serialized `bioprism_trace::Trace`; `reference` is an optional comparison trace. The
server runs the real causal-analysis, boundary, attribution, minimization, and synthesis kernels,
but does not replay either trace or infer facts from a natural-language transcript.

`context` is a bounded list of serialized `ContextItem` values. When it is non-empty,
`probe_observations` is an explicit table of rows shaped like:

```json
{
  "kept": ["panel_manifest", "evidence"],
  "signature": {
    "verdict": "invalid",
    "witnesses": ["identity_leakage"],
    "divergence_step": 3
  }
}
```

The server converts each `kept` list to an order-independent subset key and supplies the matching
`InterestSignature` to the kernel. It never executes a caller callback, invents a signature, or
interpolates an unobserved subset. If deterministic minimization requests a subset absent from the
table, the response is `ok: false`, `stage: minimization_probe`, and `fail_closed: true` with the
missing-subset count and a bounded sample of keys. This remains true even if the kernel would
otherwise have returned a partial compilation.

`budget.max_evaluations` is bounded to 1–100,000. The input also accepts an optional constraint
ledger and assertion claims. The ledger preserves task-defect precedence; uncited assertions stay
hypotheses inside the failure card rather than becoming backed findings.

## Returned layers

Success uses schema `bioprism-mcp/benchmark-compile/0.1` and returns:

- `compilation`: the full serialized kernel result, including analysis, boundaries, failure card,
  minimization, oracle proposal, output class, confidence, and provenance;
- `cell_step`, `episodes`, and `boundary_count`: compact navigation fields;
- `minimization`: preserved signature, removed and pinned items, evaluation count, fixpoint passes,
  reduction ratio, minimality-witness count, and the kernel guarantee;
- `oracle`: an unreviewed `ProposedOracle`, when causal localization and minimization support one;
- `confidence`, `limiting_stage`, and `unmeasured_stages`: decomposed stage evidence. An unmeasured
  stage is not converted to zero and the stages are not averaged into one headline score; and
- `probe`: table coverage and evaluation count, explicitly marked as caller-supplied observations
  rather than execution.

Compilation failures are structured and fail closed. An environment divergence, no decision,
nondeterministic probe, budget exhaustion, property loss, or incomplete table is not relocated to a
nearby action and is not returned as an apparently useful benchmark cell.

## Limits and SDKs

The request and traces are bounded at 20 MB and 100,000 events; context, observation, ledger, and
claim rows have independent bounds. The endpoint does not execute worlds or architectures, generate
mutations, run exploit attacks, validate realism, calibrate a panel, or publish a pack. Those are
separate compiler and evaluation responsibilities.

Python exposes `BenchmarkCompileArgs`, `BenchmarkCompileReport`, and
`benchmark_compile_report(...)` through sync/async MCP, HTTP, and workspace facades. TypeScript
exposes `BenchmarkCompileArgs`, `BenchmarkCompileResult`, and
`client.benchmarkCompile(...)`. The end-to-end surface additionally exposes
`BenchmarkCompileReviewArgs`, `BenchmarkCompileReviewReport`, and
`client.benchmarkCompileReview(...)` / `benchmark_compile_review_report(...)`.
