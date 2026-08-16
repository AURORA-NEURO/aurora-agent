# Literature binding and citation support

`literature_bind_check` makes a literature statement usable as a bounded citation without
silently upgrading what the source says. It is the public MCP projection of
`bioprism_modalities::literature::{LiteratureClaim, BoundClaim}` and keeps two decisions
separate:

1. **Binding:** may this source claim be attached to this target scope under the requested
   evidence tier and evaluation horizon?
2. **Citation support:** once bound, may it support the requested `ClaimKind`?

That separation matters because a paper can be validly bound as a statement about a paper while
still being unable to support a biological measurement or causal claim. The literature modality
only supports `published_claim_support`; it does not turn prose into a specimen observation.

## Request

```json
{
  "claim": {
    "text": "The source's own wording",
    "provenance": {
      "identifier": "doi:10.0000/example",
      "tier": "primary",
      "published": "2024-01-01T00:00:00Z",
      "population": {"disease": "diffuse_glioma"},
      "retraction": "none"
    }
  },
  "target": {"disease": "diffuse_glioma", "site": "brain"},
  "at_tier": "primary",
  "horizon": {"horizon": "open"},
  "claim_kind": "published_claim_support"
}
```

`at_tier` is the tier at which the caller wants to cite the claim, not merely the source's
declared tier. `horizon` must be explicit: `{ "horizon": "open" }` for a non-historical task or
`{ "horizon": "as_of", "instant": "2023-12-31T23:59:59Z" }` for a historical evaluation.
Scopes use the typed `ScopeKey` wire representation. The target must refine the source's stated
population; an omitted source population is a refusal, not an unconstrained population.

## Outcome contract

The response schema is `bioprism-mcp/literature-bind-check/0.1`:

| `outcome_kind` | Meaning |
| --- | --- |
| `bound` | Binding succeeded, but citation support was not requested. |
| `citable` | Binding succeeded and `published_claim_support` was admitted. |
| `cite_refused` | Binding succeeded, but the requested claim kind is not supported by literature. |
| `refused` | Binding failed before a citable claim could exist. |

Binding refusals are typed and preserve the source identifier and relevant witness:

- `citation_laundering`: a review, guideline, or database was requested as primary evidence;
- `unstated_population`: the source did not declare the population it studied;
- `population_mismatch`: the requested target is outside that population;
- `temporal_leakage`: the source was published after the declared historical horizon;
- `retracted_source`: a flagged source was used without an explicit warrant.

A flagged source can be bound only with `flag_warrant`. The warrant is carried in the serialized
bound-claim evidence so a downstream reader can distinguish an explicit exception from a clean
source. This is an allowance for claims about a flagged source or field history, not a clearance
of the source.

## SDK usage

Python exposes `LiteratureBindCheckArgs`, `LiteratureBindCheckReport`, and
`literature_bind_check_report(...)` through synchronous MCP, asynchronous MCP, and HTTP client
facades. TypeScript exposes `LiteratureBindCheckArgs`, `LiteratureBindCheckResult`, and
`ApiClient.literatureBindCheck(...)`. Both SDKs preserve `bound`, `citable`, binding refusals,
citation refusals, guarantees, and limitations without converting a refusal into `false` data.

The workflow performs no retrieval, DOI/PMID resolution, entailment checking, citation-graph
construction, venue ranking, or clinical interpretation. Identifiers are opaque; the claim text
remains the source's own wording. A successful response is a deterministic evidence-boundary
decision, not independent verification of the paper or its biology.
