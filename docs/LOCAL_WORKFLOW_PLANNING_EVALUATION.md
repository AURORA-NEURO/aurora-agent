# Local workflow planning: limited prospective checks

The instruction change measured on October 26, 2026 (America/Chicago) did **not** improve
accepted-task count: the existing planner and the instruction-only candidate each passed two
of four new developer-authored cases on `qwen2.5:1.5b`. Neither produced an accepted multi-step
glioma review in this check. General natural-language workflow construction remains unproven.

This is repository-defined application evaluation, not a blueprint capability measurement.
Valid JSON, schema admission, a compiled graph, and task success are separate observations.
Every accepted result below also needed its immutable caller contract and the prospective
graph audit to pass. No retrieved evidence or scientific claim was evaluated.

## What changed

The candidate removes a pseudo-path example from the reference instructions. It describes
whole-output references (`producer#`), schema-grounded child selection, dependency closure,
and the absence of an implicit output wrapper. For a schema 0.2 caller link it also explains
the exact `source_pointer` suffix and the direct argument placement at `target_path`. Empty
source pointers select the complete native output; empty target paths select the entire
consumer arguments object. Legacy 0.1 links retain their existing transitive-flow meaning.

The change does not generate a workflow or substitute a template after model failure. The
model still supplies every proposed step, argument, dependency, and output. Actual input and
output schemas remain in model context. Decoder schema, provider/source approvals, immutable
requirements, operation allowlists, planning attempts, transport attempts, static reference
analysis, execution checks, and existing byte bounds are unchanged. Static schema admission
still does not prove that an optional child or array element will exist at execution time.

The already-present output-schema context, static impossible-pointer rejection, and exact
0.2 flow contracts were included in both baseline and candidate. They are not credited to
this instruction change. An operation-discriminated decoder schema was considered but was
not implemented or measured in this iteration.

## Prospective protocol

The four new cases and their exact tasks, contracts, allowlists, limits, and graph audits were
frozen at `2026-10-27T03:12:17Z`, before model calls. The candidate source was separately
frozen at `2026-10-27T03:15:33Z`. The held-out run occurred from `03:15:50Z` to `03:16:26Z`.
These UTC timestamps fall on October 26 in the client timezone.

The cases were held out from this change's synthetic development fixtures. The same developer
authored both. Their graph families overlap earlier probes; these are **four new cases, not
four new independent benchmark classes**. Source combinations and output arity differ from
the earlier checks. No sampling distribution, statistical confidence, or population success
rate is established.

| Case | Fixed public-metadata task and graph |
|---|---|
| Registry only | ClinicalTrials.gov `glioblastoma` lane; page size 4; maximum 1 page; one native output |
| Aggregate review | GDC `TCGA-LGG` only; native bundle to reconciliation to human-review handoff; PubMed and trials forbidden |
| Literature and aggregate review | Independent PubMed `glioma` lane, limit 3, and GDC `TCGA-GBM`; ordered native bundles to reconciliation to review; trials forbidden |
| Three native bibliography outputs | Independent OpenAlex, Crossref, and Zenodo searches; exact query `glioma research`; limit 1 each; three separate outputs |

Review contracts fixed the exact question, requested source list, source labels, direct
producer-root references, consumer argument locations, output operation, and per-operation
call caps. They used schema 0.2 `source_pointer` and `target_path` fields. All cases had one
instance of each required operation. Readers had to remain independent; review dependencies
had to match the requested graph. Caller contracts expose exact operation identities and
constraints to the model; the score does not measure unconstrained intent interpretation.

Both variants used the same frozen reviewed-operation descriptors. Every experiment executor
was an execution-refusing guard. This was a planning-only catalogue snapshot, not a claim of
live adapter/service coverage. No typed glioma interpreter, deterministic workflow template,
source fallback, or source result was used.

