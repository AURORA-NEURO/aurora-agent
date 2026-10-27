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

## Separate opt-in decoder experiment

A later SDK-only experiment adds `workflow_decoder.prepare_workflow_decoder_schema` and
the constructor options `LocalWorkflowBuilder(..., constrain_decoder=False,
decoder_prefix_items=False)`. Both options require literal booleans and default to false.
The helper returns an immutable `WorkflowDecoderSchema` with copy-returning
`response_schema`, `audit`, and `to_dict()` accessors. Unsupported transformations raise
`WorkflowDecoderSchemaError` before provider dispatch. The helper performs no I/O and
provides no model or source authorization. No CLI flag or default-model change follows from
this experiment.

The helper emits operation-discriminated step alternatives from each operation's actual
input schema. Literal values retain their native constraints; explicit reference alternatives
represent values whose types must be checked after resolution. A schema 0.2 `target_path`
forces a strict, direct reference object at that exact argument leaf. If `source_pointer`
is present, its exact suffix is encoded. Empty source pointers require the trailing `#`.
Descendant links prohibit whole-ancestor reference shortcuts. No step IDs, dependency graph,
source result, or fallback workflow is inserted by this compiler.

The public audit distinguishes these **provider hints** from source-operation identity,
dependency closure/cycles, required operations and call budgets, caller argument predicates,
actual pointer existence, and resolved value types. Those remain compiler/executor work.
Links without `target_path` are explicitly recorded as compiler-only. The current runtime
validator does not fully check `anyOf`, `const`, `pattern`, `prefixItems`, and other emitted
constraints. A provider ignoring the hints can still return an invalid proposal; normal
workflow compilation and every immutable requirement remain mandatory.

Positional argument arrays require the caller to declare `prefix_items_supported=True` to
the helper, or `decoder_prefix_items=True` to the builder. This declaration is application
knowledge, not an automatic capability probe or a universal provider guarantee. Every native
length compatible with the exact target index is represented as a separate bounded tuple:
the native maximum must be finite, at most 128 items, with at most eight permitted lengths.
Each slot retains the native item schema, including literal sibling constraints. Native
`uniqueItems` is retained, while its actual enforcement remains compiler-owned. Unbounded or
larger positional ranges refuse; there is no minimum-length-only narrowing or generic
fallback. Prefix schemas are not combined with an ambiguous homogeneous `items` schema.

The opt-in subset refuses unsupported input schema composition/negation, false input
branches, ambiguous/scalar target traversal, closed-property violations, contradictory exact
leaves, and schema traversal/size overflow. Response schemas are capped at 256,000 serialized
bytes. Opt-in planning checks the complete UTF-8 message context, including repair context,
against 2,000,000 bytes before a provider call. Actual operation input/output schemas remain
in context; bounds do not cause them to be silently omitted. Default planning retains its
existing bounded runtime behavior.

