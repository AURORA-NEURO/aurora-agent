# Bioevaluation claim-grounding audit

`bioeval_grounding_audit` exposes the claim-evidence graph in `bioprism-bioevalx` as a bounded,
reviewable MCP projection. It is designed for the failure mode where a report has many citations
but the cited artifacts are adjacent, unresolved, stale, lineage-free, or contradictory. The route
therefore reports a partition of claim states and typed provenance findings; it never reduces the
graph to a citation count or a grounding percentage.

## Request

```json
{
  "claims": [
    { "id": "amplified" },
    { "id": "safe-to-release" }
  ],
  "evidence": [
    {
      "id": "fish-panel",
      "last_modified": "2026-01-01T00:00:00Z",
      "lineage": ["specimen-17", "library-prep-17"],
      "locator_status": {
        "locator": "resolved",
        "digest": "sha256:..."
      }
    },
    {
      "id": "review-note",
      "last_modified": "2026-04-01T00:00:00Z",
      "locator_status": { "locator": "not_checked" }
    }
  ],
  "edges": [
    { "claim": "amplified", "evidence": "fish-panel", "kind": "supports" },
    { "claim": "amplified", "evidence": "review-note", "kind": "contradicts" },
    { "claim": "safe-to-release", "evidence": "review-note", "kind": "adjacent" }
  ],
  "stale_against": "2026-03-01T00:00:00Z",
  "max_items": 100
}
```

`claims`, `evidence`, and `edges` are each bounded at 4096 rows. IDs are non-empty and at most
256 bytes. `last_modified` and `stale_against` are explicit timestamps; the route never reads a
clock. `max_items` bounds each row and identifier projection independently, and every bounded
projection retains `total` and `omitted` counts.

## Locator states

The evidence kernel has three locator states:

- `resolved` carries a caller-supplied digest and is the only state that can make a supporting edge
  shown rather than asserted;
- `not_checked` means that no dereference was performed, not that the locator is good; and
- `unresolvable` carries a detail describing the failed resolution.

The route records these states. It does not fetch a URL, open a file, compare a digest, or infer
that a caller's digest is correct. A resolved locator is provenance evidence supplied by the caller,
not an external verification performed by the agent.

## Five-way claim partition

The underlying `Grounding` kernel classifies each declared claim as exactly one of:

- `supported`: at least one resolved supporting edge and no contradiction;
- `contested`: supporting and contradicting edges both exist, regardless of support count;
- `contradicted`: only contradicting edges exist;
- `unsupported`: no supporting or contradicting edges exist, including adjacent-only citations; or
- `support_unverified`: supporting edges exist but none has a resolved locator.

This is intentionally not a weighted score. A claim with three supports and one contradiction is
`contested`, not 75% grounded. `adjacent` is also retained as a separate edge kind because a
relevant-looking citation that does not bear on the atomic claim must not be credited as support.

## Successful projection

Schema is `bioprism-mcp/bioeval-grounding-audit/0.1`.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/bioeval-grounding-audit/0.1",
  "workflow": "bioeval_grounding_audit",
  "claims": { "rows": [], "returned": 0, "total": 2, "omitted": 2 },
  "evidence": { "rows": [], "returned": 0, "total": 2, "omitted": 2 },
  "edges": { "rows": [], "returned": 0, "total": 3, "omitted": 3 },
  "census": {
    "claims": 2,
    "supported": 0,
    "contested": 1,
    "contradicted": 0,
    "unsupported": 1,
    "support_unverified": 0,
    "adjacent_citations": 1,
    "fully_grounded": false
  },
  "graph": {
    "claim_count": 2,
    "evidence_count": 2,
    "edge_count": 3,
    "support_edge_count": 1,
    "contradiction_edge_count": 1,
    "adjacent_edge_count": 1,
    "duplicate_edge_count": 0
  },
  "locator_census": { "resolved": 1, "not_checked": 1, "unresolvable": 0 },
  "staleness": {
    "requested": true,
    "freeze": "2026-03-01T00:00:00Z",
    "stale_count": 1,
    "stale_evidence": { "ids": ["review-note"], "total": 1, "omitted": 0 }
  },
  "findings": {
    "contested_claims": { "ids": ["amplified"], "total": 1, "omitted": 0 },
    "contradicted_claims": { "ids": [], "total": 0, "omitted": 0 },
    "unsupported_claims": { "ids": ["safe-to-release"], "total": 1, "omitted": 0 },
    "support_unverified_claims": { "ids": [], "total": 0, "omitted": 0 },
    "lineage_gap_evidence": { "ids": ["review-note"], "total": 1, "omitted": 0 },
    "orphan_evidence": { "ids": [], "total": 0, "omitted": 0 },
    "adjacent_citation_count": 1,
    "duplicate_edge_count": 0
  },
  "guarantees": ["..."],
  "limitations": ["..."]
}
```

Claim rows include their state, grounding boolean, supporting/contradicting/adjacent evidence IDs,
and resolved versus unresolved support counts. Evidence rows include locator state, digest-bearing
resolution status, last-modified timestamp, lineage, linked claims, per-edge-kind counts, stale
status, lineage-gap status, and orphan status. Edge rows retain insertion order and identify
whether their evidence locator was resolved. Set-derived finding IDs are canonical lexical order;
edge rows preserve the caller's order.

## Staleness, lineage, and graph hygiene

`stale_against` is a reproducible freeze comparison. Evidence whose `last_modified` is later than
the freeze is listed as stale; omitting the freeze returns `requested: false` rather than implying
that all evidence is current. A source modified after a benchmark freeze is not silently treated as
usable simply because its locator resolves.

`lineage_gap_evidence` comes directly from the kernel's empty-ancestry predicate. It can include a
linked source such as a review note: linkage to a claim does not create specimen ancestry. Conversely,
an evidence object can have complete lineage and still be an orphan if no edge uses it. These are
separate findings because they require different remediation.

`duplicate_edge_count` counts repeated `(claim, evidence, kind)` triples without deleting them from
the graph. The kernel preserves insertion order and allows repeated edges; the audit makes that
possible inflation visible instead of rewriting caller history.

## Refusals and boundaries

Missing top-level arrays, malformed envelopes, invalid scalar shapes, and out-of-bound requests are
argument errors. Duplicate claim/evidence IDs, malformed timestamps or locator states, unknown edge
kinds, and edges to undeclared endpoints return a structured `ok: false` response with a stage,
actionable refusal, and `fail_closed: true`. No partial graph is projected as a success result.

The route does not extract atomic claims from prose, dereference or hash-check artifacts, resolve
locators against a storage service, validate specimen registries, check assay/analyte compatibility,
estimate causal identification, or establish biological or clinical validity. `fully_grounded` is
only the kernel's all-claims-supported predicate over the supplied graph.

## SDK surfaces

- Python exposes `BioevalGroundingClaimArgs`, `BioevalGroundingEvidenceArgs`,
  `BioevalGroundingEdgeArgs`, `BioevalGroundingAuditArgs`,
  `BioevalGroundingAuditReport`, and `bioeval_grounding_audit_report(...)` through `Workspace`,
  `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`.
- TypeScript exposes the typed claim, evidence, locator, edge-kind, and audit argument interfaces
  plus `bioevalGroundingAudit(...)`; nested census and bounded-row projections remain JSON objects
  so omitted counts and domain refusals are not flattened away.

Use this route for provenance and contradiction review. Use `bioeval_reference_audit` for reference
distribution semantics, `bioeval_acquisition_audit` for ordered evidence-acquisition obligations,
and `epistemic_selection_audit` for decision-relative retention of observed evidence.