The task-owned Ollama server at loopback port 11435 supplied the real model. The model tag was
`qwen2.5:1.5b`, digest
`65ec06548149b04c096a120e4a6da9d4017ea809c91734ea5631e89f96ddc57b`,
quantization `Q4_K_M`. Temperature was zero. Baseline cases ran before candidate cases;
requests were sequential. Each case allowed at most two proposal attempts, with exactly one
transport attempt per provider invocation. Runtime JSON/transport refusals propagated rather
than being silently retried as proposal repair. No OpenAI API or credential was used.

The maximum was 16 provider calls; 11 occurred. Separate development used controlled
in-memory model seams and disjoint synthetic records, with zero real-model calls. No case,
contract, prompt, or candidate code was tuned after seeing these held-out outputs. The
frozen set was run once per variant.

## Observed results

| Case | Baseline | Candidate | Baseline calls | Candidate calls |
|---|---|---|---:|---:|
| Registry only | Accepted | Accepted | 1 | 1 |
| Aggregate review | Failed | Failed | 2 | 2 |
| Literature and aggregate review | Failed | Failed | 2 | 1 |
| Three native bibliography outputs | Accepted | Accepted | 1 | 1 |
| Total | 2 of 4 accepted | 2 of 4 accepted | 6 | 5 |

The baseline aggregate case ended with malformed reference syntax. Its two-source review
left steps disconnected from the declared output. The candidate aggregate case fabricated
inline bundle/reconciliation objects and violated the exact caller contract. Its two-source
review failed JSON parsing in the provider runtime before a repairable proposal existed.
Rejected proposals were retained as failures; no successful graph was manufactured.

The lower candidate call count is **not** an efficiency improvement: one failed call did
not produce a proposal that could be repaired. Latencies are retained in the local report
but are not compared as a benchmark; the first run included model loading, and warm state
and run order were not controlled. Zero source calls and zero source-dispatch attempts were
observed. No scientific support, clinical applicability, or general NLP quality follows from
the two accepted cases.

The earlier four-case comparison remains unchanged: both `qwen2.5:1.5b` and `qwen2.5:7b`
passed two of four prospective structural probes. Larger-model review graphs used invented
child pointers and failed that experiment's native-root graph audit, despite satisfying its
then-weaker generic flow contract. This new run does not reclassify those prior results or
justify a default-model upgrade. No new 7B calls were made in this iteration.

## Evidence and checks

The local accompanying result artifact is
`aurora-local-planning-heldout-20261026.json`. It retains the frozen cases/contracts and
operation descriptors, model metadata, provider calls and latencies, accepted plans,
requirements/reference/graph audits, and sanitized failures. Source code snapshots, raw
returned proposals, and the experiment script are retained in the development workspace.
These local artifacts are separate from the repository's deterministic unit tests.

| Evidence | Digest |
|---|---|
| Baseline repository commit | `8baff9ce5b76dce565e9c79ec6421056ab194347` |
| Baseline builder source | `2a8df773790823304a0761ee8717b493a49a993d542161153bb554fe3526fd63` |
| Candidate builder source | `8f8002139f792b746ad64a965d97e4973a941a46c1535e48723766703653794c` |
| Frozen testset | `7f1b0b2681269b714467d63341d234bd4d7cc209d4811548fe697cb6bc47db92` |
| Frozen catalogue | `45960c447e4c3e457a7625d5e160615a4c9dc07807595e2a5070b3c357779c1c` |
| Completed result report | `e4cd3007f9b0725bf1a0464f4c664c7095bc8cc17428b3cc86c19bb545996f40` |

The five new controlled-model tests cover complete/child producer selection, whole-arguments
target placement, legacy-link compatibility, and bounded repair without a synthetic fallback.
The focused command passed 133 tests and 20 subtests:

```powershell
python -B -m pytest tests/test_workflow_planning_guidance.py tests/test_workflow_builder.py tests/test_workflow_builder_requirements.py tests/test_workflow_builder_integrity.py tests/test_workflow_reference_integrity.py tests/test_workflow_requirements.py -q
```

This is a Python-only change. A broader suite, installed-wheel validation, Rust checks, and
current integration/medical capability claims require their own evidence. The negative live
result remains evidence that clearer instructions alone did not solve multi-step fidelity.
