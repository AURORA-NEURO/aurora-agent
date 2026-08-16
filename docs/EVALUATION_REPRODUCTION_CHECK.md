# Evaluation reproduction check contract

`evaluation_reproduction_check` certifies a serialized `bioevalx::Reexecution` under
`bioprism-mcp/evaluation-reproduction-check/0.1`. It is a comparison receipt, not an executor and
not a biological validity assessor.

The certificate preserves the declared workflow, environment-pinning posture, and every output
verdict in declaration order. Each verdict is one of:

- `matched`: the exact or numeric comparison satisfied its declared rule;
- `diverged`: the observed output was present but failed its exact, numeric, or kind comparison;
- `missing`: the output was declared but the rerun did not provide it.

The top-level `verdicts` rows repeat the certificate pairs with their output id attached. The
`verdict_count`, `matched_count`, `diverged_count`, and `missing_count` fields must reconcile with
those rows. `first_divergence` is the earliest non-matching row, not an aggregate match rate, and
`missing_outputs` is a separate projection of missing verdicts. These distinctions prevent a
rerun from improving an apparent match rate by omitting a required artifact or from hiding the
first actionable failure inside a summary statistic.

`portability_demonstrated` remains conservative: it is true only when every output reproduced and
the certificate records an unpinned environment. The tool never reconstructs an environment,
runs a workflow, regenerates a figure, or recomputes a statistical estimand. If
`biological_claim` is supplied, `validity_claim` is an explicit fail-closed refusal; matching a
pipeline cannot be promoted into biological validity.

The Python SDK exposes `EvaluationReproductionCertificateProjection`, ordered
`EvaluationReproductionVerdictProjection` rows,
`EvaluationReproductionFirstDivergenceProjection`, and
`EvaluationValidityClaimProjection`. `EvaluationReproductionReport` rejects forged counts,
missing-output lists, first-divergence positions, certificate/top-level verdict mismatches, and
validity responses that are not fail-closed. TypeScript exposes the same schema, verdict union,
certificate pairs, summary counts, divergence shape, and validity refusal shape.
