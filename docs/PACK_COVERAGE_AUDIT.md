# Pack coverage audit

`pack_coverage_audit` is the portfolio-level inspection surface for the benchmark-pack catalogue.
It answers a narrower question than pack health:

> Which capability families are represented by the selected declarations, and where are the
> portfolio's coverage gaps or single points of failure?

The endpoint is backed by the `bioprism_packs::coverage` and `bioprism_packs::matrix` kernels. It
does not count catalogue rows, infer performance from a declaration, execute pack instances, or
turn coverage into a health score.

## Request

```json
{
  "section": "15",
  "pack_ids": ["prism.context-acquisition", "bio.causal-inference"],
  "max_items": 100
}
```

All fields are optional. `section` is `all`, `15`, or `29`; it defaults to `all`. `pack_ids`, when
present, is an explicit unique subset of at most 100 catalogue identifiers. A supplied identifier
that is unknown or outside the selected section is not silently discarded. `max_items` defaults to
100 and bounds every disclosed family and matrix projection independently; the response includes
an omission count for each bounded list.

## Successful projection

The result has schema `bioprism-mcp/pack-coverage-audit/0.1` and includes:

- `selected_pack_ids` and `selected_pack_count`, preserving the exact denominator;
- `summary.families`, `covered`, `uncovered`, `singly_covered`, `weakly_covered`, and
  `coverage_fraction`;
- `summary.gap_summary`, a human-readable kernel-derived summary;
- bounded `rows` describing capability-family coverage;
- `uncovered`, `singly_covered`, and `weakly_covered` family lists;
- a bounded `matrix` for pack/family membership and omission counts for every bounded list;
- `guarantees` and `limitations`, including the declaration-level/no-execution boundary.

`covered` means at least one selected pack declares the family. `singly_covered` means the family
has only one selected declaration, making it a portfolio fragility signal. `weakly_covered` is the
packs kernel's separate warning class and must not be merged into `uncovered`; a weak declaration
is not equivalent to absence. `execution_grounded` and related fields in rows preserve oracle
posture but do not convert it into observed validity.

## Fail-closed selection

Unknown identifiers and empty section/subset intersections return a normal structured result with
`ok: false`, `fail_closed: true`, `stage: "pack_selection"`, and a refusal. This is deliberately
different from a zero-coverage success: an empty or misspelled denominator is an input failure,
not evidence that the portfolio covers no families.

```json
{
  "ok": false,
  "schema": "bioprism-mcp/pack-coverage-audit/0.1",
  "stage": "pack_selection",
  "unknown_pack_ids": ["pack-does-not-exist"],
  "refusal": "coverage cannot be computed for unknown pack identifiers",
  "fail_closed": true,
  "guarantees": [
    "an unknown pack is not silently dropped from a coverage denominator"
  ]
}
```

The Python `PackCoverageAuditReport` and TypeScript `PackCoverageAuditResult` keep this refusal
shape visible. Callers should inspect `accepted`/`refused` (Python) or `ok` (TypeScript) before
using summary values, and should never treat `coverage_fraction` as a measured success rate.

## SDK surfaces

- Python: `PackCoverageAuditArgs`, `PackCoverageAuditReport`, and
  `pack_coverage_audit_report(...)` are available through `Workspace`, `AsyncWorkspace`,
  `ApiClient`, and `AsyncApiClient`.
- TypeScript: `packCoverageAudit(...)` accepts `PackCoverageAuditArgs` and returns the typed REST
  envelope containing `PackCoverageAuditResult`.

The endpoint is also listed in the `benchmark_pack_portfolio` workspace capability group beside
`pack_catalogue`, `pack_health_assess`, and the benchmark audit/compiler surfaces. Those surfaces
are complementary: catalogue describes declarations, coverage audits portfolio representation,
health audits observed pack quality, and benchmark compilation produces a reviewed decision cell.
For the adjacent declared release sequence and unsequenced remainder, see
[`PACK_RELEASE_AUDIT.md`](PACK_RELEASE_AUDIT.md).
