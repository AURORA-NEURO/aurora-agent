# Local NLP suggestions for mandatory workflow work

`LocalWorkflowIntentExtractor` asks an explicitly registered credentialless local model for
mandatory workflow declarations. This repository-defined application extension has no blueprint
module ID. It adds an interpretation proposal before graph authoring; a syntactically accepted
candidate does not establish that the model understood every instruction.

```python
from prism_sdk.workflow_intent_extractor import LocalWorkflowIntentExtractor
from prism_sdk.llm_runtime import LLMRuntime, ollama_provider

runtime = LLMRuntime()
runtime.register_provider(ollama_provider(max_attempts=1, structured_output_mode="json_schema"))
result = LocalWorkflowIntentExtractor(catalog, runtime, model="qwen2.5:7b").extract(
    task, approve_provider_call=True,
)
if result.status == "candidate":
    unreviewed_declarations = result.obligations
    assert unreviewed_declarations.to_dict()["interpretation_basis"] == "model_candidate"
```

The caller supplies the actual catalogue. The model name is illustrative configuration, not a
model download or a claim of measured extraction quality. No OpenAI key is needed. The provider
must be in-memory or loopback, explicitly credentialless, and configured for exactly one
transport attempt. Each call to `extract` needs literal provider approval. Extraction cannot
invoke a catalogue executor, compile a plan, authorize source dispatch or accept declarations.

## Protocol and retained provenance

New suggestions use `aurora-workflow-intent-extraction-protocol/0.3`. It supplies exact task text,
the actual catalogue digest and every selected operation's complete native `to_dict()` descriptor:
descriptor schema, operation identity, description, input schema, output schema, integration
identity, authentication mode, read-only flag and implementation label. Nested schemas, optional
fields, constants and literal JSON types are retained without summaries. Descriptor labels are
catalogue declarations; they establish neither execution authority nor live source availability.

An input schema describes arguments. An output schema describes a possible future returned
value, not acquired evidence or proof that any optional field will exist. A required operation
with `output=true` requests its whole future return as a final plan output. A returned artifact
does not require an extra input flag unless the native input contract defines one. There is no
implicit result wrapper, new output-field predicate or permission to invent source values.
Requested details that the contract cannot establish remain unresolved. These are instructions
to the model, not guarantees that it will interpret them correctly.

Protocol 0.3 adds two generic role instructions: interpret mandatory work for a future workflow
run by a separate executor, and distinguish the model's own lack of callable tools, network
access or credentials from the catalogue's declared operation contracts. That distinction does
not establish live reachability, authentication state, execution grants or scientific validity.
Essential unsupported work and incompatible literals must still remain visible; clarification
and refusal remain available. The change adds no operation-specific examples, new message roles,
automatic acceptance, response repair or execution ability.

The extractor does not silently rank or truncate the selection. One to 128 visible operations
and the existing two-million-byte context bound, including complete output schemas, are enforced
before invocation; a caller must explicitly select a smaller catalogue when the full catalogue
exceeds those bounds. This byte bound does not verify a particular model's token capacity or
establish that a server processed every token. Token allocation and model observations require
separate prospective checks.

Retained protocols 0.1 and 0.2 remain replayable with their original protocol and request bytes.
Version 0.1 exposed only identity, description, input schema and read-only flag. Version 0.2
exposed complete contracts with future-output instructions, without the two new role rules.
Validation never upgrades an old record into evidence of different contract exposure or role
instructions. The result schema and model response grammar stay at version 0.1; these request
protocol changes do not change the obligation kinds or caller acceptance boundary.

The model can propose up to 32 mandatory rows using the existing `required_operation`,
`argument_equals` and `unresolved_action` kinds. It is instructed to retain essential unavailable
work, preserve literal conflicts rather than replace requested values with defaults, and clarify
or refuse when needed. These instructions do not prove compliance or complete task coverage.

For a row's source span, the model selects exact `text` and a zero-based `occurrence` among
overlapping matches in the original task. The host resolves that selection to the existing
Python Unicode character offsets. For `aaa`, excerpt `aa` occurrence 1 selects `[1:3]`. This is
an explicit selector protocol, not a repair of invented offsets. Missing excerpts, out-of-range
occurrences, normalization, additional selector fields and native-row validation errors reject
the whole candidate. The host does not trim excerpts, rewrite actions, substitute literals or
drop bad rows to rescue a candidate.

The immutable result retains the exact task, request/schema/provider configuration digests,
actual visible identities, raw returned model text, its digest and byte length, and the
selector-to-offset mapping for accepted candidate syntax. The host always constructs the native
obligation document with `interpretation_basis="model_candidate"`. Model authority labels,
acceptance flags or replacement contracts are not accepted.

## Outcomes and boundaries

