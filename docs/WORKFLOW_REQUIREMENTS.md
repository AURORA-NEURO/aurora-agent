# Caller-bound workflow requirements

`workflow_requirements.py` defines a repository-specific contract for the parts of a research task
that the caller can state precisely. A model may propose a graph, but it cannot replace the fixed
contract while repairing that graph. The contract can require operations and final outputs,
forbid operations, constrain arguments, require value flow, and bound steps and dispatch attempts.

This checks explicit caller requirements. It does not establish that a free-text request was
translated correctly, that retrieved records are relevant or complete, or that a scientific claim
is supported. The caller remains responsible for reviewing the contract against the intended task.
See [the development plan](WORKFLOW_DEVELOPMENT_PLAN.md) for the remaining task-fidelity work.

The opt-in [caller-declared intent obligations](WORKFLOW_INTENT_OBLIGATIONS.md) sidecar can retain
unavailable/unresolved mandatory actions before model authoring, then lower admitted exact
operation/literal declarations conjunctively into this contract. It does not extract freeform
intent or prove that all task obligations were captured.

Legacy `aurora-workflow-requirements/0.1` links retain their original transitive value-flow
semantics and canonical digests. A link may now add `source_pointer` and/or `target_path`; such
contracts use version `0.2`. `source_pointer` is an RFC 6901 path inside the producing output;
the empty string requires the whole output value. `target_path` identifies the exact argument
leaf on the consuming operation. A reference at a different argument or to a different child
cannot satisfy that link. The contract still does not authenticate a source or certify science.

`audit_workflow_reference_paths(proposal, catalog)` separately reports known impossible paths
and paths requiring runtime resolution. Closed object properties, scalar traversal and bounded
array limits can prove impossibility. Open properties, optional fields and unbounded indices
remain unresolved until an actual source value is available. Compilation refuses proven
impossibility; a deferred check does not prove that its value exists. Model proposal context now
includes operation output schemas as well as input schemas within the existing byte bound.

## Compile and check without calling a source

The following example registers three implemented public readers and compiles a one-step plan.
Registration, contract compilation, audit and plan compilation make no model or source requests.
The requested limit bounds returned metadata; it does not promise that two matching records exist.

```python
from prism_sdk import (
    WorkflowOperationCatalog,
    audit_workflow_requirements,
    compile_workflow_plan,
    compile_workflow_requirements,
    register_research_source_operations,
    validate_workflow_requirements,
)

catalog = register_research_source_operations(WorkflowOperationCatalog())
task = "Retrieve at most two public glioma metadata records from Crossref."
requirements = compile_workflow_requirements(
    task,
    required_operations=["crossref.search_works"],
    output_operations=["crossref.search_works"],
    argument_rules=[
        {
            "operation": "crossref.search_works",
            "path": "/query",
            "comparison": "equals",
            "value": "glioma",
        },
        {
            "operation": "crossref.search_works",
            "path": "/limit",
            "comparison": "equals",
            "value": 2,
        },
    ],
    max_steps=1,
    max_calls_by_operation={"crossref.search_works": 1},
)
validate_workflow_requirements(requirements, catalog)
proposal = {
    "name": "Bounded Crossref metadata",
    "steps": [{
        "id": "metadata",
        "operation": "crossref.search_works",
        "arguments": {"query": "glioma", "limit": 2},
        "depends_on": [],
    }],
    "outputs": ["metadata"],
}
audit = audit_workflow_requirements(proposal, requirements, catalog)
assert audit["contract_satisfied"] is True
plan = compile_workflow_plan(proposal, catalog, task=task, requirements=requirements)
```

The complete contract uses `aurora-workflow-requirements/0.1`. Plans containing it use
`aurora-executable-workflow-plan/0.2`; plans without a contract retain
`aurora-executable-workflow-plan/0.1`.
The task digest binds the exact task text, including whitespace. The plan digest covers the full
contract and its digest. Loading a modified contract or applying it to a different task fails
validation. These hashes detect inconsistent content; they are not signatures or an authorization
mechanism, and a trusted application able to replace all inputs can recompute them.

## What each requirement checks

| Field | Check |
| --- | --- |
| `required_operations` | Each named operation occurs in at least one plan step. |
| `forbidden_operations` | No plan step uses a named operation. |
| `output_operations` | At least one step for each named operation is selected in the final `outputs` list. |
| `argument_rules` | Every occurrence of the named operation satisfies its JSON-pointer predicate. |
| `data_links` | Every occurrence of the target operation has the source operation in its actual argument-reference ancestry. |
| `max_steps` | The plan has no more than the specified number of logical steps, within 1–32. |
| `max_calls_by_operation` | Both logical plan occurrences and cumulative persisted dispatch attempts stay within each operation's cap. |

