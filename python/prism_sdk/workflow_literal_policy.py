"""Additive current-policy scalar checks before new workflow authoring.

This repository-defined extension has no blueprint module identifier. It deliberately does
not change admission0.1, historical review/acceptance bytes or extraction request protocols.
An assessed leaf is conditional on its obligation-required presence, not proof of complete
arguments, schema satisfiability, task interpretation, source availability or authority.
"""
from __future__ import annotations

from dataclasses import dataclass
import math
from typing import Any, Mapping

from .authoring import canonical_json, content_digest
from .errors import ArgumentError
from .tooling import ToolCatalogue, ToolDefinition
from .workflow_builder import WorkflowValidationError
from .workflow_integrations import WorkflowOperationCatalog
from .workflow_intent_obligations import (
    WorkflowIntentObligations, _decode, _safe, _task, validate_workflow_intent_obligations,
)

WORKFLOW_LITERAL_POLICY_SCHEMA = "aurora-workflow-literal-policy/0.1"
MAX_WORKFLOW_LITERAL_POLICY_BYTES = 512_000
_ANNOTATIONS = {"$comment", "$id", "$schema", "default", "description", "examples", "title"}
_ROOT_KEYS = _ANNOTATIONS | {"type", "properties", "required", "additionalProperties"}
_BOUNDS = {"minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"}
_LEAF_KEYS = _ANNOTATIONS | {"type", "const", "enum"} | _BOUNDS
_TYPES = {"string", "boolean", "null", "integer", "number"}
_POLICY = {"id": "direct-declared-scalar-type-and-numeric-bounds/0.1",
    "root_keys": sorted(_ROOT_KEYS), "leaf_keys": sorted(_LEAF_KEYS), "scalar_types": sorted(_TYPES),
    "evaluated_constraints": ["type", *sorted(_BOUNDS)],
    "legacy_const_enum": "not_evaluated_here;admission0.1_unchanged",
    "path_scope": "one_declared_property;conditional_on_obligation_required_presence",
    "unknown_or_malformed": "not_assessed", "defaults": "never_applied"}
WORKFLOW_LITERAL_POLICY_DIGEST = content_digest(_POLICY)


@dataclass(frozen=True, slots=True)
class WorkflowLiteralPolicyReport:
    """Recomputable findings; no stored report or digest can authorize authoring."""

    _encoded: str

    def to_dict(self) -> dict[str, Any]:
        return _decode(self._encoded, MAX_WORKFLOW_LITERAL_POLICY_BYTES)

    @property
    def has_definite_conflicts(self) -> bool:
        return self.to_dict()["status"] == "current_policy_conflict"


class WorkflowLiteralPolicyError(WorkflowValidationError):
    """Current constraints block new authoring; historical admission stays unchanged."""

    def __init__(self, report: WorkflowLiteralPolicyReport) -> None:
        super().__init__("current_policy_conflict: inspect the bound literal policy report before new authoring")
        self.report = report


def _not_assessed(reason: str) -> dict[str, Any]:
    return {"state": "not_assessed", "reason": reason, "evaluated_schema": None, "diagnostics": []}


def _leaf(row: Mapping[str, Any], schema: Mapping[str, Any]) -> dict[str, Any]:
    path = row["path"]
    if not path.startswith("/") or "/" in path[1:]:
        return _not_assessed("not_a_direct_property_pointer")
    if schema.get("type") != "object" or set(schema) - _ROOT_KEYS:
        return _not_assessed("root_not_in_direct_object_scope")
    properties = schema.get("properties")
    required = schema.get("required", [])
    if (not isinstance(properties, dict) or not isinstance(required, list)
            or any(type(name) is not str for name in required) or len(set(required)) != len(required)
            or not isinstance(schema.get("additionalProperties", True), (dict, bool))):
        return _not_assessed("malformed_root_shape")
    name = path[1:].replace("~1", "/").replace("~0", "~")
    if name not in properties:
        return _not_assessed("property_not_explicitly_declared")
    definition = properties[name]
    if not isinstance(definition, dict) or set(definition) - _LEAF_KEYS:
        return _not_assessed("leaf_not_in_scalar_policy_scope")
    kind = definition.get("type")
    if type(kind) is not str or kind not in _TYPES:
        return _not_assessed("missing_or_nonscalar_or_union_type")
    if "enum" in definition and (type(definition["enum"]) is not list or not definition["enum"]):
        return _not_assessed("malformed_legacy_enum_shape")
    projection = {"type": kind}
    for key in sorted(_BOUNDS & set(definition)):
        bound = definition[key]
        if (type(bound) not in {int, float} or (type(bound) is float and not math.isfinite(bound))):
            return _not_assessed("malformed_numeric_bound")
        if kind not in {"integer", "number"}:
            return _not_assessed("numeric_bound_on_nonnumeric_declared_type")
        projection[key] = bound
    wrapper = {"type": "object", "properties": {"value": projection},
        "required": ["value"], "additionalProperties": False}
    try:
        report = ToolCatalogue.from_definitions((ToolDefinition("workflow_literal", wrapper),)).validate(
            "workflow_literal", {"value": row["value"]})
    except (ArgumentError, ValueError, TypeError, OverflowError, RecursionError) as error:
        return {**_not_assessed("native_leaf_validator_did_not_complete"), "validator_error_type": type(error).__name__}
    if report.warnings or any(issue.code not in {"type", *_BOUNDS} for issue in report.issues):
        return {**_not_assessed("unchecked_or_unexpected_native_leaf_diagnostic"),
            "diagnostics": [{"path": issue.path, "code": issue.code, "message": issue.message} for issue in report.issues],
            "warnings": [{"path": issue.path, "code": issue.code, "message": issue.message} for issue in report.warnings]}
    pointer = "/properties/" + name.replace("~", "~0").replace("/", "~1")
    return {"state": "definite_scalar_conflict" if report.issues else "no_evaluated_violation",
        "reason": "evaluated_type_and_numeric_projection_only", "evaluated_schema": projection,
        "leaf_schema_pointer": pointer, "leaf_schema_digest": content_digest(definition),
        "conditional_on_obligation_required_presence": True,
        "legacy_const_enum_assessment": "separate_admission0.1_unchanged",
        "diagnostics": [{"path": issue.path, "code": issue.code, "message": issue.message} for issue in report.issues]}


