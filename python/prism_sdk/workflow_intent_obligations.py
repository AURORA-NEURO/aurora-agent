"""Caller-declared mandatory task obligations before model workflow authoring.

This repository-defined application contract is not a blueprint implementation or an NLP
interpreter. It retains unavailable and unresolved actions instead of dropping them from a
registered-operation contract. Admitted obligations lower to existing immutable requirements;
source authority, automatic extraction completeness and scientific validity are not established.
"""
from __future__ import annotations

from dataclasses import dataclass
import json
import re
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .domain_tools import _reject_secret_fields
from .errors import ArgumentError
from .workflow_builder import LocalWorkflowBuilder, WorkflowPlan, WorkflowValidationError
from .workflow_incremental_planner import IncrementalLocalWorkflowPlanner, IncrementalWorkflowResult
from .workflow_integrations import WorkflowOperationCatalog
from .workflow_requirements import (
    WorkflowRequirementError, WorkflowRequirements, compile_workflow_requirements,
    merge_workflow_requirements, validate_workflow_requirements,
)

WORKFLOW_INTENT_OBLIGATIONS_SCHEMA = "aurora-workflow-intent-obligations/0.1"
WORKFLOW_INTENT_ADMISSION_SCHEMA = "aurora-workflow-intent-admission/0.1"
MAX_WORKFLOW_INTENT_OBLIGATIONS = 128
MAX_WORKFLOW_INTENT_OBLIGATION_BYTES = 128_000
MAX_WORKFLOW_INTENT_ADMISSION_BYTES = 512_000
_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}$")
_BASIS = {"caller_supplied", "model_candidate"}


class WorkflowIntentObligationError(ArgumentError):
    """An obligation document or its exact task binding is invalid."""


def _safe(value: Any, maximum: int = MAX_WORKFLOW_INTENT_OBLIGATION_BYTES) -> Any:
    try:
        encoded = canonical_json(dict(value) if isinstance(value, Mapping) else value)
        if len(encoded.encode("utf-8")) > maximum:
            raise WorkflowIntentObligationError("workflow intent document exceeds its byte bound")
        result = json.loads(encoded)
        _reject_secret_fields(result)
        return result
    except (ValueError, TypeError, RecursionError, UnicodeError) as error:
        if isinstance(error, WorkflowIntentObligationError):
            raise
        raise WorkflowIntentObligationError("workflow intent requires bounded canonical JSON without secret fields") from error


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise WorkflowIntentObligationError("workflow intent encoding repeats an object key")
        result[key] = value
    return result


def _decode(encoded: Any, maximum: int) -> dict[str, Any]:
    try:
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > maximum:
            raise WorkflowIntentObligationError("workflow intent encoding is outside its byte bound")
        value = _safe(json.loads(encoded, object_pairs_hook=_unique_object), maximum)
        if not isinstance(value, dict):
            raise WorkflowIntentObligationError("workflow intent encoding requires an object")
        return value
    except (ValueError, TypeError, RecursionError, UnicodeError) as error:
        if isinstance(error, WorkflowIntentObligationError):
            raise
        raise WorkflowIntentObligationError("workflow intent encoding is invalid") from error


def _task(task: Any) -> str:
    try:
        valid = isinstance(task, str) and bool(task.strip()) and "\x00" not in task and len(task.encode("utf-8")) <= 32_000
    except UnicodeError:
        valid = False
    if not valid:
        raise WorkflowIntentObligationError("workflow intent needs a nonempty task within 32000 UTF-8 bytes")
    return task


def _id(value: Any) -> str:
    if not isinstance(value, str) or not _ID.fullmatch(value):
        raise WorkflowIntentObligationError("workflow intent identity is invalid")
    return value


def _has_reference(value: Any) -> bool:
    if isinstance(value, dict):
        return "$ref" in value or any(_has_reference(child) for child in value.values())
    return isinstance(value, list) and any(_has_reference(child) for child in value)


