# Modality support and analysis-unit checks

`modality_support_check` turns the modalities crate's support relation into a reusable MCP and
SDK boundary for all 17 assay families. It answers two independent questions:

1. Does the selected descriptor have the measurand, resolution, imputation policy, and evidence
   design needed for the requested `ClaimKind`?
2. If the caller counted observations at a declared analysis unit, does that unit match the
   descriptor's declared independent unit, or is the result pseudoreplication?

The first answer is a modality-eligibility decision. The second is an analysis-design decision.
A modality can support a claim and the analysis can still be inadmissible; conversely, a sound
analysis unit cannot make an assay support a claim about an axis it does not resolve.

## Request

```json
{
  "modality": "bulk_transcriptomics",
  "claim": "cell_intrinsic_change",
  "counted_unit": "population"
}
```

Omitting `descriptor` uses the catalogue's general-case contract. A study-specific descriptor may
be supplied when the actual acquisition resolves more axes than the general modality, for example
a longitudinal metabolomics study or an explicitly transported value. Its `modality` must match
the request; the server never silently merges two descriptors or changes their modality identity.

The `claim` vocabulary is intentionally finite: it includes population average, cell identity,
cell composition, cell-intrinsic change, spatial localisation, communication, protein activity,
flux, dependency, causal perturbation, binding, exposure, host mechanism, subject outcome,
treatment effect, temporal order, cross-species equivalence, published-claim support, and dataset
content. Requirements are returned in the response so a refusal can be repaired at the right
boundary rather than by guessing.

## Response

The schema is `bioprism-mcp/modality-support-check/0.1`.

- `outcome_kind` is `supported` or `refused` for the modality/claim relation;
- `support` carries the serialized refusal, its kind, the root refusal kind, and text;
- `analysis_unit` is independently `requested`, with `admissible` set to `true`, `false`, or
  `null` when no unit audit was requested;
- `descriptor` retains measurand, design, all seven resolution states, caller-supplied constants,
  failure modes, and the claims supported by the selected catalogue descriptor;
- `claim_requirements` shows the axes, measurand requirement, evidence design, and imputation
  policy that drove the decision.

Refusal layers remain visible. For example, a bulk transcriptomics descriptor used for a
cell-intrinsic claim may have `support.refusal_kind = "named_failure_mode"` because the blueprint
names the composition failure mode, while `support.root_refusal_kind = "missing_resolution"`
identifies the mechanical reason. A separate `analysis_unit.refusal_kind` can report
`pseudo_replication` for a counted-unit mismatch.

## Guarantees and limits

The relation checks structural entitlement, not biological truth, power, effect size, FDR,
confidence intervals, or clinical validity. The catalogue's unmechanised failure modes remain
visible but are not claimed to be detected. A successful response therefore means “this descriptor
is the right kind of instrument for this kind of statement,” not “the statement is true.”

Python exposes `ModalitySupportCheckArgs`, `ModalitySupportCheckReport`, and
`modality_support_check_report(...)` through workspace and HTTP sync/async facades. TypeScript
exposes `ModalitySupportCheckArgs`, `ModalitySupportCheckResult`, and
`ApiClient.modalitySupportCheck(...)`.
