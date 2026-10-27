# Glioma TP53 source-reference review

`python/prism_sdk/glioma_gene_reference_workflow.py` composes the reviewed
[Ensembl reference reader](ENSEMBL_GENE_REFERENCES.md) and existing NCBI Gene TP53 reader into a
local, replayable comparison and scientific review hold. This is a repository-defined extension
without an available detailed blueprint identifier. It supplies no independently validated
identifier-equivalence proof and does not change the existing molecular workflow or its
`gene_crosswalk.state="not_supplied"` field.

The fixed four-stage graph is:

1. `ensembl.gene_references`: human/core TP53 gene metadata and every admitted EntrezGene
   cross-reference candidate, with `timeout_ms=30000`.
2. `ncbi_gene.summary`: exactly `gene_symbols=["TP53"]`, with `timeout_ms=30000`.
3. `glioma.gene_references.analyze`: native source replay, exact accession comparison and citations.
4. `glioma.gene_references.review_handoff`: a retained scientific human-review hold.

The first two stages request public annotation metadata. The last two stages are local read-only
operations. Registering the local stages does not register, contact or authorize either source.
The question labels this fixed scope; it is not interpreted to select another gene, disease,
measurement, variant or action.

```python
from prism_sdk.workflow_integrations import builtin_workflow_operation_catalog
from prism_sdk.reviewed_ensembl_gene_retrieval import register_ensembl_gene_operations
from prism_sdk.glioma_gene_reference_workflow import (
    register_glioma_gene_reference_workflow_operations,
    compile_glioma_gene_reference_workflow,
    validate_glioma_gene_reference_handoff,
)
from prism_sdk.workflow_builder import execute_workflow_plan

catalog = register_ensembl_gene_operations(builtin_workflow_operation_catalog())
register_glioma_gene_reference_workflow_operations(catalog)
plan = compile_glioma_gene_reference_workflow(
    "Review public glioma TP53 source gene references", catalog,
)
# Review plan.to_dict() before separately granting source dispatch.
run = execute_workflow_plan(plan, catalog, approved=True)
if run.status == "completed":
    review = validate_glioma_gene_reference_handoff(run.outputs["review"])
```

This example makes source requests only when the execution grant is supplied. Model calls,
credentials, account onboarding, individual records and clinical or laboratory actions are absent.
The existing builtin catalogue is unchanged; Ensembl and the local stages require explicit
registration. Two local operation descriptors do not represent two additional external services.

## Source assertions and missingness

`analyze_glioma_gene_references(question=..., ensembl_bundle=..., ncbi_bundle=...)` first calls
the Ensembl public bundle validator and the existing molecular layer's native NCBI bundle replay.
The NCBI input must contain exactly the reviewed human TP53 row with its fixed Entrez accession.
An arbitrary gene bundle, a larger multi-gene bundle, edited source counts or provenance, an alias
substitution, or a malformed source artifact is refused.

The comparison uses exact retained accession strings. It performs no case folding, alias matching,
numeric conversion or preferred-candidate selection. Its states distinguish:

| Observation | Comparison state |
| --- | --- |
| Either native bundle was not supplied | `not_comparable_missing_source` |
| Successful Ensembl response contains no reference candidates | `observed_no_reference_candidates` |
| One candidate agrees with the NCBI accession | `single_candidate_id_value_agreement` |
| One candidate disagrees | `single_candidate_id_value_mismatch` |
| Several candidates include the NCBI accession | `multiple_candidates_with_matching_id` |
| Several candidates contain no matching accession | `multiple_candidates_without_matching_id` |

All admitted candidates remain visible. Matching and mismatching counts are measured only when
both sources were observed. A missing source has null counts, coverage and omissions; it never
becomes an observed zero. Direct `None` is the sole representation of a bundle not supplied.
Failed-source objects, false values and malformed mappings are errors. In the executable graph a
source failure halts before analysis and cannot manufacture a missing-source artifact.

An empty successful array receives its own source-value citation. It does not establish biological
absence or exhaustive cross-database coverage. Ensembl's unreported total remains null and its
reference exhaustiveness remains unassessed. The NCBI partition retains the eleven unrequested
reviewed catalogue symbols, source limitations and the fact that it supplies no cross-database
reference coverage. These omissions do not grant access to an omitted evidence plane.

Agreement means the two retained source annotations used the same accession value for this fixed
query. Scientific identifier equivalence and source independence remain unestablished. No mutation,
expression, association, assay, cohort, prevalence or outcome values are joined. Existing molecular
and mutation artifacts retain their own source boundaries.

## Citations, replay and review

The analysis retains both native bundles, source partitions, every candidate comparison and every
citation. Each citation includes the exact source value, its digest, native JSON pointer, source
URI, retrieval clock and source-bundle digest. The small fixed scope admits all citations; the
maximum 64-candidate case retains 66 citations, with no local citation truncation. Source-level
omissions remain explicit separately from the zero local citation omission count.

`validate_glioma_gene_reference_analysis()` reconstructs all derived fields from retained sources.
`validate_glioma_gene_reference_handoff()` then reconstructs the entire handoff from that analysis.
Both return defensive dictionaries and make no source or model requests. Restamping an outer
digest after changing comparison counts, citations, omissions or review authority cannot bypass
this replay. Consistently rewritten unkeyed source artifacts are not provider-authenticated;
internal consistency is not independent source truth.

The analysis and handoff each have a 2,000,000-byte canonical JSON bound and use the native source
normalizers' bounded fields. A review with both observed sources is
`awaiting_scientific_human_review`, including empty, ambiguous and mismatching cases. A missing
source yields `needs_evidence`. Every result keeps `human_review_required=True`,
`scientific_support_established=False`, `scientific_identifier_equivalence="unestablished"` and
`clinical_actions_authorized=False`.

## Bound workflow contract and recovery

`compile_glioma_gene_reference_workflow(task, catalog, requirements=None)` returns a schema-0.2
plan with schema-0.2 requirements. The contract fixes both complete source argument objects,
the question label, all four operations, a four-step maximum, one call per operation, the review
output, and three exact root-to-argument value links. The analysis receives the whole source
bundles at `/ensembl_bundle` and `/ncbi_bundle`; the review receives the whole analysis at
`/analysis`. A gene subobject or comparison summary cannot replace a native replay input.

Caller requirements are conjoined with these invariants. A tighter incompatible step budget,
forbidden source or conflicting selector refuses compilation before dispatch. No caller or
generated proposal can relax the native requirements by replacing them.

The standard executor can pause after the two source stages and resume the two local stages from
retained results. Completed source calls are not repeated. Replaying a completed checkpoint makes
zero additional source requests; a failed stage remains a failure and requires explicit retry
authority within its retained call budget. This fixed one-call contract does not grant a second
attempt after an attempted source failure; that requires a newly reviewed plan and budget.
A human review hold is the artifact's scientific status and is not lifted by workflow
completion or checkpoint replay.

Focused checks use source parsers with offline callback fixtures, not live service counts:

```powershell
$env:PYTHONPATH = 'python'
python -W error::ResourceWarning -m pytest python/tests/test_glioma_gene_reference_workflow.py -q
```

Tests cover native replay, exact comparison, multiple and missing candidates, failed-source refusal,
all citation pointers and values, retained review holds, restamped artifact mutation, caller-contract
conflicts, exact whole-bundle data links, source approval, pause/resume and zero-request replay.
