# Deterministic glioma workflow interpretation

`glioma_workflow_intent.py` interprets a small, explicit request grammar without a model. Accepted
requests compile through the existing reviewed source -> reconciliation -> review template. This
avoids asking a small model to invent dependency references. It does not interpret unrestricted
natural language or establish that the requested research question can be answered scientifically.

Every nonempty clause must match a documented form. Unknown text, unsupported filters and
conflicts produce `needs_clarification`, with spans copied from the original request. Compilation
refuses that state. It never discards a restrictive clause and executes a broader search.

## SDK use

The CLI exposes the same model-free interpretation and compilation:

```powershell
aurora-agent workflow-interpret --task "Review glioma evidence using PubMed only; exclude trials and TCGA."
aurora-agent workflow-build --planner glioma-intent --task "Review glioma evidence using PubMed and trials; PubMed limit 2; trials page size 3; trials max pages 1." --out plan.json
aurora-agent workflow-execute --plan plan.json --approve-source-dispatch --checkpoint-store run.json --outputs-file public-review.json
```

`--planner local-model` remains the default for `workflow-build` and `workflow-run`. The
`glioma-intent` mode needs no model or provider approval unless `--grounded-glioma` enables a
later synthesis stage. `workflow-run` can compile and execute the typed plan in one command;
source approval is still independent. `--requirements FILE` conjoins additional caller predicates
with the derived contract. Conflicting predicates or source exclusions fail before dispatch.
The exact `--operation` allowlist and `--max-steps` bound still apply. A request requiring
clarification writes no plan and makes no provider, discovery or source call.

```python
from prism_sdk import (
    builtin_workflow_operation_catalog,
    register_glioma_workflow_operations,
)
from prism_sdk.glioma_workflow_intent import (
    compile_glioma_workflow_request,
    interpret_glioma_workflow_request,
)

task = (
    "Review glioma evidence using PubMed and trials; "
    "PubMed limit 2; trials page size 3; trials max pages 1."
)
catalog = register_glioma_workflow_operations(builtin_workflow_operation_catalog())
intent = interpret_glioma_workflow_request(task)
assert intent.status == "ready"
plan = compile_glioma_workflow_request(task, catalog, intent=intent)
assert plan.to_dict()["outputs"] == ["review"]
```

Interpretation, registration and compilation make no provider or source calls. A supplied receipt
must match fresh interpretation of the exact original task; changing a slot and recomputing its
digest does not make it acceptable. A valid plan still requires separate source approval when
executed through the [durable workflow executor](NLP_WORKFLOW_BUILDER.md).

`GliomaWorkflowIntent` provides `to_dict()`, `status` and `intent_digest`. The receipt uses schema
`aurora-glioma-workflow-intent/0.1` and contains the original `task`, exact `task_digest`, public
research `scope`, `source_operations`, `literature_limit`, `trial_page_size`, `trial_max_pages`,
`explicit_defaults`, and `unresolved_spans`. Each unresolved span contains `start`, `end`, `text`
and `reason`; offsets are Python string indices and select the original text exactly. It also
records `source_dispatch=false` and `provider_calls=0`. Unlike the general digest-only intake
projection, this review receipt retains task text. Supply public research requests and store or
display receipts accordingly.

## Accepted grammar

Clauses are separated by semicolons, newlines or sentence-ending periods. Case and surrounding
whitespace are ignored for matching. Commas are accepted inside source lists, rather than as
arbitrary clause separators. The dot in `ClinicalTrials.gov` remains part of the source alias.
Original spelling and punctuation remain in the task and its digest.

Begin with a bounded glioma research request such as `Review public glioma evidence`,
`Retrieve glioma metadata` or `Build glioma evidence workflow`. The supported initial verbs are
`review`, `retrieve`, `find`, `collect`, `gather`, `research`, `compile`, `build` and `prepare`.
The optional nouns are `public`, `research`, `evidence`, `metadata`, `sources`, `workflow`,
`review` and `handoff`. This is an English clause grammar; other wording may need clarification.

| Selection | Examples |
| --- | --- |
| Explicit source list | `Review glioma evidence using PubMed and trials` |
| Source-only request | `Review PubMed only for glioma` |
| Additional source clause | `use PubMed and TCGA`; `include trials`; `sources: PubMed, trials, and TCGA` |
| Source exclusion | `Review glioma evidence without TCGA`; `exclude trials and TCGA`; `do not use PubMed` |
| Explicit exclusivity | `use PubMed only`; a later `include trials` is a conflict |

