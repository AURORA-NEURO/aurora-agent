"""Bounded, model-chosen plan construction over prior accepted plan symbols.

This is a repository-defined application protocol, not a blueprint implementation.
Symbols describe unexecuted plan steps, never completed source observations. The model
chooses operations, supplied literals, bindings, control edges and final outputs. Native
schema defaults are retained by the existing compiler. This module supplies only
identifiers and reference/edge syntax. The existing workflow
compiler and immutable caller requirements retain acceptance authority.

Not implemented: automatic requirement extraction, scientific validation, source
execution, arbitrary input-schema algebra, persistent planner resumption, or a fallback
graph when a model fails. Decoder hints are not evidence of task fidelity.
"""
from __future__ import annotations

from dataclasses import dataclass
import json
import re
from time import monotonic
from typing import Any, Mapping, Sequence
from urllib.parse import urlsplit

from .authoring import content_digest
from .llm_runtime import LLMRuntime, ProviderRequest
from .workflow_builder import (
    MAX_WORKFLOW_PLANNING_CONTEXT_BYTES, MAX_WORKFLOW_STEPS, WorkflowPlan,
    WorkflowValidationError, _WorkflowProviderInvocationGuard,
    _assert_workflow_provider_binding, _capture_workflow_provider_binding,
    _encode, _proposal, _references, _safe,
    _unique_json_object, compile_workflow_plan,
)
from .workflow_integrations import WorkflowOperationCatalog
from .workflow_requirements import (
    WorkflowRequirements, audit_workflow_requirements, validate_workflow_requirements,
)

INCREMENTAL_WORKFLOW_RESULT_SCHEMA = "aurora-incremental-workflow-planning-result/0.1"
_AUTHORING_PROTOCOL = "aurora-incremental-workflow-decisions/0.2"
MAX_INCREMENTAL_MODEL_CALLS = 128
MAX_INCREMENTAL_SCHEMA_BYTES = 256_000
_RESERVED = frozenset({"$ref", "$symbol"})
_SCHEMA_KEYS = frozenset({
    "$comment", "$id", "$schema", "default", "description", "examples", "title",
    "type", "properties", "required", "additionalProperties", "minProperties", "maxProperties",
    "items", "minItems", "maxItems", "uniqueItems", "minLength", "maxLength", "pattern",
    "minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum", "const", "enum",
})
_PENDING_PREFIX_ISSUES = frozenset({"required_operation_missing", "required_output_missing"})


class IncrementalWorkflowPlanningError(WorkflowValidationError):
    """Configuration, schema support or an identity boundary prevents planning."""


@dataclass(frozen=True, slots=True)
class IncrementalWorkflowResult:
    """Immutable tagged result; non-planned states contain no executable plan field."""

    _encoded: str

    def to_dict(self) -> dict[str, Any]:
        return json.loads(self._encoded)

    @property
    def status(self) -> str:
        return self.to_dict()["status"]

    @property
    def plan(self) -> WorkflowPlan | None:
        value = self.to_dict().get("plan")
        return WorkflowPlan(_encode(value)) if value is not None else None


def _object(properties: Mapping[str, Any]) -> dict[str, Any]:
    return {"type": "object", "properties": dict(properties), "required": list(properties),
            "additionalProperties": False}


def _terminal_schema(action: str) -> dict[str, Any]:
    return _object({"action": {"type": "string", "enum": [action]},
                    "message": {"type": "string", "minLength": 1, "maxLength": 2000}})


def _reference_schema(symbols: Sequence[str]) -> dict[str, Any]:
    return _object({"$symbol": {"type": "string", "enum": list(symbols)},
                    "pointer": {"type": "string", "maxLength": 4096, "pattern": r"^(?:/.*)?$"}})


def _reserved_literal(value: Any) -> bool:
    if isinstance(value, dict):
        return bool(set(value) & _RESERVED) or any(_reserved_literal(child) for child in value.values())
    return isinstance(value, list) and any(_reserved_literal(child) for child in value)


