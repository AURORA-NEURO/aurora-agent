# OncoWorlds Clonal History Check

`oncoworlds_clonal_history_check` audits caller-supplied ancestry hypotheses against a supplied
tumour population. It does not infer a phylogeny from raw variants and it never selects the first
compatible history. Successful responses use:

```text
bioprism-mcp/oncoworlds-clonal-history-check/0.1
```

## Candidate accounting

Every submitted candidate is retained in exactly one of:

- `compatible`: histories that pass unknown-subclone, cycle, parent-fraction, and whole-tumour
  arithmetic checks;
- `rejected`: the legacy pair form `[history, refusal]`; or
- `rejected_records`: the typed form with `history`, typed `refusal`, `refusal_kind`, and
  human-readable `refusal_text`.

`candidate_count` must equal `compatible_count + rejected_count`. The Python SDK exposes typed
`OncoClonalHistoryProjection` and `OncoClonalRejectedHistoryProjection` records while retaining the
legacy arrays for compatibility. Rejection kinds remain explicit: `fractions_exceed_whole`,
`child_exceeds_parent`, `cyclic`, `unknown_subclone`, and other typed refusal variants are not
collapsed into a generic invalid-history flag.

## Ambiguity is a result

`unique_history` is a tagged domain result, not an exception. `unique_status` is one of:

| Status | Meaning |
| --- | --- |
| `unique` | Exactly one compatible history exists and is retained under `history` |
| `ambiguous` | Multiple compatible histories survive; the refusal carries their count |
| `refused` | No unique history can be released for another typed refusal |

The SDK validates the status against `unique_history.ok` and its typed refusal. Multiple compatible
histories therefore remain auditable ambiguity rather than being reduced to one apparent tree.

## Edge representation

Canonical serialized edges are parent/child pairs, for example:

```json
{ "edges": [["parent", "child"]] }
```

`OncoClonalHistoryProjection.edges` exposes those pairs as typed tuples and rejects duplicate edge
pairs. The raw history is retained so future population metadata can be carried without losing
information at the SDK boundary.

## Scope and limitations

Compatibility is arithmetic consistency, not biological truth. The check does not enumerate trees,
model sequencing error, estimate detection sensitivity, convert allele fractions, or establish
treatment causation. It is a bounded hypothesis audit for downstream research reasoning.
