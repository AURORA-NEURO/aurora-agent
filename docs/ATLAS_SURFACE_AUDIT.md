# Atlas surface audit

`atlas_surface_audit` exposes the publication-facing remainder of `bioprism-atlasx`.
The base `atlas_report` reads an Atlas and reports measured cells, holes, failure debt, and
composite eligibility. This route reads a `bioprism-metrics::CapabilityGrid` and makes the
denominator, the claim-blocking holes, the discharge between two readings, and the failure-browser
surface independently inspectable.

The route is deliberately an audit and projection contract. It does not run an assay, recalculate
an Atlas, dereference a failure record, authenticate a publication declaration, or turn a failure
histogram into a prevalence claim.

## Why the second surface exists

An Atlas is an evidence index. A CapabilityGrid is a reading of that evidence under explicit
conditions. The distinction matters at publication time:

- coverage is measured capabilities divided by capabilities in this grid, not an unstated ontology
  denominator;
- zero capabilities in an empty grid are vacuous evidence, not zero-percent performance;
- a hole closed by `OutOfScopeByDeclaredUse` is not the same evidence as a hole that became
  measured;
- a failure browse has failure records but no attempt denominator;
- withholding a record is a publication state, not an empty bucket;
- a surface can explicitly refuse a question and still be sound.

The endpoint preserves those distinctions as separate output fields and refusal stages.

## Request

`grid` is required and must be a serialized `CapabilityGrid`. Its label becomes the
coverage subject and, unless overridden, the failure-browse subject. The grid carries measurement
conditions and every cell is either a measured estimate with an effective size or an
`UnmeasuredReason`.

`later_grid` is optional. When supplied, it must carry the same grid label. The route invokes
`DebtStatement::discharged_by`, returning four disjoint ledgers:

- `measured`: holes that became measured cells;
- `declared_away`: holes closed by a declared intended-use boundary;
- `persisting`: holes still present;
- `newly_unmeasured`: capabilities present only in the later reading.

No net “holes closed” number is emitted because that number would combine evidence with scope
declaration.

`failures` is an optional bounded array of serialized `FailureRecord` values. The
`facet` is a closed vocabulary:

```text
mechanism
first_divergence_stage
severity
inducement
architecture_component
```

Every bucket retains failure identifiers. A record whose publication declaration is not
`available` is placed in a state-bearing withheld bucket, so a chart cannot leak its
diagnosis by publishing a count.

`visibility` contains `{failure_id,state}` declarations. The wire names are the
canonical publication-state names, including hyphenated values such as `under-review` and
`not-comparable`. Duplicate declarations, absent record identifiers, duplicate failure
identifiers, and mixed failure-taxonomy versions are refusals.

`rate_capabilities` asks for explicit `FailureBrowse::rate_against` projections. Each
rate uses the visible, system-charged failures from the browse and the matching grid cell's
effective-size denominator. The route does not infer a trial incidence denominator.

The four boolean policies are fail-closed:

- `require_no_holes` refuses any unmeasured capability;
- `require_no_blocking_debt` refuses any hole that still blocks a claim;
- `require_no_withheld` refuses any withheld publication record;
- `require_sound_surfaces` refuses if either atlasx surface answers outside its declaration.

`max_items` bounds each returned projection independently. Every truncated list includes an
omitted count.

## Successful result

A successful result uses schema `bioprism-mcp/atlas-surface-audit/0.1` and includes:

```json
{
  "ok": true,
  "coverage": {
    "subject": "surface-system",
    "total_capabilities": 3,
    "measured": 1,
    "unmeasured": 2,
    "blocking": 2,
    "closed_by_declaration": 0,
    "vacuous": false,
    "profile_coverage": {
      "outcome": "answered",
      "cell": {
        "kind": "share",
        "value": { "numerator": 1, "denominator": 3 }
      }
    }
  },
  "debt_discharge": {
    "measured": { "rows": ["cohort.statistics"], "total": 1, "omitted": 0 },
    "declared_away": { "rows": ["causal.interpretation"], "total": 1, "omitted": 0 },
    "persisting": { "rows": [], "total": 0, "omitted": 0 },
    "newly_unmeasured": { "rows": [], "total": 0, "omitted": 0 },
    "any_evidence": true
  },
  "failure_browse": {
    "records_browsed": 2,
    "visible": 1,
    "withheld": 1,
    "shares_sum_to_one": true,
    "buckets": []
  },
  "rate_checks": {
    "rows": [
      {
        "capability": "identity.lineage",
        "answer": {
          "outcome": "answered",
          "cell": { "kind": "score", "value": 0.25 }
        }
      }
    ]
  },
  "surface_audits": {
    "sound": true
  }
}
```

The full `coverage.holes`, `failure_browse.buckets`, surface audit records, guarantees,
limitations, policy selection, and bounded rate rows remain in the response. A typed SDK client may
inspect the structured projection while retaining `to_dict()` access to the raw response.

## Refusal behavior

Invalid serialized domain objects are returned as structured fail-closed refusals rather than
partially projected success. Representative stages include:

```text
grid_deserialization
later_grid_deserialization
debt_discharge
coverage_policy
blocking_debt_policy
failure_deserialization
facet_deserialization
visibility_deserialization
failure_browse
visibility_policy
surface_soundness
rate_capability_deserialization
```

Each refusal carries `ok: false`, `fail_closed: true`, the stage, the refusal text, and
the same guarantee/limitation posture as the successful route. A transport-level argument error
is reserved for malformed envelopes or safety-bound violations.

## SDK surfaces

Python exposes `AtlasSurfaceAuditArgs`, `AtlasSurfaceCoverageReport`,
`AtlasSurfaceBrowseReport`, `AtlasSurfaceAuditReport`, and
`atlas_surface_audit_report(...)`. The typed facade is available through `Workspace`,
`AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`. The argument object checks
facet membership, list bounds, JSON size, boolean policies, and maximum projection size. The report
keeps successful coverage, browse, discharge, rate, and soundness layers separate from fail-closed
refusals.

TypeScript exposes `AtlasSurfaceAuditArgs`, `AtlasSurfaceCoverageResult`,
`AtlasSurfaceBrowseResult`, `AtlasSurfaceAuditResult`, and
`PrismClient.atlasSurfaceAudit(...)`. The facet is a string union and refusal fields remain
optional because the same result type represents both a successful audit and a typed refusal.

## Nonclaims

This route does not claim:

- that a measured score is biologically valid or clinically useful;
- that effective size is an independent trial count unless the grid's conditions say so;
- that a failure record is complete, correctly diagnosed, or externally authenticated;
- that a publication-state declaration enforces access control;
- that a clean surface audit proves the underlying evidence is true;
- that a successful rate projection is causal, prevalence, or population incidence;
- that a full public atlas renderer, leaderboard, web publication workflow, or durable registry exists.
