# Incremental local workflow planning

`prism_sdk.workflow_incremental_planner` is an explicit, repository-defined SDK API for
constructing a workflow through bounded local-model decisions. It leaves
`LocalWorkflowBuilder`, existing plan formats, canonical digests and launcher defaults
unchanged. It never calls an operation executor.

Previous developer-authored local-model probes retained failures involving malformed
whole-graph references, invalid dependencies and output choices. The default model
condition comparison accepted 0/3 cases for Qwen 2.5 1.5B and 1/3 for 7B. One additional
7B graph passed the manually authored caller contract but failed its separate graph
audit. Those measurements concern the previous whole-graph builder. They do not measure
this incremental protocol, automatic requirement extraction or scientific quality.

A later frozen three-case developer-authored probe at 16,384 allocated context tokens
accepted 3/3 whole-graph proposals and 1/3 incremental plans. The incremental planner
explicitly refused two cases at its first chooser, referring to missing executed steps
or source annotations. Both were future workflow-authoring requests: no source execution
was available or needed to connect planned outputs. Different protocol call/output
budgets make that probe an operating-point comparison, not a causal reliability estimate.
Those failures remain evidence against assuming that incremental planning is more reliable.
The protocol remains opt-in and does not replace the default builder.

The protocol removes model-authored step identifiers and forward edges. The model still
chooses every operation, supplied literal argument, data binding, control dependency and
final output. The native compiler preserves reviewed schema defaults for omitted arguments;
a retained default is not evidence of a model choice.
It receives no prepared workflow graph. There is no template, domain-specific
operation selection or fallback graph.

## API and authority

```python
from prism_sdk.workflow_incremental_planner import IncrementalLocalWorkflowPlanner

planner = IncrementalLocalWorkflowPlanner(
    catalog, runtime, model="qwen2.5:1.5b", provider="ollama",
    max_visible_operations=32,
)
result = planner.build(
    task,
    approve_provider_call=True,
    operation_ids=reviewed_operation_allowlist,
    requirements=caller_requirements,
    max_steps=16,
    max_model_calls=64,
    max_decision_attempts=2,
)
if result.status == "planned":
    plan = result.plan
```

The application must already register a credentialless loopback provider, or an explicit
in-memory provider for fixtures. Provider `max_attempts` must be exactly 1. An approval
must be the literal boolean `True`. It grants bounded model calls only. Source execution
uses the existing separate approval and checkpoint APIs.

The CLI exposes the same opt-in protocol. With an installed local model and an existing,
operator-authored `reviewed-requirements.json` for the exact task, a bounded build is:

```console
python -m prism_sdk workflow-build --planner local-incremental --task "Retrieve at most two public PubMed glioma metadata records." --model qwen2.5:1.5b --base-url http://127.0.0.1:11434/v1 --operation pubmed.search --requirements reviewed-requirements.json --max-steps 1 --model-call-budget 4 --decision-attempts 1 --approve-provider-call --planning-receipt planning-result.json --out plan.json
```

`--model-call-budget` and `--decision-attempts` apply only to this planner;
`--planning-attempts` must remain 1. The optional planning receipt is separate from a plan.
Clarification, refusal and budget exhaustion write no executable plan or workflow run.
This build command grants model calls and performs no source dispatch. The model name is
an example, not a measured recommendation or default upgrade.

The API accepts at most 32 steps, 128 model calls and 4 attempts for each decision phase.
The defaults are 16, 64 and 2 respectively. An accepted append consumes at least two
calls; finishing consumes another. Rejections also consume calls. A provider transport
or runtime structured-output error propagates without a planner retry. Catalogue or
provider configuration drift, changed response identity and tool calls also fail closed.
The shared provider binding checks the exact registration, complete typed scalar controls,
transport/callable identities and HTTP connection factory before/after calls and at the
runtime's transport boundary. Its digest is stronger than the presentation-only provider
metadata. It does not freeze arbitrary mutable state inside a same-process callable's
closure; caller code remains part of the Python trust boundary.

Operation visibility is explicit. The planner refuses an overlarge catalogue instead of
silently omitting operations; provide a reviewed allowlist when needed. Read-only and
caller-forbidden filtering occurs before model visibility. Per-operation call caps and
the total step cap remove exhausted append choices, and validation independently checks
the model's returned decision.

## Model decisions

The first call selects one action:

| Action | Model-selected fields | Result |
| --- | --- | --- |
| `append` | A visible, non-exhausted operation identity | Request this operation's arguments |
| `finish` | A workflow name and existing output symbols | Compile the complete graph against the original caller contract |
| `clarify` | A bounded question/message | `clarification_required`, without a plan |
| `refuse` | A bounded explanation | `refused`, without a plan |