def _descriptor(value: Any, task: str) -> dict[str, Any]:
    value = _safe(value)
    if not isinstance(value, dict) or set(value) != {"schema", "task_digest", "interpretation_basis", "obligations"}:
        raise WorkflowIntentObligationError("workflow intent obligation fields are invalid")
    if value["schema"] != WORKFLOW_INTENT_OBLIGATIONS_SCHEMA or value["task_digest"] != content_digest({"task": task}):
        raise WorkflowIntentObligationError("workflow intent schema or exact task binding is invalid")
    if not isinstance(value["interpretation_basis"], str) or value["interpretation_basis"] not in _BASIS:
        raise WorkflowIntentObligationError("workflow intent basis must be caller_supplied or unreviewed model_candidate")
    rows = value["obligations"]
    if not isinstance(rows, list) or not 1 <= len(rows) <= MAX_WORKFLOW_INTENT_OBLIGATIONS:
        raise WorkflowIntentObligationError("workflow intent needs one to 128 mandatory obligations")
    seen: set[str] = set()
    for row in rows:
        if not isinstance(row, dict) or not {"id", "kind", "span"}.issubset(row):
            raise WorkflowIntentObligationError("workflow intent obligation must have an identity, kind and exact span")
        identity = _id(row["id"])
        if identity in seen:
            raise WorkflowIntentObligationError("workflow intent obligation identities must be unique")
        seen.add(identity)
        span = row["span"]
        if (not isinstance(span, dict) or set(span) != {"start", "end", "text"}
                or type(span["start"]) is not int or type(span["end"]) is not int
                or not 0 <= span["start"] < span["end"] <= len(task)
                or not isinstance(span["text"], str) or not span["text"].strip()
                or task[span["start"]:span["end"]] != span["text"]):
            raise WorkflowIntentObligationError("workflow intent span must select exact nonempty task text by Python character offsets")
        kind = row["kind"]
        base = {"id", "kind", "span"}
        if kind == "required_operation":
            if set(row) != base | {"operation", "output"} or type(row["output"]) is not bool:
                raise WorkflowIntentObligationError("required operation obligation fields are invalid")
            _id(row["operation"])
        elif kind == "argument_equals":
            if set(row) != base | {"operation", "path", "value"}:
                raise WorkflowIntentObligationError("argument equality obligation fields are invalid")
            _id(row["operation"])
            path = row["path"]
            if (not isinstance(path, str) or len(path) > 512 or (path and not path.startswith("/"))
                    or re.search(r"~(?![01])", path) or _has_reference(row["value"])):
                raise WorkflowIntentObligationError("argument obligation needs a bounded JSON pointer and literal value without $ref")
        elif kind == "unresolved_action":
            if (set(row) != base | {"reason"} or not isinstance(row["reason"], str)
                    or not row["reason"].strip() or "\x00" in row["reason"] or len(row["reason"].encode("utf-8")) > 512):
                raise WorkflowIntentObligationError("unresolved action obligation needs a bounded reason")
        else:
            raise WorkflowIntentObligationError("workflow intent obligation kind is unsupported")
    return value


@dataclass(frozen=True, slots=True)
class WorkflowIntentObligations:
    """Immutable caller declarations, not evidence of complete NLP interpretation."""

    _encoded: str

    def to_dict(self) -> dict[str, Any]:
        return _decode(self._encoded, MAX_WORKFLOW_INTENT_OBLIGATION_BYTES)

    @property
    def obligations_digest(self) -> str:
        return self.to_dict()["obligations_digest"]


