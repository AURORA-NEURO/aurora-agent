# Cellosaurus cell-line identity and curation metadata

This repository-defined reader has no available blueprint module identifier. It adds an
explicitly registered source operation, `cellosaurus.glioblastoma_cell_line_metadata`, for one
fixed public metadata window. The optional `cellosaurus-glioblastoma-metadata` capability profile
installs this one reader; the default catalogue stays unchanged. Provider manifest `.4` names
twenty reader paths and seventeen declared API groups. Earlier `.3`/nineteen-path/sixteen-group
records remain tied to their earlier source revisions. An operation descriptor's `live_adapter` label describes its implementation path;
it is not evidence that the committed adapter has executed against the live service.

The source contract is grounded in the official [API documentation](https://api.cellosaurus.org/),
[field reference](https://api.cellosaurus.org/api-fields), [OpenAPI document](https://api.cellosaurus.org/openapi.json),
and one separately retained 7,883-byte format capture. The published success JSON schema did
not specify the native response structure. The format capture predates this reader and does
not establish live reader conformance. Synthetic fixtures measure local admission/refusal
behavior; they are distinct from that historical capture and from any future separately
authorized default-operation proof. No new source request is required to run these tests.

## Fixed authority and native admission

The only public input is exact integer `timeout_ms` from 100 through 30,000. An explicitly
approved operation makes one anonymous GET to the installed `api.cellosaurus.org` search route:
`q=ox:sapiens din:glioblastoma`, `start=0`, `rows=5`, JSON,
`fields=ac,id,sy,ox,din,caution,problematic,dt`, `sort=ac asc`. The URL, query, field set and
existing reviewed HTTPS policy/configuration match the format capture. Callers cannot supply
another endpoint, query, cursor, field list, credential or retry policy. Redirects, retries,
fallbacks, source-link visits and release-info calls are absent. No service-wide or distributed
quota guarantee is made.

Admission requires the exact `{Cellosaurus:{cell-line-list:[...]}}` wrapper and zero through
five records. Required accession/name/species/disease arrays cannot be missing or null.
Each accession array contains exactly one `primary` entry; records must already have strictly
ascending unique primary strings. Each ordered name array has exactly one `identifier` and
zero or more `synonym` entries. The identifier need not be first. The reader never sorts,
normalizes, deduplicates, selects an alias fallback, or joins identifiers to another source.

Species references retain all native NCBI_TaxID members and require the explicit source
assertion `9606`. Other NCBI_TaxID assertions remain visible; this is not independent human-only
validation. Disease references require NCIt identity strings, without a concept allowlist or
equivalence mapping. `C3058` and `C39750` remain distinct source strings/labels. A present empty
disease list is admitted as observed empty metadata; query/disease correspondence remains
`unestablished`, without confirmed glioblastoma classification or negative scientific evidence.

Reference `category`, `iri` and `url`, and record `created`, `last-updated`, `entry-version` and
`comment-list`, retain missing/null/observed states. Dates and versions remain exact strings;
they are not parsed or promoted to API release or snapshot identity. Native URLs and HTTP IRIs
are inert strings. A missing optional field has no native value pointer/citation and cites the
containing native object as absence context. Explicit null cites its actual pointer/value.
Empty arrays remain observed arrays, not missing or null.

Only exact comment objects with category `Problematic cell line` are admitted in this version.
That category was observed in the capture. The requested Caution field's serialization was
not observed; guessed categories and unknown/unselected fields refuse the entire normalized
artifact. Missing, null or empty comments never establish absence of an identity problem or
an authenticity/contamination/quality/suitability pass. Only missing and nonempty comment arrays
were observed in the historical capture. Null/empty optional shapes and optional reference
missingness are explicit synthetic policy cases, not claims about source-wide behavior.

## Replay, bounds and meaning

The bundle retains exact response bytes/base64/SHA256, parsed native JSON, typed transport
metadata, fixed configuration/policy identity, supplied UTC retrieval clock, every selected
observation, ordered array entry, receipt and native citation. Citation pointers resolve relative
to `artifact_pointer_root=validated_cellosaurus_bundle`, under
`/source_inputs/0/native_payload`. Each citation retains its exact source value/digest, original
clock, response digest and native primary accession. Default reviewed transport and caller
fixtures have distinct provenance; each caller registration/direct request has a nonce binding.
Raw replay and hashes authenticate neither the caller's assertions nor source truth.

`validate_cellosaurus_cell_line_bundle` decodes canonical base64, reparses the retained raw
bytes and rebuilds the complete artifact. Restamping an outer digest cannot hide a changed
projection, citation, query, receipt, scope or metadata field. A coherent whole-source
replacement can still be supplied by a caller; it remains an unauthenticated caller assertion.
Validation never falls through to source acquisition. When normalization refuses, no partial
bundle is returned; an application observing a typed response must separately retain raw bytes
and failure metadata if it needs an unsuccessful-acquisition record.

Local caps are 524,288 response bytes, five records, depth 20, 30,000 JSON nodes, 128 names,
16 species references, 32 disease references and 32 comments per record. Identity/category/
date/version strings are at most 128 UTF-8 bytes; names/labels/links 4,096; comment text 32,768.
The complete derived citation index is capped at 4,096 and canonical bundle at 8,000,000 bytes.
These are local admission limits, not source guarantees. Some otherwise-shaped combinations
exceed the citation/artifact bound and refuse as a whole. No prefix is presented as complete,
and no proof that every combination of per-field maxima fits is claimed.

Returned count is the exact admitted list length. Total, unreturned count, page count, source
release/snapshot, completeness and omitted influence remain unknown. A short or empty window
is not exhaustive absence, prevalence, a donor/patient/cohort/assay denominator, an independent
publication count, or decision sufficiency. Curation free text and field allowlists do not
establish semantic absence of incidental context. No scientific equivalence, suitability
ranking, biological-use instruction, clinical or laboratory action follows from this metadata.

```python
from prism_sdk import (
    WorkflowOperationCatalog, register_cellosaurus_cell_line_operations,
    validate_cellosaurus_cell_line_bundle,
)

catalog = register_cellosaurus_cell_line_operations(WorkflowOperationCatalog())
# Registration dispatches nothing. Execution requires its separate explicit source approval.
# validate_cellosaurus_cell_line_bundle(retained_bundle) is entirely offline.
```

The [capability selector](WORKFLOW_CAPABILITIES.md) also offers a model-free one-step template:

```console
aurora-agent workflow-template --template cellosaurus-glioblastoma-metadata --task "Inspect the fixed Cellosaurus metadata window" --out cell-lines.json
```

Its native requirements retain the one required source operation/output, exact 30,000 ms
arguments, one-step/one-call cap and exact task identity. Task text labels the fixed inspection;
it cannot become an arbitrary cell-line query. Planning/inspection grant no source authority.
Worker startup must separately enable `--capability cellosaurus-glioblastoma-metadata`.
