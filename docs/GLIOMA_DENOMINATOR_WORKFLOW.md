# Glioma TP53 denominator admissibility assessment

`glioma_denominator_workflow.py` compares retained, source-native TP53 mutation-affected case
aggregates with separately acquired project SSM-data availability counts. It answers which
requested count observations exist, which necessary numerical check can be evaluated, and
which comparability evidence remains missing. It supplies no prevalence estimate.

This repository-defined application extension is under-specified by the available blueprint;
no source-specific denominator or scientific capability claim is attributed to a blueprint
module. It follows the research boundary and keeps unavailable measurements distinct from zero.

The existing [GDC TP53 numerator reader](GDC_GENE_MUTATION_COUNTS.md) fixes the Ensembl ID
`ENSG00000141510`; the separate [SSM coverage reader](GDC_SSM_COVERAGE.md) retrieves counts of
cases with associated SSM data. The primary
[GDC analysis documentation](https://docs.gdc.cancer.gov/API/Users_Guide/Data_Analysis/#analysis-endpoints)
defines these two aggregate queries with distinct semantics. Their fixed reviewed contracts
do not supply a gene-specific eligible/callable population, assay design, callable regions,
calling threshold, exclusion policy, reference build or release identity adequate to establish
comparability. The implementation does not assert that such evidence is unavailable everywhere.

```python
from prism_sdk.glioma_denominator_workflow import (
    assess_glioma_denominator,
    glioma_denominator_review_handoff,
    validate_glioma_denominator_analysis,
)

assessment = assess_glioma_denominator(
    question="Review TP53 denominator admissibility for TCGA-GBM/LGG.",
    mutation_bundle=retained_tp53_bundle,
    coverage_bundle=retained_ssm_coverage_bundle,
)
validated = validate_glioma_denominator_analysis(assessment)
handoff = glioma_denominator_review_handoff(validated)
```

Both input bundles are independently replayed through their native validators before local
analysis. TCGA-GBM and TCGA-LGG remain separate rows. Every row retains source numerator,
availability state/value and native observation; neither source's project count nor nested
document count is re-labelled as an assay denominator. Missing acquisitions, missing requested
project/summary/count fields, explicit observed zero and failed acquisition stay distinct.
An acquisition failure raises; the analyst does not create a successful empty bundle.

When both counts are observed, the output compares their integer ordering. A candidate count
below the numerator records `candidate_count_less_than_numerator` and an explicit failed
necessary-order blocker. Otherwise `necessary_count_order_holds` reports only an inequality:
**conditional on a comparable denominator, its count could not be below its numerator**.
Passing that arithmetic check establishes no comparable denominator, callability or scientific
population relationship. Missing either count yields `not_evaluable_missing_count`.
Zero versus zero therefore remains two observed zeros with an unavailable denominator;
there is no division or calculated zero prevalence.

All rows retain `gene_specific_denominator=null`, `prevalence=null`,
`prevalence_calculated=false`, `scientific_comparability=unestablished`, and an explicit blocker
list. The list includes gene-specific eligibility, cohort membership/overlap, same release,
reference build, assay callability/exclusions and non-observation ascertainment. Numerical or
missing-count blockers are added from the actual retained values. Exact project label equality
compares source strings only. Separate acquisition clocks are visible; equal clocks do not
establish the same source release or common population.

Every available source row has a content-bound citation, including missing-field observations.
The numerator pointer authority is the existing validated source projection: original raw
source buckets omitted by that legacy reader cannot be reconstructed. Coverage citations
resolve into the exact retained native response payload, or the derived missing-project row
when no native requested bucket exists. Citations retain their artifact pointer, original source
pointer where available, URI, clock, value, bundle/value digests and explicit pointer authority.
No citation selection cap discards either project's observations.

Analysis/handoff validators rebuild every derived field from both retained bundles. A changed
count comparison, rate, conclusion, source clock, citation, review state or clinical permission
fails replay even if its outer digest is recomputed. Retained consistency does not authenticate
source truth, independently establish scientific validity, or replace qualified human review.

`register_glioma_denominator_workflow_operations(catalog)` installs the two local operations
`glioma.denominator.assess` and `glioma.denominator.review_handoff`. It performs no acquisition.
With the two source registrations, `compile_glioma_denominator_workflow(task, catalog)` creates:

1. One independent fixed `gdc.gene_mutation_counts` acquisition.
2. One independent fixed `gdc.ssm_project_coverage` acquisition.
3. Native root-linked local denominator assessment using the exact question.
4. A root-linked scientific review handoff as the only output.

The immutable requirements retain exact 30,000 ms source arguments, one call per operation,
all four operations, native root data links, exact question and review output. Caller-supplied
requirements are conjoined. Template creation is model-free and does not claim automatic
interpretation of arbitrary natural-language scientific requests. Source dispatch requires
workflow approval; normal durable checkpoints permit pausing after each source and completing
local stages/replaying the completed graph without refetch. The final state remains a
scientific human-review hold, with no clinical or laboratory authorization.

Focused offline tests cover native pointer resolution, zero/missing observations, failed
necessary ordering, source clock separation, restamped tampering, exact-contract substitutions,
approval holds and checkpoint resume/replay:

```powershell
$env:PYTHONPATH = (Join-Path (Get-Location) 'python')
python -m pytest python/tests/test_glioma_denominator_workflow.py -q
```