def compile_workflow_intent_obligations(
    task: str, obligations: Sequence[Mapping[str, Any]], *, interpretation_basis: str = "caller_supplied",
) -> WorkflowIntentObligations:
    """Record mandatory declarations without extracting, approving or executing anything.

    Caller-supplied means caller-asserted, not independently verified human provenance. The
    host must classify raw model output as model_candidate; labels/digests cannot prevent host
    relabeling. No parser or human-reviewed flag is accepted. A model_candidate always blocks.
    """
    task = _task(task)
    if not isinstance(obligations, (list, tuple)):
        raise WorkflowIntentObligationError("workflow intent obligations must be an explicit list or tuple")
    descriptor = _descriptor({"schema": WORKFLOW_INTENT_OBLIGATIONS_SCHEMA,
        "task_digest": content_digest({"task": task}), "interpretation_basis": interpretation_basis,
        "obligations": list(obligations)}, task)
    value = _safe({**descriptor, "obligations_digest": content_digest(descriptor)})
    return WorkflowIntentObligations(canonical_json(value))


def validate_workflow_intent_obligations(
    value: WorkflowIntentObligations | Mapping[str, Any], *, task: str,
) -> WorkflowIntentObligations:
    task = _task(task)
    raw = value.to_dict() if isinstance(value, WorkflowIntentObligations) else _safe(value)
    if not isinstance(raw, dict):
        raise WorkflowIntentObligationError("workflow intent obligations require an object")
    descriptor = dict(raw)
    digest = descriptor.pop("obligations_digest", None)
    if digest != content_digest(descriptor):
        raise WorkflowIntentObligationError("workflow intent obligation digest does not match")
    descriptor = _descriptor(descriptor, task)
    return WorkflowIntentObligations(canonical_json({**descriptor, "obligations_digest": digest}))


@dataclass(frozen=True, slots=True)
class WorkflowIntentAdmission:
    """Bounded preflight findings; ready means constrained authoring may be attempted."""

    _encoded: str

    def to_dict(self) -> dict[str, Any]:
        return _decode(self._encoded, MAX_WORKFLOW_INTENT_ADMISSION_BYTES)

    @property
    def status(self) -> str:
        return self.to_dict()["status"]

    @property
    def requirements(self) -> WorkflowRequirements | None:
        contract = self.to_dict()["requirements"]
        return validate_workflow_requirements(contract) if contract is not None else None


class WorkflowIntentAdmissionError(WorkflowValidationError):
    """Mandatory declarations block authoring; the structured receipt remains available."""

    def __init__(self, admission: WorkflowIntentAdmission) -> None:
        super().__init__("workflow intent obligations block planning; inspect the admission receipt")
        self.admission = admission


def _literal_preflight(row: Mapping[str, Any], schema: Mapping[str, Any]) -> str:
    """Check only applicable direct top-level const/enum constraints, never satisfiability."""
    path = row["path"]
    if (schema.get("type") != "object" or any(key in schema for key in ("anyOf", "oneOf", "allOf", "if", "$ref"))
            or not path.startswith("/") or "/" in path[1:]):
        return "not_assessed"
    properties = schema.get("properties")
    name = path[1:].replace("~1", "/").replace("~0", "~")
    definition = properties.get(name) if isinstance(properties, dict) else None
    if not isinstance(definition, dict):
        return "not_assessed"
    actual = canonical_json(row["value"])
    if "const" in definition and actual != canonical_json(definition["const"]):
        return "definite_conflict"
    if "enum" in definition and isinstance(definition["enum"], list):
        if not any(actual == canonical_json(member) for member in definition["enum"]):
            return "definite_conflict"
    return "no_const_enum_conflict" if "const" in definition or "enum" in definition else "not_assessed"


