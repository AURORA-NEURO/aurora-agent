# Caller-declared workflow intent obligations

The Python SDK can retain mandatory actions before a model authors a workflow. This opt-in
application contract closes a specific gap: a required action absent from the catalogue must
remain a blocking finding, and an essential literal argument must become an immutable equality
predicate. A valid reader-only graph cannot satisfy a separately declared unavailable action.

This is not automatic natural-language extraction. The caller supplies the declarations and
their exact task spans. Task interpretation completeness, scientific validity, live service
availability and general constraint satisfiability remain unassessed. This is a
repository-defined application extension; no blueprint implementation or coverage is claimed.

## Declare and admit

```python
from prism_sdk import (
    compile_workflow_intent_obligations, admit_workflow_intent_obligations,
    build_workflow_with_obligations, WorkflowIntentAdmissionError,
)

task = "Read the metadata; archive the result."
obligations = compile_workflow_intent_obligations(task, [
    {"id": "read", "kind": "required_operation",
     "span": {"start": 0, "end": 17, "text": "Read the metadata"},
     "operation": "reviewed.read", "output": False},
    {"id": "archive", "kind": "required_operation",
     "span": {"start": 19, "end": 37, "text": "archive the result"},
     "operation": "unavailable.archive", "output": True},
])
admission = admit_workflow_intent_obligations(task, obligations, catalog)
assert admission.status == "blocked"
assert admission.requirements is None
assert admission.to_dict()["provider_calls"] == 0

# `builder` may be LocalWorkflowBuilder or IncrementalLocalWorkflowPlanner.
# The unavailable mandatory action blocks before either builder invokes its provider.
try:
    build_workflow_with_obligations(builder, task, obligations=obligations,
                                    approve_provider_call=True)
except WorkflowIntentAdmissionError as error:
    blocking_findings = error.admission.to_dict()["blocking_issues"]
```

The example operation identities are illustrative declarations, not shipped integrations.
If `reviewed.read` is also absent, its separate finding is retained. All declarations are
mandatory. There is no best-effort substitution, optionality weight or semantic alias matching.

`aurora-workflow-intent-obligations/0.1` supports three record kinds:

| Kind | Additional fields | Meaning |
|---|---|---|
| `required_operation` | `operation`, boolean `output` | An exact operation must occur; `output=True` additionally requires a final output from it. `False` leaves output selection unconstrained. |
| `argument_equals` | `operation`, `path`, `value` | Require the operation and apply canonical JSON equality at an RFC 6901 argument pointer. |
| `unresolved_action` | `reason` | Preserve an action without guessing its operation mapping. Always blocks. |

Repeated `required_operation` declarations for one operation require presence, not separate
invocations. Their distinct obligation IDs/spans remain in the receipt; lowering deduplicates
operation identities. This version cannot express required instance counts or distinct roles.

Every record also has a unique bounded `id` and a `span` with `start`, `end` and `text`.
Offsets are Python Unicode character indices, with an exclusive end, not UTF-8 byte offsets.
The selected text must exactly equal the task slice. Spans establish retained text provenance;
they do not prove semantic entailment, correct interpretation or coverage of the whole task.
The document binds the exact task digest and contains at most 128 obligations within 128,000
UTF-8 bytes. Admission receipts are bounded to 512,000 bytes. Canonical JSON distinguishes
booleans, integers and floats, including inside containers. Secret fields, duplicate JSON keys,
non-finite numbers and `$ref`-bearing equality literals refuse validation.

The only interpretation bases are `caller_supplied` and `model_candidate`. Caller-supplied
means caller-asserted, not independently verified human provenance. Every model-candidate
document remains unreviewed and blocks, even when its listed operations are available. There
is no promotion API or accepted `human_reviewed`/verified-parser flag. This API does not discover
the true origin of arbitrary caller-authored data and does not establish extraction completeness.
The host application must classify raw model output as `model_candidate`. The serialized label
and recomputable digest authenticate neither authorship nor human review, and cannot prevent
an application from relabeling/restamping model output as `caller_supplied`. Admission receipts
explicitly retain this caller-asserted provenance boundary; relabeling is not reviewed extraction.

## Admission and immutable requirements

