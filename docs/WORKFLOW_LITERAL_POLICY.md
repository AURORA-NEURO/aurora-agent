# Current literal policy before new authoring

The optional obligation authoring wrapper and the API worker's accepted-review planning path
apply a separate scalar policy before model invocation or new plan authoring. The wrapper's
caller has already constructed its builder/runtime; the worker gates before provider construction.
For example, an explicit integer `page_size=150` at a declared integer leaf with `maximum=100`
produces `current_policy_conflict`. The literal is retained; it is never replaced by 100 or a
default. This is a repository-defined application extension, without a blueprint module ID.

```python
from prism_sdk import assess_workflow_literal_policy, validate_workflow_literal_policy_report

report = assess_workflow_literal_policy(task, obligations, catalog)
checked = validate_workflow_literal_policy_report(
    report, task=task, obligations=obligations, catalog=catalog)
findings = checked.to_dict()
```

The report schema is `aurora-workflow-literal-policy/0.1`. Its digest binds the exact task,
obligations, catalogue and explicit policy definition. Each literal finding retains its operation,
argument pointer, literal digest, input-schema digest, and, when assessed, exact native leaf
pointer/digest, evaluated schema projection and validator diagnostics. Recomputing and comparing
the complete report rejects restamped changes. Recomputable digests authenticate neither an
application, a source, a model nor a human.

## Conservative applicability

Only a single JSON-pointer token into an explicitly declared property of an object root is
eligible. The root must have only known annotations and well-formed direct `type`, `properties`,
`required` and `additionalProperties` structure. The leaf must declare one scalar type: string,
boolean, null, integer or number. Only type and finite numeric minimum/maximum/exclusive bounds
are evaluated, through the existing `ToolCatalogue` object-wrapper validator. Booleans are not
numbers, integer and float input types are not coerced, and no default is inserted. Exact
`const`/`enum` findings remain the separate, unchanged admission0.1 responsibility.

Nested/array/open-property paths, missing or union types, alternative schemas, unsupported
constraints, malformed bounds and unchecked validator observations remain `not_assessed` with
a reason. Other scalar restrictions such as patterns are outside this initial policy. There is
no attempt to select a favorable schema branch, repair malformed schemas or infer constraints
through unknown paths. Limits refuse a whole report rather than returning a shortened prefix.

The three per-literal states are:

- `definite_scalar_conflict`: the evaluated necessary type/numeric constraint rejects the literal.
- `no_evaluated_violation`: those projected constraints accepted it; no whole-call validity follows.
- `not_assessed`: no definitive finding within this narrow scope; unknown is not a successful check.

An optional declared property is assessed only conditional on the equality obligation requiring
that path's presence. This does not claim the schema itself requires that property. Sibling
requirements, complete arguments, general satisfiability, task coverage, source availability,
scientific validity and execution authority remain unassessed. The report counts assessed and
unassessed findings separately. With no literal rows or with every row unassessed, it reports
`not_assessed`. `partially_assessed` requires both assessed and unassessed rows;
`no_evaluated_violation` requires all literal rows to have been assessed without a conflict.
Only definite conflicts block this additional gate; unknowns remain subject to all existing
later compiler, requirement, input and execution checks.

## Historical evidence stays historical

`admit_workflow_intent_obligations`, admission schema0.1 and its direct const/enum receipt are
unchanged. Existing review and acceptance records still load, validate, export and return their
original bytes and findings. A prior acceptance is a historical event, not permission to bypass
current constraints when attempting new authoring. The current report never restamps that event,
changes its old status, or treats it as invalid storage.

`build_workflow_with_obligations` first applies the existing admission gate, then raises
`WorkflowLiteralPolicyError` carrying `.report` for a current scalar conflict before calling
either model builder. Without obligations, the existing default builder path remains unchanged.
The worker similarly checks current policy after validating an accepted review and its exact
planning context. A conflict returns `status: "current_policy_conflict"`, the exact report in
`interpretation`, the unchanged `intent_review`, and `source_dispatch: false`. It saves no plan
or run and constructs no provider. The browser treats every non-`planned` status as a stopped
result, clears any earlier plan/source approval and displays the report as inert text.

This gate does not retroactively reassess retained plans/runs or arbitrary argument requirements
outside the supplied literal obligations. Extraction request protocols and their actual-model
results are separate. The frozen six-call0.2/0.3 prompt probe used the old admission behavior;
this policy is not an additional probe arm or a remedy measured by those observations.

## Evidence scope

Synthetic tests exercise types and numeric boundaries, conditional optional leaves, explicit
unknown states, no defaults/coercion, report/catalogue binding, zero-provider conflict gates,
historical review/acceptance bytes and safe continued authoring. A fixture captured the actual
old 25c6816 admission/worker producer using local synthetic rows and caller acceptance, with no
model, source or plan calls. These are compatibility and boundary checks, not NLP quality,
clinical evidence, independent service coverage or authenticated human review.