An append's second call uses that operation's input schema and returns an `arguments`
action with arguments plus `additional_dependencies`. References are explicit wire
values with exactly two fields:

```json
{"$symbol": "s1", "pointer": ""}
```

The empty pointer selects the earlier step's complete native output. `/metadata` selects
an actual child; pointer escapes follow RFC 6901. Accepted symbols are **unexecuted plan
steps**, not completed source observations. No observed data is available to the model.
Chooser calls receive only operation summaries (identity, description, integration and
argument property names). Argument calls receive the exact selected input schema and
descriptor metadata plus full output schemas of prior accepted producers. Equal producer
operations share one unchanged output schema with their distinct symbols listed. Unused
operation schemas and the selected operation's future output schema are omitted from
that phase's context. This reduces repeated context without choosing an operation or
binding. Both phases retain the complete caller contract and accepted steps; compilation
still uses the full catalogue. Context bytes and request digests record this deliberate
premeasurement protocol choice.

Decision protocol version 0.2 explicitly distinguishes **authoring a future workflow**
from answering a research question after execution. Every call labels an empty or partial
draft, records that execution has not started by design, and describes symbols as future
outputs. Missing observed values do not prevent the model from binding an earlier planned
output. An append records future work; a finish proposes a complete graph and makes no
claim that it ran. This changes context wording and labels, not decoder alternative order,
schema exposure, operation selection, compiler checks or provider-call budgets. It supplies
no example graph, automatic append, forced finish, retry after a terminal response or fallback.

`authoring_state.pending_contract_obligations` labels only missing required operations and
output coverage as unfinished graph obligations. At the empty draft these come directly
from the unchanged caller contract; after an append they match the native prefix audit's
pending issues. Without a caller contract the field is null, rather than a fabricated
successful audit. The original `prefix_audit` remains present without rewriting its digest,
issues, false/unknown contract result or deferred arguments. An irreversible or unknown
issue cannot be relabelled as pending. Temporary prefix outputs still do not establish
the model's final output selection.

Clarification and refusal remain available in every phase. The instructions specifically
preserve clarification for a genuinely unspecified selector and refusal for unavailable
capabilities, incompatible constraints or prohibited authority. They prohibit invented
selectors and observed values. Whether a local model follows these instructions requires
prospective positive cases and intent controls; fixture acceptance cannot establish it.

Only previously accepted symbols appear in the reference and control-edge enums. The
protocol independently rejects unknown, self and forward symbols, invalid pointer syntax,
native `$ref` injection and duplicate/unknown control dependencies. The native compiler
also refutes statically impossible output pointers. An optional or open-schema path still
requires resolution against the actual output at execution; admission proves no value
existence or type compatibility between producer and consumer.

After validation, the protocol supplies the next identifier (`s1`, `s2`, ...), compiles the
chosen wire references into native `$ref` values and derives dependency edges from those
references. Model-selected additional control edges are included after validation. Edge
ordering follows accepted symbol order. This deterministic syntax translation does not
choose data flow or add a missing operation. No accepted step can be rewritten by a later
decision.

Each prefix undergoes the existing structural/compiler argument validation. A separate
prefix audit temporarily names every accepted symbol as an audit output. These temporary
outputs are inspectable, marked non-executable and never substitute for the model's
final output selection. Irreversible caller-contract violations reject an append, including
false literal argument predicates and missing data flow into an already chosen consumer.
Missing required operations and output coverage remain pending. Deferred predicates stay
deferred until actual values exist.

Only a model-selected finish calls `compile_workflow_plan` with the complete original
immutable caller requirements. A rejected finish leaves the prefix unchanged; it never
automatically fills missing steps or selects outputs. The caller contract and its digest
are retained, including exact 0.2 native-root/target-path constraints. Any constraints that
remain deferred continue to be enforced by the existing executor.

## Schema subset and bounds

Operation-specific response schemas are decoder hints. Their `anyOf`, enum and pattern
coverage depends on the provider implementation; the SDK runtime does not evaluate every
emitted JSON Schema keyword. Explicit protocol checks and the native compiler remain
authoritative. Decoder JSON acceptance is not workflow acceptance.

The supported input subset contains explicitly typed object/array containers, the exact
nullable-object type union `["object", "null"]` (either order), scalar types, scalar
enums/constants, homogeneous array items and the native scalar/container
constraints. Nested arrays retain their native minimum, maximum and uniqueness rules;
the protocol does not use `prefixItems`, restrict arrays to a minimum tuple length or
select a fixed arity. An unconstrained value schema may remain unconstrained as a decoder
hint, while explicit protocol validation still examines every reserved reference marker.

Nullable objects retain their original type list and native null literal; only the object
property/additional-property schemas are recursively adapted. Scalar/array unions and
ambiguous multiple-container unions are unsupported.

