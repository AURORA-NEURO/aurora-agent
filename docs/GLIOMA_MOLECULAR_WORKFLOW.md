# Glioma molecular metadata workflow

`python/prism_sdk/glioma_molecular_workflow.py` composes four existing reviewed public source
adapters into executable, deterministic metadata coverage and scientific review operations. This
is a repository-defined deployment contract. It inventories catalogue availability, gene identity,
and source-reported association annotations. It does not establish molecular observations,
mechanisms, evidence validity, or clinical authority.

```text
cbioportal.studies -----------+
ncbi_gene.summary -----------+
open_targets.associations ---+--> glioma.molecular.coverage --> glioma.molecular.review_handoff
gwas_catalog.associations ---+     (GWAS optional)
```

Register the local operations explicitly. The built-in source catalogue remains unchanged.
Creating the catalogue or proposal makes no network or model request.

```powershell
aurora-agent workflow-template --template molecular --task "Review public glioma molecular metadata" --association-page-size 1 --gwas-page-size 1 --gwas-max-pages 1 --out molecular-plan.json
aurora-agent workflow-execute --molecular-glioma --plan molecular-plan.json --approve-source-dispatch --checkpoint-store molecular-run.json --outputs-file molecular-review.json
```

The molecular template registers its local operations automatically; repeat `--molecular-glioma`
when executing or listing the same catalogue. `--no-gwas` selects the five-stage variant.
`--grounded-glioma` and evidence-template limit flags are refused for this template, rather than
silently ignored. This path uses no local model. Metadata stdout reports the final review status
separately from graph completion; public content appears only in explicit output/checkpoint files.

```python
from prism_sdk.glioma_molecular_workflow import (
    glioma_molecular_workflow_proposal,
    register_glioma_molecular_workflow_operations,
)
from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan
from prism_sdk.workflow_integrations import builtin_workflow_operation_catalog

question = "Inventory public glioma molecular metadata and unresolved evidence gaps"
catalog = register_glioma_molecular_workflow_operations(builtin_workflow_operation_catalog())
proposal = glioma_molecular_workflow_proposal(
    question,
    include_gwas=True,
    association_page_size=10,
    gwas_page_size=10,
    gwas_max_pages=1,
    max_citations=128,
)
plan = compile_workflow_plan(proposal, catalog, task=question)
# Source dispatch requires the caller's explicit approval:
run = execute_workflow_plan(plan, catalog, approved=True)
```

The question is retained and digest-bound for review. Use public research text only: it is not a
patient-data intake form. The question does not refine any source query; every output declares
`question_used_as_source_query=false`. Source steps use the existing fixed selectors:

| Operation | Template selectors | Preserved metadata and derived coverage |
| --- | --- | --- |
| `cbioportal.studies` | `gbm_tcga`, `lgg_tcga` | Study and molecular-profile catalogues, original study/profile IDs, source-reported sample counts, profile counts by alteration type and datatype, missing descriptions and unknown flags |
| `ncbi_gene.summary` | Fixed 12-symbol reviewed catalogue: IDH1, IDH2, MGMT, EGFR, TERT, TP53, ATRX, NF1, PTEN, CDKN2A, PDGFRA, BRAF | Entrez Gene IDs, source descriptions, aliases, taxonomy, chromosome/map-location missingness; identity only |
| `open_targets.associations` | `gbm`, `lgg`; first ranked page, size 1–50 | Exact disease/Ensembl IDs, source rank and association score, returned/omitted counts; ranking is not confidence or causality |
| `gwas_catalog.associations` | `gbm`, `glioma`; direct ontology matches only, size 1–50 and 1–5 pages | Association/study/publication IDs, original p-value state, mapped-gene and location annotations, page coverage and omitted counts |

No raw response is substituted for a failed source, and no successful empty result is invented.
The general executor retains native source receipts and completed values in its checkpoint; a
resumed run reuses accepted source results. Source errors stop dependent coverage and review.
Explicit retry remains separate from source approval. The local coverage API can also receive
missing source bundles; those partitions receive `not_supplied`, null counts, and `needs_evidence`.
A successfully retrieved empty association lane keeps zero counts and `retrieved_empty`. Neither
state means negative biological evidence.

`analyze_glioma_molecular_coverage(question=..., requested_sources=..., sources=...,
max_citations=128)` accepts source entries shaped as `{"operation_id": "...", "bundle": {...}}`.
It checks the exact native schemas, boundaries, timestamps, source-set and bundle digests, and
replays each source owner's normalizers. Counts, identifiers, completeness, page bounds, and
provenance must match that replay. This reuses the reviewed source validation rather than
coercing bundles into `RealGliomaBundle`, whose broader snapshot contract these bundles do not
satisfy. Self-computed digests establish internal consistency; they do not authenticate a source.

Each retained record has an exact source URL, native identifier, record digest, and stable citation
ID. The citation index interleaves source planes and exports at most 128 entries. If that index
omits records, their exact citation IDs, count, and digest remain explicit; the records themselves
remain in the source partitions. This local index bound is separate from omitted upstream pages.
Combined source inputs are bounded to 750,000 canonical JSON bytes, and local artifacts to
2,000,000 bytes. Oversized inputs fail instead of silently truncating source metadata.

`glioma_molecular_review_handoff(coverage)` verifies the coverage digest and regenerates the entire
coverage artifact from retained native inputs. Changes to summaries, citations, missingness, or
authority flags fail replay, even if an outer digest was recomputed. The handoff retains typed
coverage, exact source metadata, all omission declarations, and a review checklist. It always
requires human review and declares scientific support and clinical authority false.

The source planes stay separate. Entrez identifiers, Ensembl identifiers, symbols, aliases, and GWAS
mapped-gene annotations are not joined; the Entrez–Ensembl crosswalk remains `not_supplied`.
Cross-source cohort overlap and ancestry applicability remain unassessed. Counts are not pooled.
Profile catalogue availability is not evidence that a mutation, expression change, methylation
state, or outcome was measured. GWAS mapped genes are source annotations, and zero p-values retain
the reviewed source's precision limitation. No causal statistics, treatment recommendations,
diagnosis, patient-level retrieval, laboratory protocol, or preclinical-engine routing is supplied.

On 2026-10-26, a separately approved bounded live check used one association per Open Targets lane
and one GWAS page of one association per lane. All four source steps and both local operations
completed in nine source requests: two cBioPortal studies with 28 profile catalogue rows, 12 Gene
identity rows, two Open Targets association rows, and two GWAS association rows. The review retained
46 metadata citations and ended `awaiting_scientific_human_review`. This verifies that executable
composition and native metadata replay worked for that bounded observation; upstream association
pages remained omitted and scientific support remained unestablished. Europe PMC was checked
separately and is not part of this molecular template.

Offline tests use real reviewed adapters with injected fixture transports, including exact schema
replay, null/zero/unknown states, empty association lanes, explicit citation omissions, restamped
tampering, source failures, approval holds, disk checkpoints, and resumption without duplicate reads:

```powershell
$env:PYTHONPATH = (Join-Path (Get-Location) 'python')
python -m unittest discover -s python/tests -p test_glioma_molecular_workflow.py -v
```
