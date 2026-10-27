# Local CIViC curation metadata research review

This repository-defined extension adds local inspection of an already validated
[CIViC source bundle](CIVIC_GLIOMA_EVIDENCE.md). It has no available detailed blueprint module.
It preserves the fixed accepted glioblastoma-name first page, every returned item, native
curation categories, flags, revision counts, source identifiers, missingness and omissions.
It does not interpret curation levels, directions or ratings as scientific quality or clinical
confidence. These local operations are not additional integrations. Their original manifest `.3`
checkpoint counted 19 implemented paths in 16 declared API groups; later manifest `.4` separately
adds the [Cellosaurus reader](CELLOSAURUS_CELL_LINE_METADATA.md) for twenty paths/seventeen groups.
Neither the local analysis nor its synthetic replay adds another reader.

```python
from prism_sdk import (
    analyze_glioma_civic_curation,
    glioma_civic_curation_review_handoff,
    validate_glioma_civic_curation_handoff,
)

analysis = analyze_glioma_civic_curation(
    question="Inspect the supplied curation metadata", civic_bundle=retained_bundle,
)
handoff = glioma_civic_curation_review_handoff(analysis)
assert validate_glioma_civic_curation_handoff(handoff) == handoff
```

These functions make no provider, source or catalogue-execution calls. The question is a bounded
label, never a source query or a general interpretation of research requirements. Direct Python
calls may omit `civic_bundle`; the registered operation requires an explicit object or null.
Null produces `needs_evidence` with absent coverage and no fabricated native count. A supplied
valid empty page remains `observed_empty_window`, with citations to the actual empty array;
it is not negative scientific evidence. Invalid native input refuses the operation.

## Native observations and source ID groups

Every item stays in ascending native evidence-ID order. Its original field observations and
native pointers are retained, including status, flag, open revision count, evidence type/level/
direction/rating, disease metadata and source metadata. Required flag/revision fields cannot
silently become missing values: the source validator rejects invalid required fields. Nullable
disease, rating, disease DOID and page cursor retain missing, explicit null and observed states.

`source_flagged` and `source_open_revisions_reported` are independent metadata attention reasons.
Their absence is not a scientific pass, and they do not rank, select or discard items. Status
`ACCEPTED`, direction `SUPPORTS` or `DOES_NOT_SUPPORT`, levels A–E and ratings 1–5 remain CIViC
assertions. There is no numerical quality score or inference about a statement's biological
meaning, which is not included in this source contract.

Groups compare exact native integer `source.id` values only. All member evidence items remain
separate with their original `{id, citationId, displayType}` values. The same ID with different
labels gets a metadata-discrepancy state; different IDs with identical labels remain separate.
There is no normalization, preferred publication, cross-ID join or deduplication. The optional
group count means distinct observed native IDs in this supplied page. Independent publication,
patient, cohort and assay counts remain unestablished and carry no denominator.

The complete original coverage, omissions and limitations are copied alongside the retained
bundle. Reported totals, later-page flags and cursors remain source assertions; no cursor is
followed. Completeness and omitted influence stay unknown even when `hasNextPage` is false.
Caller-supplied transport mode and nonce binding remain in the embedded bundle; replay does not
promote a supplied fixture or caller response into verified live acquisition or source truth.

## Full native citation closure and replay

Each citation identifies the source operation, bundle and response digests, retrieval clock,
native value and value digest. `artifact_pointer_root: "validated_civic_bundle"` means its
`artifact_pointer` is relative to the embedded source bundle, not the whole analysis or handoff.
The separate native payload pointer addresses the original admitted JSON.

The index contains the connection, total count, page info, next-page flag, optional cursor,
nodes array, every item row, and every present/null selected item/disease/source field. A missing
field has no value citation: it references the nearest containing native row or object as
absence context. No pointer or null value is invented for a field absent from the response.

