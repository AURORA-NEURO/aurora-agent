# Glioma TP53 aggregate mutation-count review

This repository-defined three-stage composition reads the fixed public
[GDC TP53 aggregate query](GDC_GENE_MUTATION_COUNTS.md), replays its native projection, and
produces a citation-preserving scientific review handoff. It has its own schemas; existing
molecular catalogue bundles and their saved plan identities remain unchanged.

```powershell
aurora-agent workflow-template --template mutations --task "Review public TP53 GBM/LGG aggregates" --out tp53-plan.json
aurora-agent workflow-execute --glioma-mutations --plan tp53-plan.json --approve-source-dispatch --checkpoint-store tp53-checkpoint.json
```

Planning and registration make no network or model calls. Execution requires the ordinary
source grant. The task text labels a fixed TP53/TCGA-GBM/TCGA-LGG query; it does not select genes,
projects or scientific hypotheses. In the local application, the operator enables
`--workflow-glioma-mutations`, then the connected catalogue enables **TP53 mutation aggregate
review**. This is a fixed template, separate from local-model planning.

The SDK exposes `register_gdc_gene_mutation_operations`,
`register_glioma_mutation_workflow_operations`, `compile_glioma_mutation_workflow`,
`analyze_glioma_mutation_counts`, `glioma_mutation_review_handoff`, and validators for both
derived artifacts. Registration is opt-in. The compiler retains requirements 0.2 that bind
all three operations, one call per operation, the exact timeout and question, the review output,
and complete source/analysis bundles at `/bundle` and `/analysis`. Additional caller contracts
are conjoined; incompatible source, output or budget requirements refuse before dispatch.

The analysis retains two project rows in fixed GBM/LGG order. An observed zero has a citation
and an exact native count pointer. A missing project or gene bucket has a null count and no
invented citation. `unobserved_project_count` counts projects without an observed requested
gene count; it does not assert that those project buckets were absent. Both partitions retain
native bucket metadata, source clocks, provenance and the replayable native bundle.
Citation identities bind the native observation, exact pointer, source bundle and retrieval
clock, so a changed source observation cannot alias an earlier project-only citation identity.

No gene-specific assay denominator is established: denominator and prevalence remain null.
Counts are mutation-affected cases, not mutation events, molecular profiles, expression values,
survival observations or causal effects. The composition computes no rates, pooled counts,
cross-cohort independence or identifier crosswalk. Cohort overlap and scientific validity remain
unassessed. Public human aggregates never enter the preclinical simulation worker.

Every observed count, including zero, produces `awaiting_scientific_human_review`; no observed
counts produces `needs_evidence`. An incomplete source partition remains explicit in the
analysis even when another partition can be reviewed. Every handoff retains
`scientific_support_established=False`, `human_review_required=True` and
`clinical_actions_authorized=False`.

Both artifact validators recompute all derived fields from the retained native inputs and
compare canonical values. Restamping a modified count, citation, rate or review status cannot
bypass replay. Consistently rewriting all source inputs can still forge unkeyed consistency
digests; they do not authenticate provider truth. Artifacts are bounded to 2,000,000 canonical
UTF-8 bytes. Retained source strings are untrusted content.

The serial executor can pause after the source read, resume the local stages and replay a
completed checkpoint with zero additional requests. A failed source attempt remains a failure;
the fixed one-call budget does not authorize an automatic second attempt. A new reviewed plan
and budget are required for another attempted acquisition. Workflow completion never releases
the scientific review hold.

```powershell
python -W error::ResourceWarning -m pytest python/tests/test_glioma_mutation_workflow.py -q
```

Focused tests exercise observed zero, both missing-bucket states, all-counts-unknown, native
replay, restamped analysis/handoff mutation, exact whole-bundle links, conflicting contracts,
approval, pause/resume and zero-dispatch completed replay. Offline tests establish these
invariants; live source observations are recorded separately.
