# Modality-aware comparability

`modality_comparability_check` compares two serialized `ModalMeasurement` values through the
modality kernel before it delegates to `bioprism_standards`. This boundary exists because equal
units, frames, reference builds, and ontology terms do not prove that two assays measure the same
biological quantity. A transcript abundance and a protein abundance can both be dimensionless and
still be different measurands.

## Request

The request has required `left` and `right` `ModalMeasurement` objects and an optional serialized
`ComparabilityPolicy` under `policy`:

```json
{
  "left": {
    "descriptor": {"modality": "bulk_transcriptomics", "measurand": "RNA abundance"},
    "reported_at": "population",
    "measurement": {"label": "RNA abundance", "value": {"Scalar": {"value": 1.0, "unit": "1"}}}
  },
  "right": {
    "descriptor": {"modality": "proteomics", "measurand": "protein abundance"},
    "reported_at": "population",
    "measurement": {"label": "protein abundance", "value": {"Scalar": {"value": 1.0, "unit": "1"}}}
  },
  "policy": {"require_bound_terms": true}
}
```

The objects are deserialized by Rust, so the exact measurement and descriptor representation is
owned by the `bioprism_modalities` and `bioprism_standards` crates. SDKs validate that the two
measurements and policy are objects, but intentionally do not reimplement those Rust schemas.

## Ordered checks

The handler preserves the kernel's fail-closed order:

1. Compare the declared measurands. A category mismatch blocks before standards comparison.
2. Confirm that both reported resolutions describe the same biological axis.
3. Refuse undeclared or unreportable axes and distinguish imputed from measured evidence.
4. Only then delegate unit, frame, reference-build, and ontology checks to the standards kernel.

When the modality layer blocks, `report.standards` is `null`; this makes it impossible to interpret
the absence of a standards result as “standards passed.” When delegation occurs, the standards
report remains embedded in the cross-modal report, including its conversions and caveats.

## Response contract

The response schema is `bioprism-mcp/modality-comparability-check/0.1`. `outcome_kind` is either
`comparable` or `blocked` and must reconcile with the top-level `comparable` boolean. `verdict` is
the serialized tagged verdict, while `report` retains both modality-side evidence and the optional
standards report. `report_sha256` is a digest of the canonical cross-modal report, allowing a
caller to bind a review or downstream artifact to the exact evidence shown.

The typed blocked reasons include `measurand_mismatch`, `resolution_mismatch`,
`imputed_against_measured`, `undeclared_axis`, `unreportable_axis`, and standards-wrapped
refusals. The response also repeats a compact left/right summary so a caller can inspect modality,
measurand, reported axis, axis status, and measurement without having to unpack the full report.

## Non-claims

A `comparable` result means only that the declared categories passed the modality and standards
compatibility gates. It does not assert equality of values, calibration, statistical power,
measurement correctness, causal equivalence, predictive validity, or biological agreement. The
handler does not move values or perform aggregation, deconvolution, imputation, or statistical
testing. Resolution-changing operations must first be represented by `modality_transport_check`
with an explicit loss/fidelity ledger.

The Python SDK exposes `ModalityComparabilityCheckArgs`,
`ModalityComparabilityCheckReport`, and `modality_comparability_check_report`; the TypeScript SDK
exposes `ModalityComparabilityCheckArgs`, `ModalityComparabilityCheckResult`, and
`modalityComparabilityCheck`.