`admit_workflow_intent_obligations` checks the actual catalogue, explicit caller operation
selection when supplied, read-only policy, and any additional caller requirements. It retains all
blocking findings and produces one of these states:

- `needs_clarification`: an unresolved action or unreviewed model candidate remains.
- `blocked`: a required operation is unregistered, outside the explicit selection or effectful; a checked literal
  conflicts with its native schema; or fixed requirements conflict.
- `ready_for_constrained_authoring`: declarations can be lowered into a fixed contract and
  authoring may be attempted. This does not prove that a satisfying graph exists.

The receipt's `operation_selection` binds registered identities or the explicit caller list
before builder filtering. It is not a list of operations actually shown to a model. Builders
own read-only/schema eligibility, context ranking and count limits; `actual_model_visibility`
therefore remains `unassessed` in this builder-agnostic admission receipt. A later builder
refusal remains possible even after `ready_for_constrained_authoring`.

The literal/schema preflight checks only direct top-level argument properties under an
unambiguous object schema and their applicable `const`/`enum` restrictions. Each checked
obligation reports `definite_conflict`, `no_const_enum_conflict`, or `not_assessed`.
`no_const_enum_conflict` is not a general schema-validity or satisfiability claim. Nested paths,
root equality and schema combinations remain unassessed by this preflight. Their literal
predicates still lower unchanged and must pass the native compiler and resolved dispatch checks.

Admitted records lower to the existing [workflow requirements](WORKFLOW_REQUIREMENTS.md).
Conjunction preserves additional caller predicates, output/data-flow obligations, forbidden
operations and tighter bounds. Direct contradictions refuse instead of choosing a preferred
contract. Equality rules apply to **every instance** of an operation, matching existing
requirements semantics; this increment has no role-specific or step-specific selector.

`WorkflowIntentAdmission.requirements` can be passed to `compile_workflow_plan`. The wrapper
does this for either existing model builder. On success the wrapper returns only that builder's
existing plan/result; callers must separately call `admit_workflow_intent_obligations` and retain
its receipt to preserve source-span and lowering mappings. No worker/UI receipt persistence is
provided by that SDK wrapper. The optional [application review flow](WORKFLOW_API_WORKER.md#optional-durable-mandatory-work-review)
separately retains original candidates, explicit API caller acceptance and exact admission inputs
in application envelope 0.2. The wrapper recomputes admission and never accepts a
caller-supplied approved receipt as authority. `validate_workflow_intent_admission` likewise
recomputes the receipt against the exact task, declarations, catalogue, allowlist, caller
requirements and step bound; restamping a digest alone cannot remove a blocking obligation.

Once compiled, the lowered requirements remain bound into the existing schema-0.2 plan.
Editing must compile against that retained contract; execution and checkpoint replay use
the existing deferred-predicate and call-budget checks. The sidecar does not authorize source
dispatch or change approval requirements. Requirements constrain normalized arguments: a
request to omit a property specifically at authoring time is not represented by this version.

## Compatibility and evidence limits

Calling `build_workflow_with_obligations` without obligations delegates unchanged to the
existing builder. Existing schema-0.1/0.2 plan bytes, defaults, whole-graph default mode and
experimental incremental mode remain unchanged. The SDK sidecar itself adds no persistence.
The optional application review flow preserves legacy envelope 0.1 and native plan/checkpoint
bytes when absent. Its separately approved [local-model extractor](WORKFLOW_INTENT_EXTRACTION.md)
only proposes unreviewed candidates; caller acceptance is not proof of complete NLP extraction,
human authorship or scientific validity.

Regression tests use local fixtures and in-memory provider responses. They demonstrate
enforcement of supplied declarations, blocked-before-provider behavior, canonical identity,
exact bindings, conjunction, editing and checkpoint replay. They do not measure actual-model
behavior or automatic NLP extraction. The frozen paired protocol measurement and its negative
controls remain unchanged; these tests are not additional held-out task successes.

A future extraction evaluation needs fresh, prospectively frozen task families and independent
annotations, separate manual-contract and automatic-extraction arms, and explicit incorrect
admission/over-clarification counts. Paraphrases remain robustness variants of a task family.
General language fidelity, scientific correctness and live integration reliability remain
unmeasured by this increment.
