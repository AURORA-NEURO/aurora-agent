# Glioma protein/pathway annotation context

This repository-defined composition has no detailed blueprint identifier. It preserves fixed
public human TP53 annotation context from UniProt and Reactome under a scientific review hold.
The research question labels the fixed query and never changes its gene, accession or source scope.

The four-step graph reads the two sources independently, replays their native bundles locally,
then produces a review handoff. Requirements schema 0.2 binds exact source arguments, the original
question, whole source-root data links, citation cap, one call per operation and the review output.
Caller contracts are conjoined. A compiled graph still needs explicit source-dispatch authority.
Checkpoint pause/resume and completed replay reuse the ordinary durable workflow executor.

The analysis retains every returned protein/gene-name candidate and pathway annotation. It
compares the exact UniProt primary-accession string with the Reactome query-accession string;
agreement does not independently establish scientific identifier equivalence. Names and aliases
are not identity matches. Native source disease/inferred flags retain their source meanings and
do not imply glioma activity. No enrichment calculation, measurements or cohort join is performed.

Source partitions preserve missing bundles, missing name fields, observed empty lists and source
coverage. A failure refuses and cannot become an empty success. Citations bind exact native JSON
pointers, values, clocks and digests. The default cap is 64 in native source order; the analysis
retains omitted citation identities, their count/digest and **unknown influence**. Candidate records
and native inputs remain present even when their detailed citation entry is omitted. None of this
establishes evidence sufficiency or negative biology.

All analysis/handoff fields replay from retained native source inputs. Completed execution ends in
`awaiting_scientific_human_review`, or `needs_evidence` when a source is missing. It never authorizes
clinical or laboratory actions. Sources and their self-computed digests are not independent proof.

```console
aurora workflow-template --template protein-pathways --task "Inspect public glioma TP53 annotation context" --out plan.json
aurora workflow-execute --protein-pathways --plan plan.json --approve-source-dispatch --checkpoint-store checkpoint.json --outputs-file outputs.json
```

The CLI flag and worker operator flag `--protein-pathways` install two real source reader paths
and two local operations. Local operations are not external integrations. The eight-reader default
catalogue remains unchanged. No model call occurs for the explicit template.