def admit_workflow_intent_obligations(
    task: str, obligations: WorkflowIntentObligations | Mapping[str, Any], catalog: WorkflowOperationCatalog, *,
    operation_ids: Sequence[str] | None = None,
    requirements: WorkflowRequirements | Mapping[str, Any] | None = None, max_steps: int = 16,
) -> WorkflowIntentAdmission:
    """Retain blocking actions and conjoin admitted declarations with fixed requirements.

    Registration/selection checks do not establish live service availability or actual model
    visibility. The bound selection may contain operations the builder subsequently filters.
    General schema/contract satisfiability and complete task interpretation remain unassessed.
    """
    task = _task(task)
    if not isinstance(catalog, WorkflowOperationCatalog) or type(max_steps) is not int or not 1 <= max_steps <= 32:
        raise WorkflowIntentObligationError("workflow intent admission needs a catalogue and a step bound from one to 32")
    descriptor = validate_workflow_intent_obligations(obligations, task=task).to_dict()
    catalog_digest = catalog.catalog_digest
    if operation_ids is None:
        selected = [operation.operation_id for operation in catalog.operations()]
    else:
        if (not isinstance(operation_ids, (list, tuple)) or not 1 <= len(operation_ids) <= 128
                or any(not isinstance(item, str) or not _ID.fullmatch(item) for item in operation_ids)
                or len(set(operation_ids)) != len(operation_ids)):
            raise WorkflowIntentObligationError("workflow intent planning allowlist is invalid")
        selected = list(operation_ids)
        for operation in selected:
            catalog.resolve(operation)
    selected.sort()
    caller = validate_workflow_requirements(requirements, catalog,
        task_digest=descriptor["task_digest"]).to_dict() if requirements is not None else None
    issues: list[dict[str, Any]] = []
    literal_checks: list[dict[str, Any]] = []
    lowering: list[dict[str, Any]] = []
    required: list[str] = []
    outputs: list[str] = []
    rules: list[dict[str, Any]] = []
    if descriptor["interpretation_basis"] == "model_candidate":
        issues.append({"code": "unreviewed_model_candidate"})
    for row in descriptor["obligations"]:
        identity = row["id"]
        if row["kind"] == "unresolved_action":
            issues.append({"code": "unresolved_action", "obligation_id": identity})
            continue
        operation = row["operation"]
        try:
            native = catalog.resolve(operation)
        except ArgumentError:
            issues.append({"code": "operation_unregistered", "obligation_id": identity, "operation": operation})
            continue
        if operation not in selected:
            issues.append({"code": "operation_outside_selection", "obligation_id": identity, "operation": operation})
        if not native.read_only:
            issues.append({"code": "operation_not_read_only", "obligation_id": identity, "operation": operation})
        if operation not in required:
            required.append(operation)
        mapped = {"obligation_id": identity, "required_operation": operation}
        if row["kind"] == "required_operation":
            if row["output"] and operation not in outputs:
                outputs.append(operation)
            mapped["required_output"] = row["output"]
        else:
            rule = {"operation": operation, "path": row["path"], "comparison": "equals", "value": row["value"]}
            rule_id = content_digest(rule)
            if not any(content_digest(existing) == rule_id for existing in rules):
                rules.append(rule)
            mapped["argument_rule_digest"] = rule_id
            state = _literal_preflight(row, native.to_dict()["input_schema"])
            literal_checks.append({"obligation_id": identity, "state": state})
            if state == "definite_conflict":
                issues.append({"code": "literal_schema_conflict", "obligation_id": identity, "operation": operation, "path": row["path"]})
        lowering.append(mapped)
    contract = None
    if not issues:
        try:
            derived = compile_workflow_requirements(task, required_operations=required,
                output_operations=outputs, argument_rules=rules, max_steps=max_steps)
            # Even one derived contract goes through conjunction to catch direct conflicting
            # declarations; no caller predicate or tighter bound may be silently replaced.
            contract = merge_workflow_requirements(task, derived, *([caller] if caller else []), catalog=catalog).to_dict()
        except WorkflowRequirementError:
            issues.append({"code": "requirements_conflict"})
    if catalog.catalog_digest != catalog_digest:
        raise WorkflowIntentObligationError("workflow catalogue changed during intent admission")
    codes = {issue["code"] for issue in issues}
    status = ("needs_clarification" if codes & {"unreviewed_model_candidate", "unresolved_action"}
              else "blocked" if issues else "ready_for_constrained_authoring")
    receipt = {"schema": WORKFLOW_INTENT_ADMISSION_SCHEMA, "status": status,
        "task_digest": descriptor["task_digest"], "catalog_digest": catalog_digest,
        "obligations": descriptor, "operation_selection": {"selection": "all_registered" if operation_ids is None else "explicit",
            "operation_ids": selected, "scope": "registered_or_caller_selection_before_builder_filtering"},
        "actual_model_visibility": "unassessed",
        "caller_requirements_digest": caller["requirements_digest"] if caller else None,
        "max_steps": max_steps, "blocking_issues": issues, "lowering": lowering,
        "literal_schema_preflight": {"scope": "direct_top_level_const_and_enum_only", "checks": literal_checks},
        "general_satisfiability": "not_assessed", "task_interpretation_completeness": "unassessed",
        "declaration_provenance": "caller_asserted_not_authenticated;host_must_classify_raw_model_output",
        "scientific_validity": "unassessed", "provider_calls": 0, "source_dispatches": 0,
        "requirements": contract}
    value = _safe({**receipt, "admission_digest": content_digest(receipt)}, MAX_WORKFLOW_INTENT_ADMISSION_BYTES)
    return WorkflowIntentAdmission(canonical_json(value))