| Status | Meaning |
| --- | --- |
| `candidate` | Native obligation syntax and exact span binding passed. The declarations remain unreviewed and nonexecutable. |
| `needs_clarification` | The model explicitly requested clarification; there are no declarations or plan. |
| `refused` | The model explicitly declined; there are no declarations or plan. |
| `rejected` | A returned candidate failed native validation/exact provenance, or returned text exceeded complete-retention bounds. No repaired or partial candidate is admitted. |

Runtime/transport failures propagate separately. In particular, invalid JSON can be rejected by
`LLMRuntime` before a response is returned to this extractor; it is not relabelled as a returned
candidate or a measured task failure. The extractor retains only text actually returned to it.
Text above 128,000 UTF-8 bytes is omitted in full, with digest, byte count and explicit
`omitted_exceeds_bound` state. No truncated text is presented as complete evidence. The whole
result is bounded to 512,000 bytes and refuses rather than truncate further evidence.

There is one SDK provider invocation and a one-attempt transport policy, with no repair loop,
fallback or hidden retry. An SDK invocation count does not independently measure completed
inference or wire requests. Catalogue, provider and extraction controls are checked around the
runtime callbacks and transport boundary. These checks do not sandbox arbitrary host code.

A candidate passed directly to [intent admission](WORKFLOW_INTENT_OBLIGATIONS.md) still blocks
as unreviewed, even when all listed operations exist. A host must retain the original candidate
and record separate explicit caller acceptance before lowering a new caller assertion. Such an
event authenticates neither human presence nor semantic correctness. It cannot resolve an
unavailable action, eliminate a literal conflict or establish that the task has no omitted work.

`validate_workflow_intent_extraction` replays retained text through the same exact selector and
native-row parser without calling a provider or executor. It checks strict result fields,
digests, nonauthority flags, task/schema bindings and optional current catalogue/selection
bindings. Supplying a selection without its catalogue refuses instead of ignoring that binding.
Restamping a result cannot pair different raw text with native rows or offset mappings.
With a supplied catalogue, validation reconstructs the exact selected request context for the
recorded protocol and checks its request digest and context byte count. Protocol 0.1 uses its
original four descriptor fields and instructions; protocol 0.2 uses complete contracts and its
original future-output instructions; protocol 0.3 additionally uses the future-executor role
rules. Relabelling a record and restamping its result digest cannot make a different version's
request binding agree. Without a catalogue, request digests remain declared
observations. Provider configuration digests remain declarations in either case. Consistent
requests and self-computed hashes authenticate neither model authorship nor the truth of the
catalogue. Explicitly omitted oversized text remains unreplayable.

Fixtures exercise exact Unicode/overlapping selectors, invalid-row retention, literal types,
unavailable actions, provider approval, bounded visibility/context, terminal responses, runtime
refusal and callback drift. They are boundary tests, not an actual-model benchmark. The earlier
frozen incremental-planner protocol 0.2/0.3 measurement predates this extractor and does not
evaluate it. A separate protocol 0.1 functional refusal of available HGNC work stays failed;
omitted output contracts were a known context gap, but their causal contribution is only a
hypothesis. A later, single protocol 0.2 Cellosaurus functional check also refused available
declared work after seeing the full native contract. Its retained response cited the model's
lack of execution access. Confusing the interpretation role with execution access is a
hypothesis motivating the two protocol 0.3 rules, not an established cause or measured remedy.
Neither task was rerun or repaired. Offline replay preserves both refusals and their original
request versions. No actual-model protocol 0.3 result is established here. General NLP fidelity,
scientific validity and independent live integration availability remain unassessed.

## Prospective evaluation limits

A future evaluation should freeze the producer source, prompts, full selected contracts, task
families, semantic obligations and call budgets before any model observations. It should use
fresh tasks across available whole-return requests, output fields unsupported by a contract,
essential unavailable actions, conflicting input literals, and mixed literal/Unicode provenance.
The failed functional task and earlier frozen cases must not become a new held-out set through
paraphrasing. Related variants belong to one family, with separate robustness accounting.

Report semantic coverage and surplus work separately from native syntax, admission, planning and
durability. Retain clarification, refusal, malformed responses and runtime errors distinctly; do
not drop them from denominators or repair candidates to pass. Every candidate still needs an
explicit caller review before acceptance or planning. Pre-invocation output-only context overflow
and exact legacy replay can be verified with offline fixtures, but those checks do not measure
model comprehension. A targeted clarification or refusal can recognize a genuine missing action
or literal conflict without providing declaration coverage or an accepted plan. A generic refusal
about the model's own execution access does not establish recognition of that specific conflict.
Developer-designed composition, missing-capability and literal-conflict variants remain known
structural-family robustness probes, not held-out independent classes. No future evaluation or
new inference is authorized by this design text.
