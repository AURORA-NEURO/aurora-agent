# Three-source glioma gene identity workflow

`python/prism_sdk/glioma_gene_identity_workflow.py` composes the reviewed HGNC, Ensembl and
NCBI Gene readers for fixed **TP53** source metadata. It deepens the existing Ensembl/NCBI
reference comparison by retaining HGNC nomenclature, status, aliases and reference assertions.
This is a repository-defined extension; no available blueprint module specifies this source
composition. The question labels the review task and does not change selectors or infer
scientific requirements.

The model-free template has five fixed steps:

1. `hgnc.gene_references`: one fixed public symbol-fetch GET.
2. `ensembl.gene_references`: fixed human TP53 lookup and checked gene-level Entrez references.
3. `ncbi_gene.summary`: fixed TP53 public Gene summary.
4. `glioma.gene_identity.analyze`: local native-source comparison.
5. `glioma.gene_identity.review_handoff`: local handoff requiring scientific human review.

These are three independently registered source operations, normally **four HTTP GETs**
because Ensembl uses two. They are not four providers, four independent scientific sources,
or a live availability measurement. Source approval is required before dispatch. Analysis,
handoff validation and completed-checkpoint replay make no source or model calls.

```python
from prism_sdk.glioma_gene_identity_workflow import (
    analyze_glioma_gene_identity,
    glioma_gene_identity_review_handoff,
    validate_glioma_gene_identity_analysis,
)

analysis = analyze_glioma_gene_identity(
    question="Review the fixed TP53 source identifiers before a glioma evidence join.",
    hgnc_bundle=hgnc_bundle,
    ensembl_bundle=ensembl_bundle,
    ncbi_bundle=ncbi_bundle,
    max_citations=128,
)
checked = validate_glioma_gene_identity_analysis(analysis)
handoff = glioma_gene_identity_review_handoff(checked)
```

`register_glioma_gene_identity_workflow_operations(catalog)` adds the two local operations to
a standard `WorkflowOperationCatalog`. Source registration remains explicit and separate.
`glioma_gene_identity_workflow_proposal(question)` returns the fixed proposal;
`compile_glioma_gene_identity_workflow(task, catalog, requirements=None)` compiles it under
immutable exact arguments, native root links, five-step/call limits, citation budget and
review-only output requirements. Additional caller requirements are conjoined. Changing the
question, gene selector, native pointer, source arguments or output cannot evade that contract.

## What is compared

Every HGNC document and its field observations remain visible, including all aliases and
previous names. The comparison uses exact case-sensitive source identifier strings only:
HGNC Ensembl ID against the returned Ensembl gene ID; HGNC Entrez ID against the NCBI Gene
ID; and HGNC Entrez ID against **every** Ensembl Entrez candidate. No candidate is preferred.
There is no alias matching, case folding, numeric coercion or silently resolved equivalence.
Existing Ensembl/NCBI candidate comparisons, limitations and omission states remain intact.

The [HGNC REST contract](https://www.genenames.org/help/rest/),
[Ensembl symbol lookup](https://rest.ensembl.org/documentation/info/symbol_lookup),
[Ensembl gene-reference route](https://rest.ensembl.org/documentation/info/xref_id) and
[NCBI ESummary documentation](https://www.ncbi.nlm.nih.gov/books/NBK25499/#chapter4.ESummary)
describe separate source assertions. Their matching strings establish metadata concordance
only. Shared curation may exist, so source independence is unassessed; release equivalence
and scientific identifier equivalence are unestablished. Acquisition clocks remain separate.

Analysis status preserves the reason a review cannot proceed:

| State | Meaning |
| --- | --- |
| `needs_evidence` | A source acquisition or required comparison field is absent/unobserved. |
| `needs_identity_resolution` | HGNC source selection is non-ready, including absent, withdrawn, ambiguous or partial source records. |
| `metadata_discrepancy_for_review` | Admitted identifier strings disagree. |
| `source_assertions_ready_for_review` | Admitted comparison strings agree and the required source record is ready for review. |

The final state in this table is not scientific approval. The handoff preserves missing
evidence/identity-resolution states; other cases become `awaiting_scientific_human_review`.
Every handoff keeps `scientific_support_established=False`, `human_review_required=True` and
`clinical_actions_authorized=False`. Missing, null, empty, withdrawn, ambiguous and unreturned
evidence never becomes zero biological effect or an approved identity.

## Retention, citations and replay

`source_inputs` retains all three validated source bundles. The existing Ensembl/NCBI
retention contracts remain unchanged; HGNC additionally retains exact response bytes as
UTF-8 text. Source partitions expose acquisition state, digest, identity state and omissions.
Every HGNC record, observed field and empty successful document array has a citation into
its retained native payload; missing fields have no fabricated pointer or value.

The citation budget is an integer from 1 through 1,024, default 128. Selection is an explicit
prefix in existing Ensembl/NCBI citation order followed by HGNC document/field order, not
relevance ranking. All native records and candidate comparisons remain retained. Omitted
citation IDs, count and digest are explicit and their influence is `unknown`; a shorter
index is never called sufficient. A source's unknown completeness remains unknown even when
no detailed citation is omitted.

`validate_glioma_gene_identity_analysis()` replays each source using its native validator and
reconstructs every comparison, status, citation and digest. Handoff validation replays that
analysis again. Restamping altered derived output fails validation. These checks establish
internal artifact consistency; they do not authenticate a source or independently validate
scientific claims. Analysis schema is `aurora-glioma-gene-identity-analysis/0.1`, review schema
is `aurora-glioma-gene-identity-review/0.1` and scope is
`public_glioma_TP53_three_source_metadata_concordance`.

Offline fixtures exercise all missingness/identity states, conflicting candidates, exact
native citation resolution, citation omissions, compiler substitution refusal and the actual
five-step SDK graph with approval hold/local resume/zero-dispatch completed replay. Fixtures
do not establish live connectivity. This workflow supplies a bounded metadata review for a
glioma research context; it establishes no disease association, molecular classification,
assay denominator, cohort compatibility or clinical recommendation, and joins no measurements.