def _wire_schema(schema: Any, symbols: Sequence[str], budget: list[int], depth: int = 0) -> Any:
    """Describe an explicit argument subset; no equivalence to all native schemas."""
    budget[0] -= 1
    if budget[0] < 0 or depth > 24:
        raise IncrementalWorkflowPlanningError("incremental_input_schema_traversal_bound")
    if isinstance(schema, bool):
        native = schema
    else:
        if not isinstance(schema, Mapping) or set(schema) - _SCHEMA_KEYS:
            raise IncrementalWorkflowPlanningError("incremental_input_schema_feature_unsupported")
        kind = schema.get("type")
        nullable_object = (isinstance(kind, list) and len(kind) == 2
                           and all(isinstance(value, str) for value in kind) and set(kind) == {"object", "null"})
        container_kind = "object" if nullable_object else kind
        if ((kind is not None and not isinstance(kind, str) and not nullable_object)
                or container_kind not in {None, "object", "array", "string", "integer", "number", "boolean", "null"}):
            raise IncrementalWorkflowPlanningError("incremental_input_schema_type_unsupported")
        objects = set(schema) & {"properties", "required", "additionalProperties", "minProperties", "maxProperties"}
        arrays = set(schema) & {"items", "minItems", "maxItems", "uniqueItems"}
        if (objects and container_kind != "object") or (arrays and container_kind != "array"):
            raise IncrementalWorkflowPlanningError("incremental_ambiguous_container_schema")
        if "enum" in schema and (not isinstance(schema["enum"], list) or not schema["enum"]):
            raise IncrementalWorkflowPlanningError("incremental_invalid_enum_schema")
        if any(isinstance(value, (dict, list)) for value in schema.get("enum", [])) or isinstance(schema.get("const"), (dict, list)):
            raise IncrementalWorkflowPlanningError("incremental_container_constant_unsupported")
        if "default" in schema and _reserved_literal(schema["default"]):
            raise IncrementalWorkflowPlanningError("incremental_wire_reserved_default")
        native = json.loads(json.dumps(schema, allow_nan=False))
        if container_kind == "object":
            properties = schema.get("properties", {})
            if set(properties) & _RESERVED:
                raise IncrementalWorkflowPlanningError("incremental_wire_reserved_input_property")
            required = schema.get("required", [])
            if (not isinstance(required, list) or any(not isinstance(key, str) for key in required)
                    or len(set(required)) != len(required) or set(required) & _RESERVED):
                raise IncrementalWorkflowPlanningError("incremental_invalid_or_wire_reserved_required_property")
            native["properties"] = {key: _wire_schema(child, symbols, budget, depth+1)
                                     for key, child in properties.items()}
            if isinstance(schema.get("additionalProperties"), Mapping):
                native["additionalProperties"] = _wire_schema(schema["additionalProperties"], symbols, budget, depth+1)
        elif container_kind == "array":
            native["items"] = _wire_schema(schema.get("items", True), symbols, budget, depth+1)
    return {"anyOf": [native, _reference_schema(symbols)]} if symbols else native


def _arguments_schema(schema: Mapping[str, Any], symbols: Sequence[str]) -> dict[str, Any]:
    result = {"type": "object", "anyOf": [
        _object({"action": {"type": "string", "enum": ["arguments"]},
                 "arguments": _wire_schema(schema, symbols, [20_000]),
                 "additional_dependencies": {"type": "array", "maxItems": len(symbols), "uniqueItems": True,
                    "items": {"type": "string", "enum": list(symbols)} if symbols else {"type": "string"}}}),
        _terminal_schema("clarify"), _terminal_schema("refuse"),
    ]}
    if len(json.dumps(result, allow_nan=False).encode("utf-8")) > MAX_INCREMENTAL_SCHEMA_BYTES:
        raise IncrementalWorkflowPlanningError("incremental_response_schema_byte_bound")
    return result


