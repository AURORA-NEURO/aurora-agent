# OncoWorlds clonal evidence safeguards

`oncoworlds_clonal_evidence_check` exposes the specimen-level and resistance boundaries from
blueprint section 30.12. It is a structural evidence report, not a phylogeny constructor, allele-
fraction converter, causal estimator, diagnostic classifier, or treatment recommender. The
versioned schema is `bioprism-mcp/oncoworlds-clonal-evidence-check/0.1`.

## Independent checks

- `promotion` accepts a serialized `SpecimenObservation` and applies the asymmetric promotion
  rule. A present call can produce `present_in_sampled_regions`; an absent or below-detection call
  produces only `undetected_above_fraction` when a sensitivity and sampled region support it.
  `not_collected`, failed, redacted, and otherwise uninformative observations remain typed
  refusals, never tumour-level negatives.
- `resistance` accepts `diagnosis` and `recurrence` observations. It returns the full
  `not_excluded` explanation set and `excluded` arithmetic witnesses. A unique explanation is
  reported only when every alternative except one is excluded; otherwise `ambiguous` remains an
  explicit refusal. De novo emergence is never removed by this pair of specimens alone.
- `attribution` accepts a treatment, molecular marker, and `temporal_association_only` design.
  Temporal ordering is returned as `unsupported_directionality`; treatment causation is never
  minted from recurrence timing.

At least one section is required. The report keeps each section's `allowed`, `outcome_kind`, and
typed refusal separate, then reconciles `all_admissible`, `check_count`, and `refusal_count` across
only the requested sections.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/oncoworlds-clonal-evidence-check/0.1",
  "outcome_kind": "report",
  "all_admissible": false,
  "check_count": 2,
  "refusal_count": 1,
  "checks": {
    "promotion": {"allowed": true, "outcome_kind": "undetected_above_fraction"},
    "attribution": {
      "allowed": false,
      "outcome_kind": "refused",
      "refusal_kind": "unsupported_directionality"
    }
  }
}
```

`OncoClonalEvidenceCheckArgs` and `OncoWorldsClonalEvidenceCheckReport` are available on the
Python sync/async MCP and HTTP facades. The TypeScript client provides
`oncoworldsClonalEvidenceCheck(...)`; nested domain records remain structured JSON so the Rust
crate remains authoritative for clonal arithmetic and refusal semantics.
