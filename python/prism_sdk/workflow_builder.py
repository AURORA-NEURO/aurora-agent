"""Executable local-model workflow construction over reviewed integrations.

This repository-defined application contract composes the reviewed source adapters with the
existing LLMRuntime. It does not claim an unavailable blueprint module. Model output is an
untrusted proposal: exact operation identities, JSON schemas, dependency closure, budgets and
catalogue identity are checked before dispatch. Only read-only operations are admitted here.

Not implemented: effectful writes, distributed scheduling, exactly-once external effects,
clinical inference, or scientific validation of retrieved records. Checkpoints may explicitly
retain public source values for dependency replay; metadata projections do not retain them.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import os
from pathlib import Path
import re
import tempfile
from threading import Event
from typing import Any, Callable, Mapping, Sequence
from urllib.parse import urlsplit

from .authoring import content_digest
from .domain_tools import _json_safe, _reject_secret_fields
from .errors import ArgumentError
from .llm_runtime import LLMRuntime, ProviderRequest
from .tooling import ToolCatalogue, ToolDefinition
from .workflow_integrations import WorkflowOperationCatalog
from .workflow_requirements import (
    WorkflowRequirements, WorkflowRequirementError, audit_workflow_requirements,
    enforce_workflow_step_requirements, validate_workflow_requirements,
)


WORKFLOW_PLAN_SCHEMA = "aurora-executable-workflow-plan/0.1"
WORKFLOW_CONTRACT_PLAN_SCHEMA = "aurora-executable-workflow-plan/0.2"
WORKFLOW_CHECKPOINT_SCHEMA_V01 = "aurora-executable-workflow-checkpoint/0.1"
WORKFLOW_CHECKPOINT_SCHEMA = "aurora-executable-workflow-checkpoint/0.2"
MAX_WORKFLOW_STEPS = 32
MAX_WORKFLOW_VALUE_BYTES = 8_000_000
MAX_WORKFLOW_ATTEMPTS = 3
_ID = re.compile(r"^[a-zA-Z][a-zA-Z0-9_-]{0,63}$")
_REFERENCE = re.compile(r"^([a-zA-Z][a-zA-Z0-9_-]{0,63})#((?:/.*)?)$")
_CHECKPOINT_STATUSES = frozenset({"ready", "running", "paused", "approval_required", "retry_required", "attempt_limit", "blocked", "failed", "completed", "cancelled"})


class WorkflowValidationError(ArgumentError):
    """A proposal, retained value, or catalogue failed its execution contract."""


class WorkflowCancellationToken:
    """Caller-owned, one-way stop request; an admitted synchronous call must still drain.

    This follows the existing worker drain contract in ``_async_callback_bridge``. The token
    grants no source or retry authority and is never part of a model proposal or checkpoint.
    Use a fresh token when explicitly resuming a cancelled checkpoint.
    """

    __slots__ = ("_event",)

    def __init__(self) -> None:
        self._event = Event()

    def cancel(self) -> None:
        self._event.set()

    @property
    def cancellation_requested(self) -> bool:
        return self._event.is_set()


def _safe(value: Any, *, maximum: int = MAX_WORKFLOW_VALUE_BYTES) -> Any:
    result = _json_safe("workflow value", value, maximum=maximum)
    _reject_secret_fields(result)
    return result


def _encode(value: Any) -> str:
    return json.dumps(_safe(value), ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _seal(descriptor: Mapping[str, Any], key: str) -> dict[str, Any]:
    result = _safe(dict(descriptor))
    result[key] = content_digest(result)
    return result


def _verify(value: Mapping[str, Any], key: str) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise WorkflowValidationError("workflow artifact must be an object")
    result = _safe(dict(value))
    digest = result.pop(key, None)
    if digest != content_digest(result):
        raise WorkflowValidationError("workflow artifact digest does not match")
    return result


def _unique_json_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise WorkflowValidationError("workflow JSON contains duplicate object fields")
        result[key] = value
    return result


def _references(value: Any, depth: int = 0) -> list[tuple[str, str]]:
    if depth > 24:
        raise WorkflowValidationError("workflow argument nesting exceeds its bound")
    if isinstance(value, dict):
        if "$ref" in value:
            if set(value) != {"$ref"} or not isinstance(value["$ref"], str):
                raise WorkflowValidationError("workflow reference must contain only a string $ref")
            match = _REFERENCE.fullmatch(value["$ref"])
            if match is None or re.search(r"~(?![01])", match[2]):
                raise WorkflowValidationError("workflow reference must be step#/json/pointer")
            return [(match[1], match[2])]
        return [ref for child in value.values() for ref in _references(child, depth + 1)]
    if isinstance(value, list):
        return [ref for child in value for ref in _references(child, depth + 1)]
    return []


def _validate_schema_value(schema: Any, value: Any) -> None:
    """Wrap a value so the existing object-only tool validator checks scalar/array roots."""
    wrapper = {"type": "object", "properties": {"value": schema}, "required": ["value"], "additionalProperties": False}
    report = ToolCatalogue.from_definitions((ToolDefinition("workflow_value", wrapper),)).validate("workflow_value", {"value": value})
    if not report.fully_checked:
        raise WorkflowValidationError("workflow value fails its checked schema")


def _reference_schema(schema: Any, value: Any) -> Any:
    """Defer only reference leaves; validate every literal before any stage executes."""
    if isinstance(value, dict) and set(value) == {"$ref"}:
        return {}
    if isinstance(schema, bool):
        return schema
    result = dict(schema)
    contains_references = bool(_references(value))
    if contains_references:
        # Whole-value comparisons cannot establish equality until reference values are known.
        for keyword in ("const", "enum", "not", "uniqueItems"):
            result.pop(keyword, None)
    for keyword in ("allOf", "anyOf", "oneOf"):
        if keyword in result:
            result[keyword] = [_reference_schema(branch, value) for branch in result[keyword]]
        # Reference deferral can make distinct alternatives overlap before resolution.
        if keyword == "oneOf" and contains_references and keyword in result:
            result.setdefault("allOf", []).append({"anyOf": result.pop(keyword)})
    if isinstance(value, dict) and isinstance(schema.get("properties"), Mapping):
        result["properties"] = {
            key: _reference_schema(child, value[key]) if key in value else child
            for key, child in schema["properties"].items()
        }
    if isinstance(value, dict) and isinstance(schema.get("additionalProperties"), Mapping):
        properties = dict(result.get("properties", {}))
        for key, child in value.items():
            if key not in properties:
                properties[key] = _reference_schema(schema["additionalProperties"], child)
        result["properties"] = properties
    if isinstance(value, list) and "items" in schema:
        # Validate elements independently so one reference cannot exempt literal siblings.
        item_schema = schema["items"]
        for child in value:
            _validate_schema_value(_reference_schema(item_schema, child), child)
        if schema.get("uniqueItems") is True:
            _validate_schema_value({"type": "array", "uniqueItems": True}, [child for child in value if not _references(child)])
        result["items"] = {}
    return result


def _check_arguments(catalog: WorkflowOperationCatalog, operation: str, arguments: dict[str, Any]) -> dict[str, Any]:
    if not _references(arguments):
        return catalog.validate_arguments(operation, arguments)
    schema = catalog.resolve(operation).to_dict()["input_schema"]
    arguments = dict(arguments)
    if set(arguments) != {"$ref"}:
        for name, definition in schema.get("properties", {}).items():
            if name not in arguments and isinstance(definition, Mapping) and "default" in definition:
                arguments[name] = _safe(definition["default"])
    arguments = _safe(arguments)
    # ToolCatalogue requires object arguments. Arrays below object fields are handled above.
    _validate_schema_value(_reference_schema(schema, arguments), arguments)
    return arguments


def _proposal(value: Mapping[str, Any], catalog: WorkflowOperationCatalog) -> dict[str, Any]:
    result = _safe(value, maximum=500_000)
    if not isinstance(result, dict) or set(result) != {"name", "steps", "outputs"}:
        raise WorkflowValidationError("workflow proposal requires exactly name, steps, outputs")
    if not isinstance(result["name"], str) or not 1 <= len(result["name"]) <= 160:
        raise WorkflowValidationError("workflow name is outside its bound")
    steps = result["steps"]
    if not isinstance(steps, list) or not 1 <= len(steps) <= MAX_WORKFLOW_STEPS:
        raise WorkflowValidationError("workflow requires one to thirty-two steps")
    indexed: dict[str, dict[str, Any]] = {}
    for step in steps:
        if not isinstance(step, dict) or set(step) != {"id", "operation", "arguments", "depends_on"}:
            raise WorkflowValidationError("workflow step has invalid fields")
        step_id = step["id"]
        if not isinstance(step_id, str) or not _ID.fullmatch(step_id) or step_id in indexed:
            raise WorkflowValidationError("workflow step identity is invalid or duplicated")
        if not isinstance(step["operation"], str):
            raise WorkflowValidationError("workflow operation identity must be a string")
        operation = catalog.resolve(step["operation"])
        if operation.read_only is not True:
            raise WorkflowValidationError("this workflow executor admits only reviewed read-only operations")
        if not isinstance(step["arguments"], dict):
            raise WorkflowValidationError("workflow arguments must be an object")
        dependencies = step["depends_on"]
        if not isinstance(dependencies, list) or any(not isinstance(item, str) for item in dependencies):
            raise WorkflowValidationError("workflow dependencies must be step IDs")
        if len(set(dependencies)) != len(dependencies) or step_id in dependencies:
            raise WorkflowValidationError("workflow dependency is duplicated or self-referential")
        step["arguments"] = _check_arguments(catalog, step["operation"], step["arguments"])
        indexed[step_id] = step
    pending = list(indexed)
    ancestors: dict[str, set[str]] = {}
    while pending:
        ready = [key for key in pending if all(dep in ancestors for dep in indexed[key]["depends_on"])]
        if not ready:
            raise WorkflowValidationError("workflow contains a cycle or missing dependency")
        for key in ready:
            ancestors[key] = set(indexed[key]["depends_on"])
            for dep in indexed[key]["depends_on"]:
                ancestors[key].update(ancestors[dep])
            if any(source not in ancestors[key] for source, _ in _references(indexed[key]["arguments"])):
                raise WorkflowValidationError("workflow reference is outside dependency closure")
            pending.remove(key)
    outputs = result["outputs"]
    if not isinstance(outputs, list) or not outputs or any(not isinstance(item, str) or item not in indexed for item in outputs):
        raise WorkflowValidationError("workflow outputs must reference existing steps")
    if len(set(outputs)) != len(outputs):
        raise WorkflowValidationError("workflow outputs contain duplicates")
    consumed = set(outputs)
    for output in outputs:
        consumed.update(ancestors[output])
    if consumed != set(indexed):
        raise WorkflowValidationError("every workflow step must contribute to a declared output")
    return result


@dataclass(frozen=True, slots=True)
class WorkflowPlan:
    """An immutable canonical plan; execution revalidates its complete catalogue binding."""

    _encoded: str

    def to_dict(self) -> dict[str, Any]:
        return json.loads(self._encoded)

    @property
    def plan_digest(self) -> str:
        return self.to_dict()["plan_digest"]


def _check_requirements(proposal: Mapping[str, Any], requirements: WorkflowRequirements | Mapping[str, Any],
                        catalog: WorkflowOperationCatalog, task_digest: str) -> dict[str, Any]:
    contract = validate_workflow_requirements(requirements, catalog, task_digest=task_digest).to_dict()
    if audit_workflow_requirements(proposal, contract, catalog)["blocking_issues"]:
        raise WorkflowRequirementError("workflow proposal violates the fixed task contract")
    return contract


def compile_workflow_plan(proposal: Mapping[str, Any], catalog: WorkflowOperationCatalog, *, task: str,
                          requirements: WorkflowRequirements | Mapping[str, Any] | None = None) -> WorkflowPlan:
    if not isinstance(task, str) or not task.strip() or len(task.encode("utf-8")) > 32_000:
        raise WorkflowValidationError("workflow task is outside its text bound")
    checked = _proposal(proposal, catalog)
    task_digest = content_digest({"task": task})
    descriptor = {"schema": WORKFLOW_PLAN_SCHEMA, "task_digest": task_digest,
                  "catalog_digest": catalog.catalog_digest, **checked}
    if requirements is not None:
        descriptor["requirements"] = _check_requirements(checked, requirements, catalog, task_digest)
        descriptor["schema"] = WORKFLOW_CONTRACT_PLAN_SCHEMA
    value = _seal(descriptor, "plan_digest")
    return WorkflowPlan(_encode(value))


def validate_workflow_plan(value: WorkflowPlan | Mapping[str, Any], catalog: WorkflowOperationCatalog) -> WorkflowPlan:
    raw = value.to_dict() if isinstance(value, WorkflowPlan) else value
    descriptor = _verify(raw, "plan_digest")
    fields = {"schema", "task_digest", "catalog_digest", "name", "steps", "outputs"}
    contract_plan = descriptor.get("schema") == WORKFLOW_CONTRACT_PLAN_SCHEMA
    if set(descriptor) != fields | ({"requirements"} if contract_plan else set()):
        raise WorkflowValidationError("workflow plan fields are invalid")
    if descriptor["schema"] not in {WORKFLOW_PLAN_SCHEMA, WORKFLOW_CONTRACT_PLAN_SCHEMA} or descriptor["catalog_digest"] != catalog.catalog_digest:
        raise WorkflowValidationError("workflow plan schema or catalogue identity has changed")
    if not isinstance(descriptor["task_digest"], str) or not re.fullmatch("[0-9a-f]{64}", descriptor["task_digest"]):
        raise WorkflowValidationError("workflow task digest is invalid")
    checked = _proposal({key: descriptor[key] for key in ("name", "steps", "outputs")}, catalog)
    if contract_plan:
        _check_requirements(checked, descriptor["requirements"], catalog, descriptor["task_digest"])
    return WorkflowPlan(_encode(raw))


def _resolve(value: Any, results: Mapping[str, dict[str, Any]]) -> Any:
    if isinstance(value, dict) and set(value) == {"$ref"}:
        match = _REFERENCE.fullmatch(value["$ref"])
        assert match is not None
        if match[1] not in results:
            raise WorkflowValidationError("workflow reference has no completed source")
        found = results[match[1]]["value"]
        for token in match[2].split("/")[1:]:
            key = token.replace("~1", "/").replace("~0", "~")
            if isinstance(found, dict) and key in found:
                found = found[key]
            elif isinstance(found, list) and re.fullmatch(r"0|[1-9][0-9]*", key) and int(key) < len(found):
                found = found[int(key)]
            else:
                raise WorkflowValidationError("workflow reference pointer does not exist")
        return _safe(found)
    if isinstance(value, dict):
        return {key: _resolve(child, results) for key, child in value.items()}
    if isinstance(value, list):
        return [_resolve(child, results) for child in value]
    return value


@dataclass(frozen=True, slots=True)
class WorkflowRun:
    """Public source values are transient unless explicitly exported as a checkpoint."""

    _encoded: str

    @property
    def status(self) -> str:
        return json.loads(self._encoded)["status"]

    @property
    def checkpoint(self) -> dict[str, Any]:
        return json.loads(self._encoded)

    @property
    def outputs(self) -> dict[str, Any]:
        value = self.checkpoint
        return {key: value["results"][key]["value"] for key in value["output_ids"] if key in value["results"]}

    def to_dict(self, *, include_values: bool = False) -> dict[str, Any]:
        value = self.checkpoint
        if include_values:
            return value
        return {key: child for key, child in value.items() if key != "results"} | {
            "results": {key: {field: item for field, item in row.items() if field not in {"value", "receipt"}}
                        for key, row in value["results"].items()},
            "retention": "metadata_only;exported_checkpoint_retains_public_values",
        }


def _checkpoint(value: Mapping[str, Any], plan: dict[str, Any], catalog: WorkflowOperationCatalog) -> dict[str, Any]:
    result = _verify(value, "checkpoint_digest")
    legacy = result.get("schema") == WORKFLOW_CHECKPOINT_SCHEMA_V01
    fields = {"schema", "plan_digest", "catalog_digest", "status", "output_ids", "results", "attempts", "failure", "in_flight"}
    if set(result) != fields | (set() if legacy else {"cancel_requested"}):
        raise WorkflowValidationError("workflow checkpoint fields are invalid")
    if result["schema"] not in (WORKFLOW_CHECKPOINT_SCHEMA_V01, WORKFLOW_CHECKPOINT_SCHEMA) or result["plan_digest"] != plan["plan_digest"] or result["catalog_digest"] != catalog.catalog_digest:
        raise WorkflowValidationError("workflow checkpoint belongs to a different plan or catalogue")
    if legacy:
        if result["status"] == "cancelled":
            raise WorkflowValidationError("legacy workflow checkpoint cannot encode cancellation")
        result.update(schema=WORKFLOW_CHECKPOINT_SCHEMA, cancel_requested=False)
    if type(result["cancel_requested"]) is not bool or result["cancel_requested"] != (result["status"] == "cancelled"):
        raise WorkflowValidationError("workflow checkpoint cancellation state is inconsistent")
    if result["output_ids"] != plan["outputs"] or not isinstance(result["results"], dict) or not isinstance(result["attempts"], dict):
        raise WorkflowValidationError("workflow checkpoint result identity is invalid")
    if not isinstance(result["status"], str) or result["status"] not in _CHECKPOINT_STATUSES:
        raise WorkflowValidationError("workflow checkpoint status is invalid")
    steps = {step["id"]: step for step in plan["steps"]}
    if set(result["results"]) - set(steps) or set(result["attempts"]) - set(steps):
        raise WorkflowValidationError("workflow checkpoint contains an unknown step")
    for count in result["attempts"].values():
        if isinstance(count, bool) or not isinstance(count, int) or not 1 <= count <= MAX_WORKFLOW_ATTEMPTS:
            raise WorkflowValidationError("workflow checkpoint attempt count is invalid")
    if "requirements" in plan:
        for operation, cap in plan["requirements"]["max_calls_by_operation"].items():
            used = sum(result["attempts"].get(step_id, 0) for step_id, step in steps.items() if step["operation"] == operation)
            if used > cap:
                raise WorkflowRequirementError("workflow checkpoint exceeds its fixed operation call budget")
    for step_id, row in result["results"].items():
        if not isinstance(row, dict) or set(row) != {"value", "value_digest", "receipt", "receipt_digest", "arguments_digest"}:
            raise WorkflowValidationError("workflow retained result fields are invalid")
        if row["value_digest"] != content_digest(row["value"]) or row["receipt_digest"] != content_digest(row["receipt"]):
            raise WorkflowValidationError("workflow retained result digest does not match")
        step = steps[step_id]
        if not isinstance(row["value"], dict) or not isinstance(row["receipt"], dict):
            raise WorkflowValidationError("workflow retained values and receipts must be objects")
        _validate_schema_value(catalog.resolve(step["operation"]).to_dict()["output_schema"], row["value"])
        if any(dep not in result["results"] for dep in step["depends_on"]) or step_id not in result["attempts"]:
            raise WorkflowValidationError("workflow checkpoint omits completed dependencies or attempts")
        arguments = catalog.validate_arguments(step["operation"], _resolve(step["arguments"], result["results"]))
        if "requirements" in plan:
            enforce_workflow_step_requirements(step["operation"], arguments, plan["requirements"])
        if row["arguments_digest"] != content_digest(arguments):
            raise WorkflowValidationError("workflow retained arguments have changed")
    in_flight = result["in_flight"]
    if in_flight is not None:
        if not isinstance(in_flight, str) or in_flight not in steps or in_flight in result["results"] or in_flight not in result["attempts"]:
            raise WorkflowValidationError("workflow checkpoint in-flight step is invalid")
        if any(dep not in result["results"] for dep in steps[in_flight]["depends_on"]):
            raise WorkflowValidationError("workflow checkpoint in-flight dependency is incomplete")
        if result["status"] not in {"running", "retry_required", "approval_required", "cancelled"}:
            raise WorkflowValidationError("workflow checkpoint drops its unknown-completion boundary")
    failure = result["failure"]
    if failure is not None:
        if not isinstance(failure, dict) or set(failure) != {"step_id", "error_type"}:
            raise WorkflowValidationError("workflow checkpoint failure is invalid")
        failed_step = failure["step_id"]
        if not isinstance(failed_step, str) or failed_step not in steps or failed_step in result["results"] or not isinstance(failure["error_type"], str) or not _ID.fullmatch(failure["error_type"]):
            raise WorkflowValidationError("workflow checkpoint failure identity is invalid")
        if in_flight is not None or result["status"] not in {"blocked", "failed", "retry_required", "approval_required", "cancelled"}:
            raise WorkflowValidationError("workflow checkpoint failure state is inconsistent")
        if any(dep not in result["results"] for dep in steps[failed_step]["depends_on"]):
            raise WorkflowValidationError("workflow checkpoint failure dependency is incomplete")
        if result["status"] == "failed" and failed_step not in result["attempts"]:
            raise WorkflowValidationError("workflow checkpoint failure omits its dispatch attempt")
    elif result["status"] in {"blocked", "failed"}:
        raise WorkflowValidationError("workflow checkpoint failure status omits its failure")
    unfinished_attempts = set(result["attempts"]) - set(result["results"])
    permitted_unfinished = {in_flight} if in_flight is not None else {failure["step_id"]} if failure is not None else set()
    if result["status"] == "attempt_limit" or (result["status"] == "cancelled" and unfinished_attempts and not permitted_unfinished):
        if len(unfinished_attempts) != 1 or any(result["attempts"][key] != MAX_WORKFLOW_ATTEMPTS for key in unfinished_attempts):
            raise WorkflowValidationError("workflow checkpoint attempt-limit state is inconsistent")
    elif unfinished_attempts - permitted_unfinished:
        raise WorkflowValidationError("workflow checkpoint omits an unfinished execution boundary")
    if result["status"] == "completed" and (len(result["results"]) != len(steps) or failure is not None or in_flight is not None):
        raise WorkflowValidationError("workflow checkpoint completion is inconsistent")
    if result["status"] == "retry_required" and failure is None and in_flight is None:
        raise WorkflowValidationError("workflow checkpoint retry status omits its execution boundary")
    if result["status"] == "ready" and (result["results"] or result["attempts"] or failure is not None or in_flight is not None):
        raise WorkflowValidationError("workflow checkpoint ready state contains prior execution")
    return result


def execute_workflow_plan(
    plan: WorkflowPlan | Mapping[str, Any], catalog: WorkflowOperationCatalog, *, approved: bool = False,
    checkpoint: Mapping[str, Any] | None = None, max_step_calls: int = MAX_WORKFLOW_STEPS,
    retry_failed: bool = False, retrieved_at: str | None = None,
    on_checkpoint: Callable[[dict[str, Any]], None] | None = None,
    cancellation: WorkflowCancellationToken | None = None, resume_cancelled: bool = False,
) -> WorkflowRun:
    """Run a bounded DAG, pausing durably before each external boundary and after results.

    Completed steps are not called again. A failed or interrupted step requires explicit retry;
    retry does not turn a transport failure into an empty or successful evidence result.
    Cancellation prevents new dispatch at checked boundaries and drains an admitted synchronous
    call. A persisted cancellation requires a separate explicit resume grant and a fresh token.
    """
    checked = validate_workflow_plan(plan, catalog).to_dict()
    if type(approved) is not bool or type(retry_failed) is not bool or type(resume_cancelled) is not bool:
        raise WorkflowValidationError("workflow approvals and retry flags must be literal booleans")
    if cancellation is not None and not isinstance(cancellation, WorkflowCancellationToken):
        raise WorkflowValidationError("workflow cancellation must be a caller-owned cancellation token")
    if type(max_step_calls) is not int or not 1 <= max_step_calls <= MAX_WORKFLOW_STEPS:
        raise WorkflowValidationError("workflow step-call budget is outside its bound")
    if on_checkpoint is not None and not callable(on_checkpoint):
        raise WorkflowValidationError("workflow checkpoint callback must be callable")
    state = _checkpoint(checkpoint, checked, catalog) if checkpoint is not None else {
        "schema": WORKFLOW_CHECKPOINT_SCHEMA, "plan_digest": checked["plan_digest"],
        "catalog_digest": catalog.catalog_digest, "status": "ready", "output_ids": checked["outputs"],
        "results": {}, "attempts": {}, "failure": None, "in_flight": None, "cancel_requested": False,
    }

    def cancellation_pending() -> bool:
        if cancellation is not None and cancellation.cancellation_requested:
            state["cancel_requested"] = True
        return state["cancel_requested"]

    def snapshot(status: str, *, observe_cancellation: bool = True) -> WorkflowRun:
        if observe_cancellation and cancellation_pending():
            status = "cancelled"
        state["status"] = status
        run = WorkflowRun(_encode(_seal(state, "checkpoint_digest")))
        if on_checkpoint is not None:
            on_checkpoint(run.checkpoint)
        # A persistence callback may itself receive a caller stop request. Retain the actual
        # observation before recording that request; do not replace it with an empty result.
        if observe_cancellation and status != "cancelled" and cancellation_pending():
            return snapshot("cancelled")
        return run

    if state["cancel_requested"] and resume_cancelled and not (cancellation is not None and cancellation.cancellation_requested):
        state["cancel_requested"] = False
    if cancellation_pending():
        return snapshot("cancelled")
    if len(state["results"]) == len(checked["steps"]):
        return snapshot("completed")
    if state["failure"] is None and state["in_flight"] is None and set(state["attempts"]) - set(state["results"]):
        # A validated checkpoint can retain this boundary only at the hard attempt limit.
        # Un-cancelling it must not replace that boundary with an unresumable approval hold.
        return snapshot("attempt_limit")
    if not approved:
        return snapshot("approval_required")
    if (state["failure"] is not None or state["in_flight"] is not None) and not retry_failed:
        return snapshot("retry_required")
    for _ in range(max_step_calls):
        if catalog.catalog_digest != checked["catalog_digest"]:
            raise WorkflowValidationError("workflow catalogue changed during execution")
        if cancellation_pending():
            return snapshot("cancelled")
        ready = [step for step in checked["steps"] if step["id"] not in state["results"]
                 and all(dep in state["results"] for dep in step["depends_on"])]
        if not ready:
            break
        step = ready[0]
        step_id = step["id"]
        attempt = state["attempts"].get(step_id, 0) + 1
        if attempt > MAX_WORKFLOW_ATTEMPTS:
            state["failure"] = None
            state["in_flight"] = None
            return snapshot("attempt_limit")
        # Resolve and validate before changing dispatch state. Pointer/schema failures are holds.
        try:
            arguments = catalog.validate_arguments(step["operation"], _resolve(step["arguments"], state["results"]))
            if "requirements" in checked:
                enforce_workflow_step_requirements(step["operation"], arguments, checked["requirements"])
                cap = checked["requirements"]["max_calls_by_operation"].get(step["operation"])
                used = sum(state["attempts"].get(item["id"], 0) for item in checked["steps"]
                           if item["operation"] == step["operation"])
                if cap is not None and used >= cap:
                    raise WorkflowRequirementError("workflow dispatch would exceed its fixed operation call budget")
        except (ValueError, KeyError, TypeError) as error:
            state["in_flight"] = None
            state["failure"] = {"step_id": step_id, "error_type": type(error).__name__}
            return snapshot("blocked")
        prior_failure, prior_in_flight = state["failure"], state["in_flight"]
        state["failure"] = None
        state["attempts"][step_id] = attempt
        state["in_flight"] = step_id
        snapshot("running", observe_cancellation=False)  # A failed persistence callback prevents dispatch.
        if catalog.catalog_digest != checked["catalog_digest"]:
            raise WorkflowValidationError("workflow catalogue changed during checkpoint persistence")
        if cancellation_pending():
            # The persisted intent has not crossed the source boundary. Restore any earlier
            # failed/interrupted boundary so cancellation cannot implicitly authorize its retry.
            state["failure"], state["in_flight"] = prior_failure, prior_in_flight
            if attempt == 1:
                del state["attempts"][step_id]
            else:
                state["attempts"][step_id] = attempt - 1
            return snapshot("cancelled")
        try:
            observation = catalog.execute(step["operation"], arguments, approve_source_dispatch=True, retrieved_at=retrieved_at)
            value, receipt = _safe(observation.value), _safe(observation.receipt)
            row = {"value": value, "value_digest": content_digest(value), "receipt": receipt,
                   "receipt_digest": content_digest(receipt), "arguments_digest": content_digest(arguments)}
            _safe({**state["results"], step_id: row})
        except Exception as error:
            state["in_flight"] = None
            state["failure"] = {"step_id": step_id, "error_type": type(error).__name__}
            return snapshot("failed")
        state["results"][step_id] = row
        state["in_flight"] = None
        settled = snapshot("running")
        if state["cancel_requested"]:
            return settled
    return snapshot("completed" if len(state["results"]) == len(checked["steps"]) else "paused")


def save_workflow_checkpoint(path: str | Path, checkpoint: Mapping[str, Any]) -> None:
    """Atomically save an explicitly selected public-value checkpoint; refuse symlink targets."""
    target = Path(path)
    if target.is_symlink() or not target.parent.is_dir():
        raise WorkflowValidationError("workflow checkpoint requires an existing directory and regular target")
    encoded = _encode(checkpoint)
    temporary: str | None = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=target.parent, delete=False) as stream:
            temporary = stream.name
            stream.write(encoded)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, target)
        temporary = None
    finally:
        if temporary is not None:
            os.unlink(temporary)


def load_workflow_checkpoint(path: str | Path) -> dict[str, Any]:
    target = Path(path)
    if target.is_symlink() or not target.is_file() or target.stat().st_size > MAX_WORKFLOW_VALUE_BYTES:
        raise WorkflowValidationError("workflow checkpoint file is invalid or oversized")
    value = json.loads(target.read_text(encoding="utf-8"), object_pairs_hook=_unique_json_object)
    _verify(value, "checkpoint_digest")
    return value


class LocalWorkflowBuilder:
    """Turn natural language into a checked DAG through an explicitly registered local model."""

    def __init__(self, catalog: WorkflowOperationCatalog, runtime: LLMRuntime, *, model: str,
                 provider: str = "ollama", max_visible_operations: int = 32) -> None:
        if not isinstance(catalog, WorkflowOperationCatalog) or not isinstance(runtime, LLMRuntime):
            raise WorkflowValidationError("workflow builder requires a catalogue and LLMRuntime")
        if not isinstance(model, str) or not model.strip() or len(model) > 200:
            raise WorkflowValidationError("workflow builder requires a bounded local model identity")
        if type(max_visible_operations) is not int or not 1 <= max_visible_operations <= 128:
            raise WorkflowValidationError("workflow visible-operation budget is outside its bound")
        self.catalog, self.runtime, self.model, self.provider = catalog, runtime, model, provider
        self.max_visible_operations = max_visible_operations
        self._provider_digest = content_digest(self._local_provider_metadata())

    def _local_provider_metadata(self) -> dict[str, Any]:
        metadata = next((item for item in self.runtime.provider_metadata() if item.get("provider") == self.provider), None)
        if metadata is None or metadata.get("requires_credential") is not False:
            raise WorkflowValidationError("workflow planning requires an explicitly credentialless local provider")
        if type(metadata.get("max_attempts")) is not int or metadata["max_attempts"] != 1:
            raise WorkflowValidationError("workflow planning requires a provider configured for exactly one transport attempt")
        if metadata.get("transport") == "in_memory":
            return metadata
        endpoint = metadata.get("base_url")
        if metadata.get("transport") != "http" or not isinstance(endpoint, str) or urlsplit(endpoint).hostname not in {"127.0.0.1", "localhost", "::1"}:
            raise WorkflowValidationError("workflow planning provider must be loopback or in-memory")
        return metadata

    def build(self, task: str, *, approve_provider_call: bool = False,
              operation_ids: Sequence[str] | None = None, max_steps: int = 16,
              requirements: WorkflowRequirements | Mapping[str, Any] | None = None,
              max_planning_attempts: int = 1) -> WorkflowPlan:
        if approve_provider_call is not True:
            raise WorkflowValidationError("local-model workflow planning requires explicit provider approval")
        if content_digest(self._local_provider_metadata()) != self._provider_digest:
            raise WorkflowValidationError("workflow planning provider configuration changed")
        if not isinstance(task, str) or not task.strip() or len(task.encode("utf-8")) > 32_000:
            raise WorkflowValidationError("workflow task is outside its text bound")
        if type(max_steps) is not int or not 1 <= max_steps <= MAX_WORKFLOW_STEPS:
            raise WorkflowValidationError("workflow proposal step budget is outside its bound")
        if type(max_planning_attempts) is not int or not 1 <= max_planning_attempts <= 4:
            raise WorkflowValidationError("workflow planning attempts must be between one and four")
        contract = validate_workflow_requirements(requirements, self.catalog,
                    task_digest=content_digest({"task": task})).to_dict() if requirements is not None else None
        required = set(contract["required_operations"]) if contract else set()
        forbidden = set(contract["forbidden_operations"]) if contract else set()
        if contract:
            max_steps = min(max_steps, contract["max_steps"])
        if len(required) > min(max_steps, self.max_visible_operations):
            raise WorkflowValidationError("required operations exceed the planning or visible-operation budget")
        operations = list(self.catalog.operations())
        if operation_ids is not None:
            if (isinstance(operation_ids, (str, bytes)) or not isinstance(operation_ids, Sequence)
                    or not 1 <= len(operation_ids) <= 128
                    or any(not isinstance(item, str) or not item for item in operation_ids)
                    or len(set(operation_ids)) != len(operation_ids)):
                raise WorkflowValidationError("workflow operation allowlist is invalid")
            operations = [self.catalog.resolve(item) for item in operation_ids]
        available = {op.operation_id for op in operations if op.read_only and op.operation_id not in forbidden}
        if required - available:
            raise WorkflowValidationError("required operations are outside the reviewed planning allowlist")
        tokens = set(re.findall(r"[a-z0-9]+", task.lower()))
        operations.sort(key=lambda op: (op.operation_id not in required, -len(tokens & set(re.findall(r"[a-z0-9]+", op.operation_id + " " + op.description.lower()))), op.operation_id))
        operations = [op for op in operations if op.operation_id in available][:self.max_visible_operations]
        if not operations:
            raise WorkflowValidationError("no reviewed read-only workflow operation is available")
        visible = {op.operation_id for op in operations}
        digest = self.catalog.catalog_digest
        instructions = {
            "contract": "Return one JSON object with exactly name, steps, outputs. No markdown.",
            "format": {"name": "Short workflow name", "steps": [{"id": "s1", "operation": "exact reviewed operation_id", "arguments": {}, "depends_on": []}], "outputs": ["s1"]},
            "outputs": "nonempty list of final step IDs", "max_steps": max_steps,
            "references": "Use {\"$ref\":\"dependency_id#/json/pointer\"} only in arguments; dependency must be in depends_on or its ancestry. Root value is dependency_id#.",
            "reference_example": {"explanation": "For a consumer of a producer's complete output, replace the consumer's object argument with this reference and declare the producer dependency. Never invent an empty bundle or inline source result.",
                                  "consumer_arguments": {"bundle": {"$ref": "producer#"}}, "consumer_depends_on": ["producer"]},
            "rules": ["Use only listed operations and their exact argument schemas.", "Never invent source results, credentials, endpoints or tools.",
                      "This is public research metadata only. No clinical decisions or patient data.",
                      "A plan grants no execution authority. Omitted operations are unavailable in this planning pass."],
            "operations": [{key: op.to_dict()[key] for key in ("operation_id", "description", "input_schema")} for op in operations],
            "catalog_digest": digest, "omitted_operation_count": len(self.catalog.operations()) - len(operations),
        }
        if contract:
            instructions["fixed_caller_requirements"] = contract
            instructions["rules"].append("Satisfy every fixed caller requirement. Requirements are immutable; return only the proposal, never a replacement contract.")
            instructions["required_value_flow"] = [
                {**link, "instruction": "The target must consume a $ref to the source step's output; depends_on alone does not transfer data."}
                for link in contract["data_links"]
            ]
        schema = {
            "type": "object", "additionalProperties": False, "required": ["name", "steps", "outputs"],
            "properties": {
                "name": {"type": "string", "minLength": 1, "maxLength": 160},
                "steps": {"type": "array", "minItems": 1, "maxItems": max_steps, "items": {
                    "type": "object", "additionalProperties": False,
                    "required": ["id", "operation", "arguments", "depends_on"],
                    "properties": {"id": {"type": "string", "pattern": _ID.pattern}, "operation": {"type": "string", "enum": sorted(visible)},
                                   "arguments": {"type": "object"},
                                   "depends_on": {"type": "array", "items": {"type": "string", "pattern": _ID.pattern}}}}},
                "outputs": {"type": "array", "minItems": 1, "items": {"type": "string", "pattern": _ID.pattern}},
            },
        }
        messages = (
            {"role": "system", "content": json.dumps(instructions, ensure_ascii=False)},
            {"role": "user", "content": task},
        )
        feedback = None
        previous = None
        for attempt in range(max_planning_attempts):
            if self.catalog.catalog_digest != digest or content_digest(self._local_provider_metadata()) != self._provider_digest:
                raise WorkflowValidationError("workflow catalogue or provider changed while planning")
            repair_messages = (({"role": "assistant", "content": previous},) if previous else ()) + (({"role": "user", "content": json.dumps(feedback)},) if feedback else ())
            request = ProviderRequest(model=self.model, messages=messages + repair_messages,
                require_json=True, response_schema=schema,
                max_output_tokens=min(4096, max(512, 384 * max_steps)), temperature=0)
            # Transport failures and runtime structured-output refusals propagate. Only a
            # returned proposal can be repaired; no source tools execute during this loop.
            response = self.runtime.invoke(self.provider, request)
            if self.catalog.catalog_digest != digest or content_digest(self._local_provider_metadata()) != self._provider_digest:
                raise WorkflowValidationError("workflow catalogue or provider changed while planning")
            if response.provider != self.provider or response.model != self.model or response.tool_calls:
                raise WorkflowValidationError("workflow planning response changed its provider, model or tool boundary")
            proposal = None
            try:
                proposal = json.loads(response.text, object_pairs_hook=_unique_json_object)
                checked = _proposal(proposal, self.catalog)
                if len(checked["steps"]) > max_steps or any(step["operation"] not in visible for step in checked["steps"]):
                    raise WorkflowValidationError("local model exceeded its step or visible-operation boundary")
                return compile_workflow_plan(checked, self.catalog, task=task, requirements=contract)
            except (ValueError, KeyError, TypeError) as error:
                if attempt + 1 == max_planning_attempts:
                    raise WorkflowValidationError("local model failed the workflow proposal or fixed task contract") from error
                feedback = {"repair_attempt": attempt + 2, "validation_error_type": type(error).__name__,
                            "instruction": "Generate a complete corrected proposal. Preserve the original task, schemas and every fixed caller requirement. Check dependency references, final outputs and argument limits."}
                if type(error) is WorkflowValidationError:
                    feedback["structural_issue"] = str(error)
                previous = response.text if isinstance(response.text, str) and len(response.text.encode("utf-8")) <= 100_000 else None
                if contract and isinstance(proposal, dict) and all(key in proposal for key in ("name", "steps", "outputs")):
                    try:
                        feedback["requirement_issues"] = audit_workflow_requirements(proposal, contract, self.catalog)["blocking_issues"]
                    except (ValueError, KeyError, TypeError):
                        pass
        raise WorkflowValidationError("workflow planning exhausted its attempt budget")