Output operations, argument-rule operations and both ends of data links are automatically added
to the required set when compiling the contract. Required and forbidden sets cannot overlap.
Validation with a catalogue resolves all named operations, including forbidden operations and
call-cap entries. Each selector, rule, link and cap collection is bounded to 128 entries.

An argument rule has exactly `operation`, `path`, `comparison` and `value`. Its path is a bounded
JSON pointer, for example `/limit`, `/specialty_lanes/0` or an empty string for the entire argument
object. The supported comparisons are:

- `equals`: exact canonical JSON equality, including type; `true` does not equal `1`.
- `contains`: an exact JSON array member, or a case-sensitive substring when both values are strings.
- `at_most` and `at_least`: comparisons between finite nonboolean numbers.

A predicate on `/query` checks that field's value. It does not establish biological relevance or
improve a source's search semantics. Catalogue schemas still constrain allowed fields, defaults,
types and limits independently. Plan compilation checks normalized arguments, including defaults.

A data link has exactly `source_operation` and `target_operation`, for example:

```json
{
  "source_operation": "pubmed.search",
  "target_operation": "glioma.evidence.reconcile"
}
```

The compiler follows `{"$ref": "step_id#/json/pointer"}` references in arguments, including
transitive reference ancestry. A `depends_on` edge by itself does not satisfy this requirement.
The check establishes structural value flow, not that the target used the value scientifically
correctly or that a particular claim follows from it.

## Unknown predicates and runtime bounds

`audit_workflow_requirements` expects a structurally checked proposal. Its result preserves three
states for `contract_satisfied`:

- `false`: at least one blocking issue was found.
- `null`: no blocking issue was found, but argument predicates depend on unresolved references.
- `true`: all explicit checks could be evaluated and passed.

`deferred_arguments` identifies the unresolved predicates. `blocking_issues` records failures
without copying arbitrary argument values into the audit. A successful static audit is not an
execution receipt. The executor resolves references, validates normalized arguments and checks
the predicates again before dispatch. Missing or mismatched resolved values block that step.
Resume validates the embedded contract, stored arguments and cumulative attempt counts again.

Call caps are integers from 0 through 32. A zero cap prevents use of an optional operation and
cannot be assigned to a required operation. Caps apply across all steps using that operation;
they do not reset on resume or explicit retry. Failed attempts and uncertain in-flight attempts
consume the persisted budget. A cap of one therefore does not permit a second source dispatch
after an earlier failed or interrupted attempt. Validation holds before a dispatch attempt do not
authorize a call. The separate per-run `max_step_calls` and per-step retry limits still apply.

## Local-model planning and bounded repair

`merge_workflow_requirements(task, first, second, ..., catalog=catalog)` conjoins up to sixteen
contracts bound to the exact same task. Required/output/forbidden operation sets, value-flow links
and predicates are retained; exact duplicates are removed. Step and dispatch caps take the tighter
bound. Direct equality/range conflicts and required-versus-forbidden conflicts refuse compilation.
The helper does not claim general logical satisfiability. Normal plan/dispatch checks still apply.
This is used when additional caller requirements accompany a typed glioma interpretation; the
caller file cannot replace the interpretation's source exclusions or exact argument requirements.

For an already configured `LocalWorkflowBuilder`, pass the same contract into `build`:

```python
plan = builder.build(
    task,
    operation_ids=["crossref.search_works"],
    max_steps=1,
    requirements=requirements,
    max_planning_attempts=2,
    approve_provider_call=True,
)
```

`max_planning_attempts` accepts 1–4 and defaults to one. A returned proposal that violates DAG,
schema, visible-operation or fixed-contract checks may receive another model attempt within that
budget. The same task, catalogue, operation allowlist and contract remain fixed. Each repair
attempt is another approved local provider invocation; it never executes a source operation.
Transport errors and runtime structured-output refusals propagate rather than entering the
proposal-repair loop. The effective planning step bound is the smaller of `max_steps` and the
contract bound. Required operations must fit the allowed and visible operation budgets.
The SDK requires the planning provider to use `max_attempts=1` (for example,
`runtime.register_provider(ollama_provider(max_attempts=1))`). This prevents transport retries
from silently multiplying the approved planning-call budget. The CLI already sets this bound.

Provider approval remains separate from source-dispatch approval. A valid plan and a satisfied
contract grant neither authority. Execution continues to use the reviewed catalogue and explicit
source approval described in [executable workflows](NLP_WORKFLOW_BUILDER.md). The contract adds no
credentials, model-owned URLs, new integration permissions or clinical authority.

## Scope and limits

The implementation checks selected operations, outputs, argument predicates, structural value
flow and finite bounds. It does not infer the complete contract from arbitrary prose, prove
scientific entailment, inspect full-text evidence, guarantee source availability, ensure requested
record counts exist, or certify the quality of a generated glioma research draft. Those remain
separate measurements and review tasks. Research artifacts retain their source omissions and
human-review holds; they do not diagnose an individual or recommend treatment.