At most six connection/page citations plus 17 citations per returned item gives
`6 + 25 × 17 = 431`, below the 512-entry guard. Conditional missing/null fields can only reduce
the index; source grouping and attention reference existing entries. No relevance ranking or
prefix truncation is used. Analysis and handoff independently enforce an 8,000,000-byte canonical
artifact bound, including the embedded bundle and duplicated handoff citation index. Overflow
refuses the whole artifact. A maximal raw-byte/25-item fixture exercises these bounds; it is not
a proof that every admitted source string combination fits the local artifact bound.

`validate_glioma_civic_curation_analysis` invokes the existing raw-byte CIViC bundle validator,
then rebuilds all observations, attention reasons, exact-ID groups, citations, coverage, omissions,
support boundaries and the digest. The handoff validator rebuilds the handoff from that checked
analysis. Complete canonical comparison rejects changed projections or upgraded support flags
even if their outer hashes are restamped. Coherently replacing all raw source data can produce a
different internally consistent caller assertion; hashes do not authenticate source origin.

The research handoff retains the full analysis and citation index and stays either
`needs_evidence` or `awaiting_curation_metadata_research_review`. It requires qualified review
without recording human presence, acceptance or scientific approval. It establishes no scientific
support, clinical support, publication independence, patient/cohort/assay denominator or permission
for clinical or laboratory actions.

## Supported downstream work and limits

The output supports reproducible curation-metadata inspection, review of reported flags/revisions
and missing fields, and inspection of repeated native source identifiers. It preserves material
needed for a separate scientific review without supplying that review's conclusions.

Descriptions, therapies, molecular profiles, variants/HGVS, sequences, patient records and source
abstracts are absent from the fixed reader. The composition adds none of them. Other disease
terms, ontology descendants, statuses, later pages, retraction status, publication quality and
release/snapshot equivalence remain unassessed. No source availability, model comprehension or
scientific validity is established by synthetic fixtures or local replay.

## Fixed workflow and optional profile

`glioma_civic_curation_workflow_proposal` and `compile_glioma_civic_curation_workflow`
compose the existing `civic.glioblastoma_evidence_metadata` reader with
`glioma.civic_curation.analyze` and `glioma.civic_curation.review_handoff`. The template selects
the research handoff as its sole output. The native contract requires that output and permits
additional existing step outputs, such as the source bundle; it does not enforce a sole-output
restriction. Native immutable requirements retain all three operations, one call per
operation, the exact 30,000 ms source timeout, the exact task label, and both whole-value links:
source root to `/civic_bundle`, then analysis root to `/analysis`. Edits cannot replace those
roots with subtrees or literal placeholders, bypass the handoff, substitute a different label,
or weaken the retained call/step budget. Additional caller requirements are conjoined.

The optional `civic-curation-review` profile installs these two local operations and the existing
reader. The existing one-step `civic-glioblastoma-evidence` profile and default catalogue remain
unchanged. Overlapping selection reuses the same reader; it does not add another integration.

```python
from prism_sdk import builtin_workflow_operation_catalog
from prism_sdk import with_workflow_capabilities, compile_workflow_capability

profile = "civic-curation-review"
catalog = with_workflow_capabilities(builtin_workflow_operation_catalog(), [profile])
plan = compile_workflow_capability(
    "Inspect fixed CIViC curation metadata", catalog, profile, enabled_profiles=[profile],
)
```

```console
aurora-agent workflow-template --template civic-curation-review --task "Inspect fixed CIViC curation metadata" --out civic-review-plan.json
```

Registration and compilation make no model or source request. Execution still needs its separate
explicit approval. A completed graph retains `awaiting_curation_metadata_research_review` in
CLI and worker review summaries; graph completion never becomes scientific approval. Checkpoint
fixtures pause after the single synthetic caller-fetcher request, reload native bytes, resume
the two local steps and replay completion without refetching. These are synthetic durability
checks, not a live CIViC acquisition, browser observation or actual-model planning measurement.