def assess_workflow_literal_policy(
    task: str, obligations: WorkflowIntentObligations | Mapping[str, Any], catalog: WorkflowOperationCatalog,
) -> WorkflowLiteralPolicyReport:
    """Assess only definitely resolved scalar leaves; unknown paths never become conflicts."""
    task = _task(task)
    if not isinstance(catalog, WorkflowOperationCatalog):
        raise WorkflowValidationError("literal policy requires a workflow operation catalogue")
    if content_digest(_POLICY) != WORKFLOW_LITERAL_POLICY_DIGEST:
        raise WorkflowValidationError("current literal policy definition changed")
    declarations = validate_workflow_intent_obligations(obligations, task=task).to_dict()
    catalog_digest = catalog.catalog_digest
    checks = []
    for row in declarations["obligations"]:
        if row["kind"] != "argument_equals":
            continue
        try:
            schema = catalog.resolve(row["operation"]).to_dict()["input_schema"]
        except ArgumentError:
            schema = None
        finding = _not_assessed("operation_unregistered") if schema is None else _leaf(row, schema)
        checks.append({"obligation_id": row["id"], "operation": row["operation"], "path": row["path"],
            "literal_digest": content_digest(row["value"]),
            "input_schema_digest": content_digest(schema) if schema is not None else None, **finding})
    if catalog.catalog_digest != catalog_digest:
        raise WorkflowValidationError("workflow catalogue changed during current literal policy assessment")
    if content_digest(_POLICY) != WORKFLOW_LITERAL_POLICY_DIGEST:
        raise WorkflowValidationError("current literal policy definition changed during assessment")
    counts = {state: sum(check["state"] == state for check in checks)
        for state in ("definite_scalar_conflict", "no_evaluated_violation", "not_assessed")}
    assessed = counts["definite_scalar_conflict"] + counts["no_evaluated_violation"]
    status = ("current_policy_conflict" if counts["definite_scalar_conflict"] else
        "not_assessed" if assessed == 0 else "partially_assessed" if counts["not_assessed"] else "no_evaluated_violation")
    report = {"schema": WORKFLOW_LITERAL_POLICY_SCHEMA, "status": status,
        "task_digest": declarations["task_digest"], "obligations_digest": declarations["obligations_digest"],
        "catalog_digest": catalog_digest, "policy": _POLICY, "policy_digest": WORKFLOW_LITERAL_POLICY_DIGEST,
        "checks": checks, "counts": counts, "no_literal_obligations": not checks,
        "general_satisfiability": "not_assessed", "whole_call_validity": "not_assessed",
        "task_interpretation_completeness": "unassessed", "scientific_validity": "unassessed",
        "historical_admission_and_acceptance": "not_rewritten_or_upgraded",
        "provider_calls": 0, "source_dispatches": 0, "plan_created": False}
    value = _safe({**report, "report_digest": content_digest(report)}, MAX_WORKFLOW_LITERAL_POLICY_BYTES)
    return WorkflowLiteralPolicyReport(canonical_json(value))


def validate_workflow_literal_policy_report(
    value: WorkflowLiteralPolicyReport | Mapping[str, Any], *, task: str,
    obligations: WorkflowIntentObligations | Mapping[str, Any], catalog: WorkflowOperationCatalog,
) -> WorkflowLiteralPolicyReport:
    """Replay current policy exactly; a self-restamped report is not a binding witness."""
    raw = value.to_dict() if isinstance(value, WorkflowLiteralPolicyReport) else _safe(value, MAX_WORKFLOW_LITERAL_POLICY_BYTES)
    expected = assess_workflow_literal_policy(task, obligations, catalog)
    if canonical_json(raw) != expected._encoded:
        raise WorkflowValidationError("current literal policy report does not match exact bindings and findings")
    return expected
