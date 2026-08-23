# Domain-neutral workflow execution binding

The reference workflow catalogue in `bioprism-interweave` describes six workflows and their
effect envelopes. Describing a workflow is not the same as executing one. The
`workflow_execution` module adds the narrow, reusable binding between that catalogue and the
epistemic execution contract without turning the catalogue into a scheduler or release authority.

## Binding contents

`WorkflowExecutionBinding::bind` validates an `AdaptivePlan` and records:

- the `WorkflowId` and a digest of its numbered roles, declared behaviours, and forbidden effects;
- the complete adaptive plan digest;
- the provider identity expected by the caller;
- caller-declared capability labels;
- the workflow's explicit forbidden effect kinds; and
- a canonical binding digest over all of the above.

The plan digest and binding digest are separate on purpose. Changing the decision problem or policy
changes the adaptive digest. Changing the workflow identity, provider, capability declaration, or
effect posture changes the binding digest even if the epistemic plan stays unchanged.

## Execution and replay

`WorkflowExecutionBinding::execute` accepts an optional explicit `ExecutionGrant` and a mutable
`AcquisitionExecutor`, then returns a `WorkflowExecutionReceipt` containing the lower-level
adaptive receipt. Omitting the grant is a valid, structured refusal path and does not call the
executor. It does not infer capabilities from a provider, authorize a human, schedule a
participant, publish a result, or perform an effect. A completed adaptive receipt is therefore not
a release decision.

`WorkflowExecutionBinding::replay` validates the workflow and plan identity before handing the
receipt to `AdaptivePlan::replay`. The lower layer creates a receipt-only executor with no live
source. Simulated rows stay simulated and replayed rows are labelled replayed; neither becomes
observed merely because a workflow wrapper was used.

This makes the same seam usable for software repair, claim reproduction, research-data audit,
incident response, policy comparison, and dataset transformation while preserving domain-specific
authority outside the generic kernel. Each domain still needs its own consent, privacy, safety,
rollback, publication, or human-approval gate before any effectful release.

## MCP and SDK contract

The `interweave_workflow_execute` MCP tool exposes this binding end to end for deterministic local
contract testing. Its required inputs are a closed workflow id, serialized decision problem and
belief, one to sixteen acquisitions, a finite scalar budget, and a zero-to-sixteen step bound.
Optional capabilities are digest-bound metadata. `mode: "simulate"` uses the built-in scripted
adapter and preserves `simulated` provenance; `mode: "replay"` requires a receipt from the exact
same workflow, plan, provider, capability set, and binding digest. `authorization` is optional so
the no-grant refusal can be inspected without any provider call.

The six accepted identities are:

- `reliable_software_repair`
- `scientific_claim_reproduction`
- `biomedical_research_data_audit`
- `incident_response`
- `evidence_grounded_policy_comparison`
- `dataset_transformation_molecule`

The route returns the workflow and plan/binding digests, binding metadata, completed/refused
status, provenance counts, the nested receipt, and the explicit posture
`workflow_receipt_only_external_release_not_authorized`. The Python SDK provides
`WorkflowExecutionRequest` and `WorkflowExecutionReport` through synchronous/asynchronous
Workspace and HTTP clients. The TypeScript SDK provides `WorkflowExecutionArgs`,
`WorkflowExecutionResult`, and `client.interweaveWorkflowExecute`. These facades validate the
closed workflow set, bounds, mode, capabilities, and replay receipt before transport.

## Portable workflow execution evidence

A route response is not a durable audit record. The
`interweave_workflow_execution_evidence` route converts an existing `binding` and `receipt` into
`bioprism-devplat-workflow-execution-evidence/0.1` and indexes the result in two bounded,
content-addressed registries. The record carries a caller-owned `subject_id`, caller-owned domain
labels, optional content-hash parents, the binding/plan/workflow-spec/provider identities, the
exact receipt digest, receipt status, completion posture, and separate observed/simulated/replayed
counts. Its `claim_posture` remains `review_required` and `readiness_claimed` is always `false`.

The evidence builder validates the binding's self-contained identity and the receipt's binding,
plan, provider, status, observation sequence, and digest shape before indexing. It does not need
the adaptive plan body, contact a provider, rerun a receipt, or infer that an observed provenance
label is authenticated. `interweave_workflow_execute` accepts an optional `evidence` object with
`subject_id`, `domains`, and `parent_digests`; when present, it creates this evidence projection
from the just-produced receipt without a second execution.

The registry tools are:

- `interweave_workflow_execution_evidence_import` for portable re-import with digest validation
  and idempotent duplicates;
- `interweave_workflow_execution_evidence_query` for bounded, cursor-paginated structural rows;
  and
- `interweave_workflow_execution_evidence_get` for one exact retained record.

The artifact registry also admits the `workflow_execution_evidence` kind, retaining the internal
evidence digest as a declared digest while keeping the registry's outer content digest separate.
Python exposes `WorkflowExecutionEvidenceRequest` and `WorkflowExecutionEvidenceReport` plus
sync/async Workspace and HTTP methods. TypeScript exposes the corresponding evidence args,
import/query result types, and `interweaveWorkflowExecutionEvidence*` methods.
