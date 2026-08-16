# Pack release audit

`pack_release_audit` exposes the benchmark portfolio's declared release sequencing without
turning a blueprint wave into a readiness approval. It is backed by the
`bioprism_packs::portfolio::release_order` and `unsequenced` kernels and answers two separate
questions:

1. Which selected packs have a stable position in the blueprint's initial release order?
2. Which selected packs are intentionally left outside that order?

The second answer is not a defect to be filled with an invented wave. The portfolio explicitly
leaves a remainder unsequenced, and the endpoint keeps that remainder visible.

## Request

```json
{
  "section": "15",
  "pack_ids": ["prism.context-acquisition", "prism.tool-selection"],
  "max_items": 100
}
```

`section` defaults to `all` and accepts `all`, `15`, or `29`. `pack_ids` is an optional unique
subset of at most 100 identifiers. An unknown identifier or an identifier that belongs to another
section is a fail-closed selection error; it is never omitted from the denominator. `max_items`
defaults to 100 and bounds both `release_order` and `unsequenced` independently.

## Successful projection

The result uses schema `bioprism-mcp/pack-release-audit/0.1` and preserves:

- exact `selected_pack_ids` and `selected_pack_count`;
- `sequenced_count`, `unsequenced_count`, and `release_coverage_fraction`;
- `wave_counts` and `axis_counts` for the selected denominator;
- bounded release rows with selected position, global portfolio position, blueprint module, axis,
  wave, and oracle posture;
- bounded unsequenced rows with the same declaration/oracle context;
- `release_order_omitted` and `unsequenced_omitted` reconciliation counts;
- guarantees and limitations explaining why the result is not approval, registry admission,
  deployment readiness, staffing evidence, dependency resolution, or measured quality.

For the complete section-15 portfolio, the current blueprint declares 13 sequenced packs and 12
unsequenced packs. The endpoint derives those counts from the kernel for the selected subset rather
than hard-coding them into the transport.

`portfolio_position` is the position in the complete stable sequenced order, while
`selected_position` is the position after filtering to the caller's subset. These are intentionally
separate: a subset must not look like a new global release history.

## Fail-closed selection

```json
{
  "ok": false,
  "schema": "bioprism-mcp/pack-release-audit/0.1",
  "stage": "pack_selection",
  "unknown_pack_ids": [],
  "out_of_section_pack_ids": ["bio.statistical-estimands"],
  "refusal": "release order cannot be computed for an unknown or section-incompatible pack selection",
  "fail_closed": true,
  "guarantees": [
    "section-incompatible identifiers are reported rather than reassigned"
  ]
}
```

An empty selection is also refused. This avoids presenting an empty denominator as a complete
release plan or implying that no packs are ready merely because a caller misspelled a filter.

## SDK surfaces

- Python: `PackReleaseAuditArgs`, `PackReleaseAuditReport`, and
  `pack_release_audit_report(...)` are available through `Workspace`, `AsyncWorkspace`,
  `ApiClient`, and `AsyncApiClient`.
- TypeScript: `packReleaseAudit(...)` accepts `PackReleaseAuditArgs` and returns the typed REST
  envelope containing `PackReleaseAuditResult`.

Use this surface with `pack_catalogue` for declaration details, `pack_coverage_audit` for
capability-family gaps, and `pack_health_assess` for observed reportability. None of those
projections alone is a registry approval or a measured performance claim.