Aliases are `PubMed`; `trials`, `clinical trials`, `ClinicalTrials.gov` or `trial registry`;
and `TCGA`, `GDC`, `TCGA projects` or `GDC projects`. An explicit positive source list selects
those sources. With no positive list, the visible default is PubMed + trials + TCGA, minus any
explicit exclusions. An empty selection, duplicate aliases for the same source, or inclusion and
exclusion of the same source needs clarification. Source order is canonical: PubMed, trials, TCGA.

| Bound | Examples | Allowed range |
| --- | --- | --- |
| PubMed records | `PubMed limit 2`; `limit PubMed to two records`; `two PubMed papers` | 1–50 |
| Trial page size | `trials page size 3` | 1–100 |
| Trial pages | `trials max pages 1`; `trials page limit two` | 1–5 |
| Trial record limit | `trials limit 3 records`; `three trial records` | 1–100, with one page |

Integers and the words `one` through `ten` are supported. A trial record-limit clause sets both
page size and a one-page maximum; it does not silently multiply the requested count by the default
page count. Repeated unequal bounds conflict. A limit for an excluded source needs clarification.
The interpreter refuses invalid bounds rather than clipping them. Limits bound retrieval; they
do not promise that the source contains that many matching records.

Explicit review clauses such as `reconcile and prepare a human review handoff`, `reconcile` and
`prepare a human review handoff` are supported. The boundary clauses `public metadata only`,
`no patient data`, `no clinical claims`, `no clinical recommendations` and `human review required`
are also recognized. Other negations remain unresolved instead of being ignored.

## Visible defaults and fixed compilation

Active defaults are recorded as `{field, value}` entries. PubMed defaults to 10 records. Trials
default to page size 25 and at most two pages. The base output is a human-review handoff, and
each operation has a one-attempt dispatch budget. Limits for unused sources are not applied.

The compiler registers nothing implicitly. Every selected source and local operation must already
exist in the supplied catalogue. It creates a [fixed requirements contract](WORKFLOW_REQUIREMENTS.md)
covering the generated operations, excluded sources, normalized literal arguments including
catalogue defaults, selected-source identities, final output, value-flow links and step count.
Each operation is capped at one persisted dispatch attempt, including failed or uncertain attempts.
That budget does not reset on resume; trial page bounds separately constrain requests inside a
single trial operation.

The underlying `glioma_evidence_workflow_proposal` now also accepts an explicit nonempty
`source_operations` subset. Omitting that argument preserves the existing default graph and
options. A subset containing TCGA conflicts with `include_tcga=False`.

For a catalogue already containing the reviewed local synthesis and audit operations,
`compile_glioma_workflow_request(task, catalog, grounded=True)` replaces the base review stage
with reconciliation -> synthesis -> exact draft audit, and selects the audit as the final output.
The derived contract requires both operations and their real argument-reference links.
`grounded` must be an actual boolean. Compilation invokes no model and grants no model authority;
runtime registration and provider approval remain separate. The receipt's `base_output` default
describes ordinary compilation; the explicit `grounded=True` caller option selects the audit
output instead. A draft may remain blocked for missing citations or a clinical claim, and passing
an exact metadata audit does not establish scientific entailment.

## Unsupported requests and evaluation evidence

Date or freshness filters, molecular subtypes, intervention filters, trial recruitment/phase
filters, selected individual TCGA projects, full abstracts, molecular measurements, free-form
scientific summaries, ambiguous specialties, clinical requests and external effects are outside
this grammar. Unsupported clauses survive verbatim and prevent readiness. In particular, the
sources retain their fixed glioma lanes: the original question is **not** a source search query.
The interpreter does not turn an IDH-specific request into a general glioma search.

The dedicated regression suite separates source composition/exclusion, source-only requests,
unequal numeric limits, pagination semantics, conflicts, unsupported scientific filters,
non-glioma/ambiguous intent, clinical/effectful boundaries, receipt integrity, dataflow and
approval/resume behavior. These cases were authored with the grammar. They are not a held-out
natural-language benchmark; paraphrases and repeated prompt checks are not independent examples.
Scientific relevance, retrieval completeness, general language coverage and general task fidelity
remain unmeasured. No network or live model call is needed by these tests.