def validate_workflow_intent_admission(
    value: WorkflowIntentAdmission | Mapping[str, Any], *, task: str,
    obligations: WorkflowIntentObligations | Mapping[str, Any], catalog: WorkflowOperationCatalog,
    operation_ids: Sequence[str] | None = None,
    requirements: WorkflowRequirements | Mapping[str, Any] | None = None, max_steps: int = 16,
) -> WorkflowIntentAdmission:
    """Recompute exact declarations and bindings; a recomputable digest is not authority."""
    raw = value.to_dict() if isinstance(value, WorkflowIntentAdmission) else _safe(value, MAX_WORKFLOW_INTENT_ADMISSION_BYTES)
    expected = admit_workflow_intent_obligations(task, obligations, catalog,
        operation_ids=operation_ids, requirements=requirements, max_steps=max_steps)
    if canonical_json(raw) != expected._encoded:
        raise WorkflowIntentObligationError("workflow intent admission does not match the exact declarations and planning boundary")
    return expected


def build_workflow_with_obligations(
    builder: LocalWorkflowBuilder | IncrementalLocalWorkflowPlanner, task: str, *,
    obligations: WorkflowIntentObligations | Mapping[str, Any] | None = None, **planning_options: Any,
) -> WorkflowPlan | IncrementalWorkflowResult:
    """Gate an existing builder without changing its default mode or absent-feature bytes.

    The wrapper recomputes admission rather than accepting an externally approved receipt.
    Blocked obligations raise with a receipt before any provider invocation. Success returns
    the existing plan/result only; callers separately retain an admission receipt if desired.
    A plan grants no dispatch authority and certifies no unlisted natural-language obligation.
    """
    if not isinstance(builder, (LocalWorkflowBuilder, IncrementalLocalWorkflowPlanner)):
        raise WorkflowIntentObligationError("workflow intent wrapper requires an existing local workflow builder")
    if obligations is None:
        return builder.build(task, **planning_options)
    admission = admit_workflow_intent_obligations(task, obligations, builder.catalog,
        operation_ids=planning_options.get("operation_ids"), requirements=planning_options.get("requirements"),
        max_steps=planning_options.get("max_steps", 16))
    if admission.status != "ready_for_constrained_authoring":
        raise WorkflowIntentAdmissionError(admission)
    # Current checks are additive: historical admission0.1 bytes remain replayable.
    from .workflow_literal_policy import assess_workflow_literal_policy, WorkflowLiteralPolicyError
    current = assess_workflow_literal_policy(task, obligations, builder.catalog)
    if current.has_definite_conflicts:
        raise WorkflowLiteralPolicyError(current)
    return builder.build(task, **{**planning_options, "requirements": admission.requirements})
