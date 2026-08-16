# Inference-lab Pareto audit

`lab_pareto_audit` exposes the multi-objective archive in `bioprism-lab::pareto`. It is the
architecture-search boundary for comparing candidate declarations when there is no honest way to
turn utility, cost, latency, safety, or coverage into one hidden scalar. The endpoint preserves
the archive's negative evidence: dominated candidates, displacement, trade-offs, unmeasured axes,
and unresolved selection all remain visible.

This is an audit of declared profiles, not an architecture executor. It does not instantiate
components, call a model, resolve a provider, estimate uncertainty, or approve a release.

## Request

```json
{
  "objectives": [
    { "axis": "admissible_rate", "direction": "higher_is_better" },
    { "axis": "cost_units", "direction": "lower_is_better" }
  ],
  "profiles": [
    {
      "candidate": "cheap",
      "values": {
        "admissible_rate": { "state": "measured", "value": 0.80 },
        "cost_units": { "state": "measured", "value": 10.0 }
      }
    },
    {
      "candidate": "missing-latency",
      "values": {
        "admissible_rate": { "state": "measured", "value": 0.90 },
        "cost_units": { "state": "unmeasured", "reason": "not_attempted" }
      }
    }
  ],
  "relations": [{ "left": "cheap", "right": "missing-latency" }],
  "max_rows": 100
}
```

There must be 1–64 unique objective axes and 1–512 profiles. Every profile must mention every
objective either as a measured value or as a typed `unmeasured` value. Leaving an axis out is a
different error: silence is not a declared measurement gap. The server bounds relations to 256
front-only pairs and serialized input to 10,000,000 bytes.

The unmeasured reason is the closed atlas vocabulary, including `not_attempted`,
`no_eligible_evidence`, `all_trials_non_evaluable`, `all_trials_abstained`,
`inaccessible_by_policy`, `deferred_acquisition`, and `out_of_scope_by_declared_use`.

## What the result means

The successful schema is `bioprism-mcp/lab-pareto-audit/0.1`.

- `admissions` records each input's archive admission, including displaced members; bounded rows
  carry `admissions_omitted`.
- `front.members` is the complete non-dominated archive under the declared objectives.
- `front.unresolved` identifies members whose standing depends on an unmeasured axis and retains
  the reason for each hole.
- `front.selection` is `unique`, `ambiguous`, or `empty`. An ambiguous front is a finding, not a
  ranking problem for the endpoint to solve.
- `archived` retains dominated candidates and the candidate that dominated each one;
  `archived_omitted` makes response truncation explicit.
- `relations` gives requested pairwise relations only for final-front members. A request involving
  an archived candidate refuses instead of reporting a relation that no longer describes the
  final front.

The kernel treats an unmeasured axis as incomparable even when all other measured axes would have
settled the comparison. This prevents a candidate from dominating simply by skipping an expensive
measurement. A genuine measured trade-off is also incomparable: for example, higher admissible
rate with higher cost and lower admissible rate with lower cost both remain on the front.

## Fail-closed behavior

```json
{
  "ok": false,
  "schema": "bioprism-mcp/lab-pareto-audit/0.1",
  "stage": "profile_insertion",
  "profile_index": 3,
  "candidate": "candidate-3",
  "refusal": "candidate `candidate-3` says nothing at all about objective `cost_units`",
  "fail_closed": true,
  "inserted_profiles": 3
}
```

Objective validation, profile insertion, and requested relation projection are separate refusal
stages. A failed insertion never returns the partial archive as if it were a complete comparison.
The SDK parser additionally rejects non-reconciled row counts, unknown selection labels, and front
or archive projections whose counts do not match their rows.

## SDK surfaces

- Python exposes `LabParetoAuditArgs`, `LabParetoAuditReport`, and
  `lab_pareto_audit_report(...)` through `Workspace`, `AsyncWorkspace`, `ApiClient`, and
  `AsyncApiClient`.
- TypeScript exposes `labParetoAudit(...)`, `LabParetoAuditArgs`, and
  `LabParetoAuditResult`.

Use this surface with `lab_plan` when a candidate still needs evidence acquisition, and with
`pack_health_assess` or benchmark/oracle reviews when a profile value needs external provenance.
The Pareto front itself is not a deployable recommendation and does not license a caller to hide
an unresolved axis behind a tie-break rule.
