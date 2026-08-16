# Modality transport and loss audits

`modality_transport_check` exposes the declared transport kernel behind the modality support
relation. It audits aggregation, deconvolution, and imputation without pretending that a
descriptor transition moved data or validated a model.

## Request

```json
{
  "from": "single_cell",
  "to": "bulk_transcriptomics",
  "axis": "cell",
  "transport": {"kind": "aggregation", "operator": "mean"},
  "claims": ["cell_intrinsic_change", "cell_composition"]
}
```

The three transport kinds are:

- `aggregation`: removes a resolved axis and records the discarded distribution. It is exact but
  not invertible;
- `deconvolution`: creates an axis against a named reference and records uncertainty and the
  reference condition. It is estimated and its structural inverse is an aggregation, not proof
  that the estimated components were correct;
- `imputation`: fills entries using a named model and records uncertainty plus the loss of an
  observation mask. It is not invertible without that mask.

Every operation needs a real source descriptor. The catalogue descriptor is used by default, or a
study-specific `source_descriptor` can be supplied when its acquisition has explicitly declared
different resolution. Construction refuses an aggregation over an unresolved axis, a
deconvolution without a reference, a deconvolution that would relabel a measured axis as an
estimate, and imputation without a model.

## Response contract

The schema is `bioprism-mcp/modality-transport-check/0.1`:

- `loss` is the transport's serialized `LossLedger` with discarded information, added
  uncertainty, and policy conditions;
- `fidelity` distinguishes exact aggregation from estimated deconvolution/imputation;
- `scope_mapping` renders the move in the shared `bioprism-scope` mapping vocabulary and
  `scope_mapping_check` reports its soundness;
- `inverse` keeps invertibility and a typed refusal separate from fidelity;
- `application` and `applied_descriptor` show the post-transport resolution state;
- optional `claims` rows compare support before and after transport, including support lost or
  gained, without inferring that a numeric result changed.

The report makes a critical asymmetry inspectable: aggregating a single-cell descriptor removes
cell-level claims, while deconvolving a bulk descriptor can admit composition as an imputed-axis
claim but still refuses a cell-intrinsic claim because that would use the reference as evidence
for the cell structure it supplied.

## SDK usage and limits

Python exposes `ModalityTransportCheckArgs`, `ModalityTransportCheckReport`, and
`modality_transport_check_report(...)` through workspace and HTTP sync/async facades. TypeScript
exposes `ModalityTransportCheckArgs`, `ModalityTransportCheckResult`, and
`ApiClient.modalityTransportCheck(...)`.

This is a declaration audit. It does not move values, compute an aggregation, fit a deconvolution,
validate a signature matrix, restore discarded information, or establish cross-modality biological
equivalence. A successful inverse is a structural constructor result, never evidence that an
estimated transport recovered the original measurement.
