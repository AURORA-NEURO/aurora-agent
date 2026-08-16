# Bioevaluation scoring-plane audit

`bioeval_plane_audit` exposes the `bioprism-bioevalx` scoring plane (26.17) as a bounded,
reviewable MCP projection. The plane is where a multidimensional evaluation records the
difference between a measured score, a dimension that was not measured, and a dimension that the
system could not meaningfully be asked to perform.

The route invokes the real `ScorePlane` deserializer and `FoldPolicy::ExcludeInapplicable` fold.
It never fills an absent score, reweights dimensions by preference, ranks systems, or compares
folds with different included bases. A fold is descriptive aggregation over the declared
measured dimensions; it is not biological truth, causal effect, clinical validity, or release
approval.

## Request

```json
{
  "plane": {
    "system": "fixed-model",
    "tier": "fixed_input_model",
    "dimensions": [
      { "id": "accuracy", "required": "fixed_input_model", "weight": 2.0 },
      { "id": "assay-selection", "required": "tool_using_agent", "weight": 1.0 },
      { "id": "calibration", "required": "fixed_input_model", "weight": 1.0 }
    ],
    "cells": {
      "accuracy": { "state": "scored", "score": 0.8 },
      "assay-selection": {
        "state": "inapplicable",
        "required": "tool_using_agent",
        "declared": "fixed_input_model"
      },
      "calibration": {
        "state": "unscored",
        "reason": "no_reference_standard",
        "note": "reference panel pending"
      }
    }
  },
  "max_items": 100,
  "require_fold": false
}
```

The five capability tiers are `fixed_input_model`, `workflow_pipeline`, `tool_using_agent`,
`human_in_the_loop`, and `multi_agent_molecule`. A dimension's `required` tier declares the
narrowest system that can be asked it. A cell is `inapplicable` only when the declared system tier
does not admit that requirement. Dimensions and cells must match exactly, identifiers are bounded
at 256 bytes, dimensions at 4096 rows, and the encoded request at 20,000,000 bytes.

## Cell states are not interchangeable

`scored` carries a measured finite value in `[0, 1]`. The route does not infer where the number
came from; it verifies that the serialized plane can carry it and then asks the real plane to fold.

`unscored` carries a reason and no score. Supported reasons are:

- `not_attempted` — the dimension was declared but the evaluation did not run;
- `evaluator_unhealthy` with `evaluator` — the evaluator could not produce task evidence;
- `no_reference_standard` with `note` — the reference needed to interpret the measurement is
  unavailable or disputed; and
- `sealed` with `registration` — the result is withheld under prospective reveal control.

`inapplicable` carries both `required` and `declared` tiers. It is outside the fold denominator
under the named `ExcludeInapplicable` policy. It is not a zero and does not lower a fixed-input
model's average for an action it was never designed to take.

The route checks that scored/unscored cells are not attached to out-of-tier dimensions and that
inapplicable metadata agrees with the dimension and plane. This catches a serialized plane that
would otherwise present a capability mismatch as a valid number.

## Successful projection

Schema is `bioprism-mcp/bioeval-plane-audit/0.1`.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/bioeval-plane-audit/0.1",
  "workflow": "bioeval_plane_audit",
  "plane": {
    "system": "fixed-model",
    "tier": "fixed_input_model",
    "dimension_count": 3,
    "scored_count": 1,
    "unscored_count": 1,
    "inapplicable_count": 1
  },
  "dimensions": {
    "rows": ["..."],
    "returned": 2,
    "total": 3,
    "omitted": 1
  },
  "findings": {
    "unscored_dimensions": { "ids": ["calibration"], "total": 1, "omitted": 0 },
    "inapplicable_dimensions": { "ids": ["assay-selection"], "total": 1, "omitted": 0 },
    "fold_blocked": true,
    "fold_refusal": "..."
  },
  "fold": {
    "folded": false,
    "policy": "exclude_inapplicable",
    "value": null,
    "included": [],
    "excluded": [],
    "refusal": "..."
  },
  "guarantees": ["..."],
  "limitations": ["..."]
}
```

Every returned dimension row includes its identifier, required tier, weight, serialized cell,
whether it was measured, and whether it blocks a fold. Rows and identifier findings retain
`total` and `omitted` counts so truncation cannot look like absence.

When every applicable dimension is scored, the fold includes only those scored dimensions and
lists every inapplicable dimension in `fold.excluded`. Its value is the weighted mean over the
included dimensions. For example, scores `0.75` with weight 2 and `0.90` with weight 1 produce a
fold of approximately `0.80`, while an inapplicable third dimension contributes neither a zero
nor an imputed value.

## Fold policy and refusals

The route always evaluates the real `ExcludeInapplicable` policy. If an applicable dimension is
unscored, `ok: true` still returns the cell audit, but `fold.folded` is false and the exact kernel
refusal remains visible. This is the review-friendly default.

`require_fold: true` converts that unresolved fold into `ok: false`, `stage: "fold_policy"`, and
`fail_closed: true`. This lets a release or leaderboard gate demand a complete measured basis
without losing the underlying omission finding.

Malformed serialized planes, duplicate dimensions, unknown cells, invalid weights, out-of-range
scores, inconsistent tier metadata, or unbounded requests return structured fail-closed
refusals. No fallback fold is emitted after a semantic validation failure.

## Composition and boundaries

The plane does not aggregate across systems. This is deliberate: two systems may have different
included dimensions, and a shared `[0, 1]` scale does not make those folds the same measurement.
Cross-system comparison needs a separate matched-basis contract.

The plane composes with the other evaluation cells:

- `bioeval_evaluator_audit` can explain why an applicable dimension remains unscored;
- `bioeval_reference_audit` and `bioeval_grounding_audit` can establish whether reference and
  claim evidence are available without converting them into a score;
- `bioeval_estimand_audit` constrains the meaning and scope of a result; and
- `bioeval_acquisition_audit` shows whether required information-seeking obligations were closed
  before a dimension was scored.

The route audits caller-supplied score records. It does not measure accuracy, run an assay,
validate a reference, infer capability from prose, impute missingness, or grant operational or
clinical authority.

## SDK surfaces

- Python exposes `BioevalPlaneDimensionArgs`, `BioevalPlaneCellArgs`, `BioevalScorePlaneArgs`,
  `BioevalPlaneAuditArgs`, `BioevalPlaneAuditReport`, and `bioeval_plane_audit_report(...)`
  through `Workspace`, `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`.
- TypeScript exposes typed tier, dimension, discriminated-cell, score-plane, audit-argument, and
  audit-result interfaces plus `bioevalPlaneAudit(...)`.
