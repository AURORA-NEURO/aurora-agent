# Glioma public evidence workflow

`python/prism_sdk/glioma_workflow.py` composes the general workflow builder with existing reviewed
public source adapters. This is a repository-defined deployment contract, not a newly implemented
BioPRISM blueprint feature. The detailed source blueprint is absent from this checkout, as the
glioma program plan records.

The workflow retrieves PubMed glioma citation metadata and ClinicalTrials.gov glioma study metadata,
optionally retrieves the aggregate summaries of TCGA-GBM and TCGA-LGG projects from GDC, then
reconciles the separate source partitions into a deterministic review handoff. Network retrieval
uses the underlying reviewed adapters and their source-dispatch approval gate. Building a proposal
and running the two local reconciliation operations do not contact a source or a model.

```python
from prism_sdk.glioma_workflow import (
    glioma_evidence_workflow_proposal,
    register_glioma_workflow_operations,
)
from prism_sdk.workflow_integrations import builtin_workflow_operation_catalog
from prism_sdk.workflow_builder import compile_workflow_plan

catalog = register_glioma_workflow_operations(builtin_workflow_operation_catalog())
question = "Review the public glioma evidence landscape and identify missing molecular evidence"
proposal = glioma_evidence_workflow_proposal(
    question,
    include_tcga=True,
    literature_limit=10,
    trial_page_size=25,
    trial_max_pages=2,
)
plan = compile_workflow_plan(proposal, catalog, task=question)
```

The CLI exposes the same catalogue, builder, template, and durable executor:

```powershell
aurora-agent workflow-catalogue
aurora-agent workflow-template --task "Review glioma evidence and missing molecular metadata" --out plan.json
aurora-agent workflow-execute --plan plan.json --approve-source-dispatch --checkpoint-store run.json
# Resume a paused run, retaining accepted source results:
aurora-agent workflow-execute --plan plan.json --approve-source-dispatch --checkpoint-store run.json --resume
# Retry a recorded source failure only after deciding to repeat that read:
aurora-agent workflow-execute --plan plan.json --approve-source-dispatch --checkpoint-store run.json --resume --retry-failed
```

For a workflow proposed by a running local Ollama model, use:

```powershell
aurora-agent workflow-build --task "Build a glioma public evidence review workflow" --model YOUR_LOCAL_MODEL --approve-provider-call --out plan.json
```

Model output remains an untrusted proposal and must pass the exact operation catalogue, schema,
dependency, reference, scope, and resource checks. Building a plan does not authorize source
dispatch. The explicit checkpoint file contains public source values so completed source steps can
resume without refetching them; keep that file only where public research material may be stored.

The proposal has this dependency graph:

```text
pubmed.search -----------+
clinical_trials.search --+--> glioma.evidence.reconcile --> glioma.evidence.review_handoff
gdc.projects (optional) -+
```

Each source stage returns its original adapter bundle. The reconciliation arguments refer to those
step values through dependency-bound references such as `{"$ref": "literature#"}`. Source retrieval
errors remain workflow execution failures; the template does not substitute a fixture or fabricate
a successful empty response. A caller can separately invoke reconciliation with missing source
results; those sources receive `not_supplied`, a null record count, and `needs_evidence`.

The local operations are:

| Operation | Input | Output |
| --- | --- | --- |
| `glioma.evidence.reconcile` | Question, exact public-research scope, requested source IDs, and source bundles | Separate source partitions, original record IDs and citation URLs, record/bundle digests, missing metadata, bounded-query omissions, explicit unassessed dimensions |
| `glioma.evidence.review_handoff` | Exact reconciliation artifact and its digest | Citation-preserving handoff and a review checklist; always requires qualified human review |

A present source with zero records remains different from a source that was not supplied. Unknown
GDC case counts remain null; a measured zero file count remains zero. Missing publication day,
DOI, abstract, trial enrollment metadata, and project state are not imputed. PubMed abstract text
remains in the original source step value; the handoff records abstract availability and the record
digest instead of repeating the full text. Original source timestamps, stated completeness,
limitations, trial truncation, and omitted record counts travel with their source partition.

The local handoff validates its digest, question binding, source states, record counts, citation
identities, citation index, and research-boundary declarations. A self-computed digest is integrity
evidence, not source authentication or independent validation. Caller-supplied metadata and retrieved
text are untrusted source material and must never become workflow instructions.

The free-text question is retained for review and bound by a digest. Source adapters currently use
their fixed, reviewed glioma query lanes; the question does not become an arbitrary source query.
This limitation is explicit in every reconciliation and handoff. The local operations perform no
scientific synthesis: citation counts do not establish support, registry activity does not establish
benefit, and GDC project counts do not establish patient-level molecular observations. Study quality,
independence, overlap, confounding, contradictions, and negative findings require source review.

The handoff names unassessed glioma research dimensions, including IDH, 1p/19q, MGMT, H3, TERT,
EGFR, CDKN2A/B, heterogeneity, single-cell/spatial data, imaging, and longitudinal evidence. These
are review gaps, not inferred molecular results, diagnoses, or treatment recommendations. The
workflow does not query patient-level cases, retrieve controlled-access material, match trial
eligibility, dispatch laboratory work, or admit public human metadata into the Rust preclinical
engine. A later scientific synthesis stage must cite source material and pass its own claim audit.

Run the dependency-free invariant tests from the repository root:

```powershell
$env:PYTHONPATH = (Join-Path (Get-Location) 'python')
python -m unittest discover -s python/tests -p test_glioma_workflow.py -v
```