The [Ollama structured-output guide](https://github.com/ollama/ollama/blob/main/docs/capabilities/structured-outputs.mdx)
describes supplying JSON Schema through the compatible API. This is not evidence of every
keyword's implementation. The [llama.cpp grammar documentation](https://github.com/ggml-org/llama.cpp/blob/master/grammars/README.md)
also describes limitations of schema conversion. The experiment therefore checked the actual
local provider separately and preserves the compiler as the authority.

### Development observations and two fresh held-out cases

Two new developer-authored cases and exact caller contracts froze at
`2026-10-27T03:25:11Z`, before new real-model development calls. They used a registry-only
three-stage review and a three-source literature/registry/TCGA review. These are variants of
previously seen graph families, not new independent NLP benchmarks. None of the previous
frozen failure cases was rerun or reclassified.

Three separate synthetic development calls used `qwen2.5:1.5b` on local Ollama `0.40.4`:

- A deliberately reversed prompt produced the schema-required left/right tuple order.
- A generated nested operation schema produced the two strict root-reference slots, but
  its full workflow failed dependency/output identity checks. This remains a workflow failure.
- A bounded union admitted the requested upper native length of three and forced the first
  strict root-reference leaf despite an inline-object prompt. This was a syntax/position
  observation, not a complete workflow success or source-identity proof.

The candidate and audit program froze at `2026-10-27T03:49:05Z`, before held-out calls.
Both variants used the **same frozen source and identical initial task/instruction messages**;
only decoder opt-in/preflight and the response schema differed. Baseline ran before candidate.
Each case allowed two proposal attempts and one transport attempt per invocation. All source
executors refused execution. No typed interpreter or deterministic template was used.

| Fresh case | Decoder disabled | Decoder enabled | Disabled calls | Enabled calls |
|---|---|---|---:|---:|
| Native registry bundle to reconciliation to human review | Failed | Failed | 2 | 2 |
| Three native sources to reconciliation to human review | Failed | Failed | 2 | 2 |
| Total | 0 of 2 accepted | 0 of 2 accepted | 4 | 4 |

The disabled registry proposal violated its fixed contract; its three-source proposal used
invalid reference syntax. The enabled registry proposal named an output ID absent from its
steps, and the enabled three-source proposal left steps disconnected from its output. The
enabled proposals also selected repeated reader operations instead of the required review
operations. Decoder branch admission does not establish required operation coverage. Both
were rejected by the workflow compiler; valid JSON was not promoted to task success.

There were eight held-out provider calls and three synthetic development calls: **11 total,
zero source calls**. No whole-workflow fidelity gain was measured. All failures and raw
returned proposals were retained. No code/task tuning or rerun followed observation of these
held-out outputs. The owned model was unloaded afterward, with an empty `/api/ps` result.

The accompanying local artifacts are `aurora-decoder-development-probes.json` and
`aurora-local-decoder-planning-evaluation.json`. The latter retains exact tasks/contracts,
frozen operation descriptors, actual request schema/message digests, per-call decoder audits,
model metadata, calls/latencies, graph audits and sanitized failures. Their identities are:

| Evidence | Digest |
|---|---|
| Fresh frozen taskset | `63803032b099ce2c18b34a607bc8cd4cf25ebe31ea6a463b3ca5841ace022a24` |
| Frozen builder source | `dc2958e29b99825374623a0db167f2e3cf82ed1e9d032547df0622f0756c0a43` |
| Frozen decoder helper source | `5d97f486170dbd355f39756c1f1169ee19d4bedda3e2a3fc56fb822443a7b34b` |
| Synthetic development report | `eb25c5f1dcf02000d69982eb3ba1b1c51ed9c6402426b7a277cc6bdba47685bc` |
| Held-out result report | `b3c17ceceaac909ccf48fa85f62d6996e57c32079196d69d956d98f1828319e1` |

The focused tests passed 157 tests and 20 subtests, including 24 new decoder cases. They
exercise root/child and whole-arguments references, array position and native length
preservation, pointer escaping, conflicting overlays, capability/size refusal, immutable
metadata, unchanged default prompts, and a provider that ignores decoder hints. Those are
controlled invariant checks, separate from the unsuccessful live workflow measurement.

### Independent correctness review after the frozen experiment

A subsequent offline review found a native-literal preservation defect in the measured helper:
an input schema containing `properties` could be coerced into an object schema even when its
native type was absent or explicitly scalar. For example, native
`{"properties":{"note":{"type":"string"}}}` accepts the scalar `7`; `properties` alone
does not imply `type="object"`. The earlier transformation incorrectly excluded that value.

The current helper explicitly refuses untyped object/array container-keyword schemas and
container keywords inconsistent with an explicit type. It also refuses those unsupported
shapes in nested or optional branches, before provider dispatch. It does not narrow their
native literal domain or silently emit a partially adapted alternative. Supported explicitly
typed object and array transformations retain their existing semantics and finite positional
length union. Default planning is unchanged.

This is an independently reproduced correctness fix after the frozen candidate, not tuning of
the held-out tasks. The frozen source/report hashes above still identify the code and observations
actually measured. No real-model or source calls were made for this fix; the two variants remain
**0 of 2 accepted**, and the corrected helper has no new live fidelity measurement. Offline
regressions establish pre-provider refusal and native literal admission only.