def _native_arguments(value: Any, symbols: Sequence[str], depth: int = 0) -> Any:
    if depth > 24:
        raise WorkflowValidationError("incremental argument nesting exceeds its bound")
    if isinstance(value, dict):
        if "$ref" in value:
            raise WorkflowValidationError("native $ref syntax is outside the incremental wire protocol")
        if "$symbol" in value:
            if set(value) != {"$symbol", "pointer"} or value["$symbol"] not in symbols:
                raise WorkflowValidationError("incremental reference must name a prior accepted symbol")
            pointer = value["pointer"]
            if (not isinstance(pointer, str) or len(pointer) > 4096 or (pointer and not pointer.startswith("/"))
                    or re.search(r"~(?![01])", pointer)):
                raise WorkflowValidationError("incremental reference pointer is not RFC 6901 syntax")
            return {"$ref": value["$symbol"] + "#" + pointer}
        return {key: _native_arguments(child, symbols, depth+1) for key, child in value.items()}
    if isinstance(value, list):
        return [_native_arguments(child, symbols, depth+1) for child in value]
    return value


def _authoring_state(steps: Sequence[Mapping[str, Any]], prefix_audit: Mapping[str, Any] | None,
                     contract: Mapping[str, Any] | None) -> dict[str, Any]:
    """Distinguish unfinished graph obligations from unavailable execution results.

    Native prefix diagnostics remain unchanged. Only the two already-admitted pending
    issue types may be labelled unfinished authoring; an unexpected issue must never
    become a harmless-looking progress report. This context chooses no graph elements.
    """
    if prefix_audit is not None and prefix_audit["requirements_audit"] is not None:
        pending = prefix_audit["requirements_audit"]["blocking_issues"]
    elif contract is not None:
        pending = [{"code": code, "operation": operation}
                   for field, code in (("required_operations", "required_operation_missing"),
                                       ("output_operations", "required_output_missing"))
                   for operation in contract[field]]
    else:
        pending = None
    if pending is not None and any(issue["code"] not in _PENDING_PREFIX_ISSUES for issue in pending):
        raise IncrementalWorkflowPlanningError("incremental authoring state contains a non-pending contract violation")
    return {
        "purpose": "author_a_future_workflow;not_answer_the_research_question_or_execute_steps",
        "draft": "empty" if not steps else "partial",
        "accepted_step_count": len(steps),
        "execution_status": "not_started_by_design",
        "observed_source_values": "unavailable_during_plan_authoring;not_required_to_bind_future_outputs",
        "symbol_meaning": "immutable_unexecuted_step_whose_future_output_can_be_referenced",
        "pending_contract_obligations": pending,
        "prefix_audit_meaning": "incomplete_graph_diagnostics;temporary_audit_outputs_are_not_final_outputs;deferred_values_are_checked_at_execution",
    }


