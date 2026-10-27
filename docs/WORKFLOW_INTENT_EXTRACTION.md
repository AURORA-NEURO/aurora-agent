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

Protocol `aurora-workflow-intent-extraction-protocol/0.1` supplies exact task text, the actual
catalogue digest and each selected operation's identity, description, read-only flag and native
input schema. It does not silently rank or truncate the selection. One to 128 visible operations
and the existing two-million-byte context bound are enforced before invocation; a caller must
explicitly select a smaller catalogue when the full catalogue exceeds those bounds.

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

Fixtures exercise exact Unicode/overlapping selectors, invalid-row retention, literal types,
unavailable actions, provider approval, bounded visibility/context, terminal responses, runtime
refusal and callback drift. They are boundary tests, not an actual-model benchmark. The earlier
frozen protocol 0.2/0.3 measurement predates this extractor and does not evaluate it. General NLP
fidelity, scientific validity and independent live integration availability remain unassessed.