Input composition (`allOf`/`anyOf`/`oneOf`/`not`), other union types, container constants/enums,
untyped container keywords and reserved `$symbol`/`$ref` input properties, required keys
or defaults are refused before the first provider call. These are explicit limitations,
not a generic-schema fallback. In open objects, literal data with those reserved marker
keys is outside the protocol. This supported search subset is not equivalent to every
argument value the native compiler can accept.

Preflight checks every visible operation, including an optional operation the model might
never choose. Schema traversal is bounded to 20,000 nodes and depth 24, and each response
schema to 256,000 UTF-8 bytes. Phase-relevant, unmodified input/output schemas, the caller
contract, accepted prefix and feedback must fit the existing 2,000,000-byte context bound.
Oversize context is refused before the affected call; schemas are never silently dropped
or truncated. Byte limits do not establish that a model's actual token window fits the
request. Provider token usage, context exposure and possible truncation require separate
measurement.

## Outcomes and evidence

`IncrementalWorkflowResult` is immutable and offers `.status`, `.plan` and `.to_dict()`.
Only `planned` results contain a serialized executable-plan field; the property returns
`None` for `clarification_required`, `refused` and `budget_exhausted`. Exhaustion identifies
the total call limit or per-phase decision-attempt limit, retains accepted partial steps
and preserves rejected-call receipts. It is neither a model refusal nor a successful
partial plan.

The result records catalogue/task/requirements/provider identity, visible operations,
limits, request/response/schema digests, measured context bytes including the response
schema, actual returned usage, elapsed time and acceptance/rejection per completed provider call.
Unknown usage remains absent/unknown in the provider's returned mapping. There are no
source calls. Failed transport/runtime invocations raise and do not fabricate a completed
result with zero cost. Applications measuring those errors must retain their own invocation
receipts. There is no persistent resume API for an interrupted planner session.

Focused fixture tests exercise prior-only bindings, generated IDs, derived/control edges,
native root and nested array references, contract/call limits, explicit final output
coverage, non-plan terminal states, identity drift and unsupported/oversize refusal. They
provide no evidence that an Ollama model follows the protocol or understands a task.
A fresh frozen paired probe compared historical version 0.1 with version 0.2 on three
developer-authored positive cases from known graph families and two intent controls.
Both conditions used the same exact tasks, immutable contracts, catalogue, per-case call
budgets, 2,048 output-token bound and Qwen 2.5 7B model digest. All 23 provider invocations
returned observed HTTP/text responses with a measured allocated context of 16,384 tokens;
no source ran and no arm was unjudged. Prompt truncation remained unreported/unassessed.

| Fresh case | Version 0.1 | Version 0.2 |
| --- | --- | --- |
| MGMT/TERT metadata to molecular review | Refused at initial chooser | Refused at arguments, confusing the admissible 12-gene enum with a mandatory full panel |
| IDH2/BRAF plus GWAS metadata to molecular review | Refused because source data had not executed | Accepted three draft steps, then twice finished without the required review step; rejected and exhausted |
| Independent Europe PMC and cBioPortal outputs | Requested an unnecessary extra Europe PMC operation | Complete exact graph accepted |
| Genuinely unspecified gene selector | Refused, although clarification was required | Correctly requested the missing gene symbol |
| Unavailable Slack publishing operation | Refused for page-size/execution reasons, without identifying the unavailable writer | Accepted only the read step and exhausted the two-call budget |

Full graph acceptance was **0/3 versus 1/3**, with five versus eighteen provider invocations
across all five cases. The predeclared lexical control screen passed one control in each
condition. Separate independent coding-agent review found the baseline Slack message a
screen false positive: it attributed the problem to absent executed data and small page
size, rather than the unavailable publishing capability. Combined status, lexical and
semantic control acceptance was **0/2 versus 1/2**. The candidate's exhausted Slack draft
is a control failure, not a successful refusal or usable partial plan.

The revised context passed one positive instance and the missing-selector control, but
the probe does not establish reliable general planning. Defaults remain unchanged and
the incremental planner stays opt-in. No graph was repaired, terminal response retried,
case rerun, prompt tuned or source dispatched during this measurement. Timings reflect a
shared machine with other active processes, not isolated performance measurements.
The retained task artifacts are `aurora-authoring-semantics-evaluation.json` (report digest
`8be9ec5d5eb090cccebdf3d4ee7112136d2f1139466e3825d21854b9b69d2ad2`) and
`aurora-authoring-semantics-independent-verification.json` (digest
`da4a37ba8b5a7c0944eb61a2c60f1e4235f31b93a0f4fb328c67a6f89744c6a3`). These
are retained evidence for this developer-authored comparison, not independently collected
benchmarks, automatic NLP requirement extraction, scientific validation or human review.