class IncrementalLocalWorkflowPlanner:
    """Choose one operation, then its arguments, within an explicit provider-call budget.

    Accepted symbols are immutable plan syntax, not source results. At least two calls
    per accepted step and one model-selected finish decision are required. No operation
    executor is reachable through this API.
    """

    def __init__(self, catalog: WorkflowOperationCatalog, runtime: LLMRuntime, *, model: str,
                 provider: str = "ollama", max_visible_operations: int = 32) -> None:
        if not isinstance(catalog, WorkflowOperationCatalog) or not isinstance(runtime, LLMRuntime):
            raise IncrementalWorkflowPlanningError("incremental planner requires a catalogue and LLMRuntime")
        if not isinstance(model, str) or not model.strip() or len(model) > 200:
            raise IncrementalWorkflowPlanningError("incremental planner requires a bounded model identity")
        if type(max_visible_operations) is not int or not 1 <= max_visible_operations <= 128:
            raise IncrementalWorkflowPlanningError("incremental visible-operation bound is invalid")
        self.catalog, self.runtime, self.model, self.provider = catalog, runtime, model, provider
        self.max_visible_operations = max_visible_operations
        self._local_provider_metadata()
        self._provider_binding = _capture_workflow_provider_binding(runtime, provider)
        self._provider_digest = self._provider_binding.config_digest

    def _local_provider_metadata(self) -> dict[str, Any]:
        metadata = next((item for item in self.runtime.provider_metadata() if item.get("provider") == self.provider), None)
        if metadata is None or metadata.get("requires_credential") is not False:
            raise IncrementalWorkflowPlanningError("incremental planning requires a credentialless local provider")
        if type(metadata.get("max_attempts")) is not int or metadata["max_attempts"] != 1:
            raise IncrementalWorkflowPlanningError("incremental provider must use exactly one transport attempt")
        if metadata.get("transport") != "in_memory":
            endpoint = metadata.get("base_url")
            if (metadata.get("transport") != "http" or not isinstance(endpoint, str)
                    or urlsplit(endpoint).hostname not in {"127.0.0.1", "localhost", "::1"}):
                raise IncrementalWorkflowPlanningError("incremental provider must be loopback or in-memory")
        return metadata

    def build(self, task: str, *, approve_provider_call: bool = False,
              operation_ids: Sequence[str] | None = None,
              requirements: WorkflowRequirements | Mapping[str, Any] | None = None,
              max_steps: int = 16, max_model_calls: int = 64,
              max_decision_attempts: int = 2) -> IncrementalWorkflowResult:
        """Return a tagged planning result; a plan never grants source dispatch authority.

        Native compiler failures are bounded proposal rejections. Transport/runtime,
        configuration and catalogue/provider drift errors propagate without retries.
        Unknown semantic task fidelity is not encoded as successful measurement.
        """
        if approve_provider_call is not True:
            raise IncrementalWorkflowPlanningError("incremental planning requires explicit provider approval")
        if not isinstance(task, str) or not task.strip() or len(task.encode("utf-8")) > 32_000:
            raise IncrementalWorkflowPlanningError("incremental task text is outside its bound")
        for value, maximum, label in ((max_steps, MAX_WORKFLOW_STEPS, "steps"),
                                      (max_model_calls, MAX_INCREMENTAL_MODEL_CALLS, "model_calls"),
                                      (max_decision_attempts, 4, "decision_attempts")):
            if type(value) is not int or not 1 <= value <= maximum:
                raise IncrementalWorkflowPlanningError("incremental " + label + " bound is invalid")
        contract = validate_workflow_requirements(requirements, self.catalog,
            task_digest=content_digest({"task": task})).to_dict() if requirements is not None else None
        if contract:
            max_steps = min(max_steps, contract["max_steps"])
        required = set(contract["required_operations"]) if contract else set()
        forbidden = set(contract["forbidden_operations"]) if contract else set()
        operations = list(self.catalog.operations())
        if operation_ids is not None:
            if (isinstance(operation_ids, (str, bytes)) or not isinstance(operation_ids, Sequence)
                    or not 1 <= len(operation_ids) <= 128
                    or any(not isinstance(item, str) for item in operation_ids)
                    or len(set(operation_ids)) != len(operation_ids)):
                raise IncrementalWorkflowPlanningError("incremental operation allowlist is invalid")
            operations = [self.catalog.resolve(item) for item in operation_ids]
        operations = [op for op in operations if op.read_only and op.operation_id not in forbidden]
        visible = {op.operation_id for op in operations}
        if (not operations or len(operations) > self.max_visible_operations or required-visible
                or len(required) > max_steps):
            raise IncrementalWorkflowPlanningError("incremental operation visibility cannot cover its fixed contract")
        operations.sort(key=lambda op: op.operation_id)
        by_id = {op.operation_id: op.to_dict() for op in operations}
        summaries = [{"operation_id": value["operation_id"], "description": value["description"],
                      "integration_id": value["integration_id"],
                      "argument_properties": sorted(value["input_schema"].get("properties", {}))}
                     for value in by_id.values()]
        # Refuse unsupported or oversized transformations before any provider call, even
        # when the model would never choose the affected optional operation.
        for op in operations:
            _arguments_schema(op.to_dict()["input_schema"], ["s" + str(i+1) for i in range(max_steps)])
        digest = self.catalog.catalog_digest
        steps: list[dict[str, Any]] = []
        receipts: list[dict[str, Any]] = []
        prefix_audit = None
        feedback = None
        base = {
            "protocol": _AUTHORING_PROTOCOL,
            "task": task, "catalog_digest": digest, "operations": summaries,
            "fixed_caller_requirements": contract,
            "max_steps": max_steps,
            "rules": [
                "Author a future workflow. No operation has executed or can execute during this planning conversation. An empty accepted plan is the normal initial state.",
                "Choose the next operation, its literal arguments, bindings and control dependencies yourself. Appending records a future invocation; it does not require its source results.",
                "Accepted symbols are immutable earlier unexecuted plan steps. Bind a future consumer to an earlier symbol without waiting for observed values; never invent those values.",
                "A wire reference has exactly $symbol (prior accepted symbol) and pointer (RFC 6901). Empty pointer selects the complete native output; no implicit wrapper exists.",
                "Never emit native $ref, step IDs, depends_on, fabricated source observations, credentials or replacement requirements.",
                "Control dependencies order steps but do not transfer data. Choose data bindings explicitly.",
                "Missing operations or outputs in an incomplete prefix audit are pending graph-authoring obligations, not a request for observed source data. Other contract violations remain errors.",
                "Finish proposes a complete workflow, not completed execution. Finish only when every fixed caller requirement and task instruction is met; choose actual final output symbols.",
                "Clarify unresolved intent or a missing selector that cannot be inferred from the task or fixed contract; do not invent one. Refuse unavailable capabilities, incompatible constraints or prohibited authority when needed. These terminal choices remain valid at every phase.",
                "Public research metadata only. No clinical decisions or patient data. A plan grants no source authority.",
            ],
        }

        def guard() -> None:
            if self.catalog.catalog_digest != digest:
                raise IncrementalWorkflowPlanningError("incremental catalogue or provider identity changed")
            self._local_provider_metadata()
            _assert_workflow_provider_binding(self.runtime, self.provider, self._provider_binding)

        def result(status: str, **fields: Any) -> IncrementalWorkflowResult:
            guard()
            value = {"schema": INCREMENTAL_WORKFLOW_RESULT_SCHEMA, "status": status,
                "task_digest": content_digest({"task": task}), "catalog_digest": digest,
                "provider": self.provider, "model": self.model, "provider_digest": self._provider_digest,
                "requirements_digest": contract["requirements_digest"] if contract else None,
                "visible_operations": sorted(visible),
                "model_calls": len(receipts), "source_dispatches": 0,
                "limits": {"max_steps": max_steps, "max_model_calls": max_model_calls,
                           "max_decision_attempts": max_decision_attempts},
                "accepted_partial_steps": steps, "prefix_audit": prefix_audit,
                "decisions": receipts,
                "interpretation": "only_planned_status_has_a_compiler_accepted_plan;task_fidelity_and_science_unmeasured",
                **fields}
            value["result_digest"] = content_digest(value)
            return IncrementalWorkflowResult(_encode(value))

        def call(phase: str, schema: Mapping[str, Any], extra: Mapping[str, Any]) -> tuple[Any, dict[str, Any]]:
            guard()
            messages = ({"role": "system", "content": json.dumps({**base, **extra,
                "accepted_plan_symbols": steps, "prefix_audit": prefix_audit,
                "authoring_state": _authoring_state(steps, prefix_audit, contract),
                "validation_feedback": feedback}, ensure_ascii=False)},)
            encoded = json.dumps({"messages": messages, "response_schema": schema}, ensure_ascii=False)
            if len(encoded.encode("utf-8")) > MAX_WORKFLOW_PLANNING_CONTEXT_BYTES:
                raise IncrementalWorkflowPlanningError("incremental planning context exceeds its byte bound")
            request = ProviderRequest(model=self.model, messages=messages, require_json=True,
                response_schema=schema, max_output_tokens=2048, temperature=0)
            request_digest = content_digest({"model": self.model, "messages": list(messages),
                "response_schema": schema, "require_json": True, "max_output_tokens": 2048, "temperature": 0})
            schema_digest = content_digest(schema)
            started = monotonic()
            response = self.runtime.invoke(self.provider, request, invocation_observer=
                _WorkflowProviderInvocationGuard(self.runtime, self.provider, self._provider_binding))
            guard()
            if response.provider != self.provider or response.model != self.model or response.tool_calls:
                raise IncrementalWorkflowPlanningError("incremental response changed its model/provider/tool boundary")
            receipt = {"call": len(receipts)+1, "phase": phase, "response_digest": content_digest({"text": response.text}),
                "response_schema_digest": schema_digest, "elapsed_seconds": monotonic()-started,
                "request_digest": request_digest,
                "context_bytes_including_response_schema": len(encoded.encode("utf-8")),
                "usage": _safe(response.usage), "status": "rejected"}
            receipts.append(receipt)
            try:
                decision = _safe(json.loads(response.text, object_pairs_hook=_unique_json_object), maximum=500_000)
            except (ValueError, KeyError, TypeError):
                decision = None
            return decision, receipt

        def terminal(decision: Any, receipt: dict[str, Any]) -> IncrementalWorkflowResult | None:
            if not isinstance(decision, dict) or decision.get("action") not in {"clarify", "refuse"}:
                return None
            message = decision.get("message")
            if (set(decision) != {"action", "message"} or not isinstance(message, str)
                    or not message.strip() or len(message) > 2000):
                raise WorkflowValidationError("incremental terminal decision is invalid")
            receipt.update(status="accepted", action=decision["action"])
            return result("clarification_required" if decision["action"] == "clarify" else "refused", message=message)

        guard()
        while len(receipts) < max_model_calls:
            symbols = [step["id"] for step in steps]
            available = [op.operation_id for op in operations if not contract
                or sum(step["operation"] == op.operation_id for step in steps) < contract["max_calls_by_operation"].get(op.operation_id, max_steps)]
            branches = [_terminal_schema("clarify"), _terminal_schema("refuse")]
            if len(steps) < max_steps and available:
                branches.append(_object({"action": {"type": "string", "enum": ["append"]},
                    "operation": {"type": "string", "enum": available}}))
            if symbols:
                branches.append(_object({"action": {"type": "string", "enum": ["finish"]},
                    "name": {"type": "string", "minLength": 1, "maxLength": 160},
                    "outputs": {"type": "array", "minItems": 1, "maxItems": len(symbols), "uniqueItems": True,
                                "items": {"type": "string", "enum": symbols}}}))
            selected = None
            for _ in range(max_decision_attempts):
                if len(receipts) == max_model_calls:
                    return result("budget_exhausted", reason="model_call_limit")
                decision, receipt = call("choose", {"type": "object", "anyOf": branches}, {"phase": "choose_next_action"})
                try:
                    ended = terminal(decision, receipt)
                    if ended:
                        return ended
                    if not isinstance(decision, dict):
                        raise WorkflowValidationError("incremental action must be an object")
                    if decision.get("action") == "finish" and set(decision) == {"action", "name", "outputs"}:
                        proposal = {"name": decision["name"], "steps": steps, "outputs": decision["outputs"]}
                        plan = compile_workflow_plan(proposal, self.catalog, task=task, requirements=contract)
                        receipt.update(status="accepted", action="finish")
                        return result("planned", plan=plan.to_dict())
                    if (set(decision) != {"action", "operation"} or decision["action"] != "append"
                            or decision["operation"] not in available or len(steps) >= max_steps):
                        raise WorkflowValidationError("incremental append operation is unavailable")
                    selected = decision["operation"]
                    receipt.update(status="accepted", action="append", operation=selected)
                    feedback = None
                    break
                except (ValueError, KeyError, TypeError):
                    receipt["error_code"] = "invalid_action_or_final_contract"
                    feedback = {"code": receipt["error_code"], "instruction": "Choose a valid action. Accepted steps are unchanged; satisfy the immutable task contract before finish."}
            if selected is None:
                return result("budget_exhausted", reason="decision_attempt_limit")
            schema = _arguments_schema(by_id[selected]["input_schema"], symbols)
            accepted = False
            for _ in range(max_decision_attempts):
                if len(receipts) == max_model_calls:
                    return result("budget_exhausted", reason="model_call_limit")
                descriptor = by_id[selected]
                producers = [{"operation_id": operation, "symbols": [step["id"] for step in steps if step["operation"] == operation],
                    "output_schema": by_id[operation]["output_schema"]}
                    for operation in dict.fromkeys(step["operation"] for step in steps)]
                decision, receipt = call("arguments", schema, {"phase": "choose_operation_arguments", "selected_operation": selected,
                    "selected_operation_descriptor": {key: descriptor[key] for key in
                        ("operation_id", "description", "integration_id", "input_schema")},
                    "referenceable_producer_output_schemas": producers})
                try:
                    ended = terminal(decision, receipt)
                    if ended:
                        return ended
                    if not isinstance(decision, dict) or set(decision) != {"action", "arguments", "additional_dependencies"} or decision["action"] != "arguments":
                        raise WorkflowValidationError("incremental argument decision has invalid fields")
                    arguments = _native_arguments(decision["arguments"], symbols)
                    control = decision["additional_dependencies"]
                    if (not isinstance(control, list) or any(not isinstance(item, str) or item not in symbols for item in control)
                            or len(set(control)) != len(control)):
                        raise WorkflowValidationError("incremental control dependencies must be distinct prior symbols")
                    dependencies = {source for source, _ in _references(arguments)} | set(control)
                    step = {"id": "s" + str(len(steps)+1), "operation": selected, "arguments": arguments,
                            "depends_on": [symbol for symbol in symbols if symbol in dependencies]}
                    # Audit-only outputs retain every prefix step for structural validation.
                    # They are never a final model output selection or an executable plan.
                    prefix = _proposal({"name": "Incremental prefix audit", "steps": [*steps, step], "outputs": [*symbols, step["id"]]}, self.catalog)
                    audit = audit_workflow_requirements(prefix, contract, self.catalog) if contract else None
                    if audit and any(issue["code"] not in _PENDING_PREFIX_ISSUES for issue in audit["blocking_issues"]):
                        raise WorkflowValidationError("incremental step irreversibly violates its fixed caller contract")
                    steps = prefix["steps"]
                    prefix_audit = {"temporary_audit_outputs": prefix["outputs"], "requirements_audit": audit,
                                    "executable_plan": False, "final_outputs_selected": False}
                    receipt.update(status="accepted", action="arguments", operation=selected, generated_symbol=step["id"])
                    accepted, feedback = True, None
                    break
                except (ValueError, KeyError, TypeError):
                    receipt["error_code"] = "invalid_arguments_binding_or_prefix_contract"
                    feedback = {"code": receipt["error_code"], "instruction": "Choose valid literal arguments and explicit prior-symbol bindings for the selected operation. Accepted steps are unchanged."}
            if not accepted:
                return result("budget_exhausted", reason="decision_attempt_limit")
        return result("budget_exhausted", reason="model_call_limit")
