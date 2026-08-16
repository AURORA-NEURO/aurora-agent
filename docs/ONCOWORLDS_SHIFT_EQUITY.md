# OncoWorlds era, site, and equity checks

The 30.27 boundary is about more than a model score surviving a site change. Classification
criteria change, assays are unavailable at some sites, administrative population descriptors are
not biological mechanisms, and pooled performance can hide unstable subgroups. The two tools in
this document expose those facts as auditable projections.

## `oncoworlds_era_shift_check`

Versioned results use:

```text
bioprism-mcp/oncoworlds-era-shift-check/0.1
```

The required `left` and `right` inputs are serialized `bioprism-oncoworlds::era::Cohort` values.
When classification versions differ, `mapping` must be supplied and must cover every entity label
present in the older cohort. A partial mapping is refused with `incomplete_mapping`; no entity is
silently copied across a criteria revision. The projection keeps:

- both cohort names, sites, classification versions, entity labels, and reconciled counts;
- whether a mapping was declared, how many fates it carries, and whether its versions match;
- optional site-assay contexts, where `not_collected` remains distinct from a negative call; and
- optional population-descriptor checks, where administrative descriptors may stratify a report
  but are refused as mechanistic variables.

An accepted cross-version comparison is still a comparability decision, not a claim that the
cohorts have identical case mix or that an assay was performed. The tool does not infer label
semantics, run assays, estimate site effects, or infer biology from race, geography, or resource
availability.

The site evidence projection deliberately reports:

```json
{
  "availability": {"availability": "unavailable_at_site"},
  "observation": {"unobserved": "not_collected"},
  "negative_call_supported": false,
  "negative_call_refusal_kind": "resource_absence_read_as_biology"
}
```

## `oncoworlds_equity_check`

Versioned results use:

```text
bioprism-mcp/oncoworlds-equity-check/0.1
```

The input is a serialized `PooledScore`. A pooled value alone returns a fail-closed
`pooled_score_only` refusal. Every retained subgroup must have a nonzero `n`, an estimate, and an
uncertainty interval; missing intervals produce `unquantified_subgroup`, and empty groups produce
`empty_subgroup`. The report reconciles:

- `pooled_value`;
- the complete `subgroups` array;
- `subgroup_count` and `interval_count`; and
- `all_intervals_present`.

Small subgroups are not discarded. Their uncertainty remains visible in the interval, so the
reader can distinguish an unstable estimate from a stable low-performing group. The endpoint does
not calculate estimates, confidence intervals, calibration, parity metrics, or causal fairness.

## SDK usage

Python provides `OncoWorldsEraShiftCheckArgs`, `OncoWorldsEraShiftCheckReport`,
`OncoWorldsEquityCheckArgs`, and `OncoWorldsEquityCheckReport`, plus typed report helpers on
`Workspace`, `ApiClient`, and their async counterparts. TypeScript provides
`oncoworldsEraShiftCheck` and `oncoworldsEquityCheck` with the corresponding result types.

Both SDKs reject mismatched outcome/refusal tags, missing evidence accounting, inconsistent
mapping counts, invented negative calls, incomplete intervals, and contradictory subgroup counts.
