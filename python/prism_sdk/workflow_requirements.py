"""Caller-owned task contracts for executable workflow fidelity.

This repository-defined contract separates the user's fixed requirements from a model's plan.
It checks exact operation/output identities, argument predicates, value-flow links and bounds.
It does not infer arbitrary natural-language intent or certify scientific correctness. A model
cannot loosen the contract during repair; the complete contract is bound into the plan digest.
"""
from __future__ import annotations

from dataclasses import dataclass
import json
import math
import re
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .domain_tools import _reject_secret_fields
from .errors import ArgumentError
from .workflow_integrations import WorkflowOperationCatalog

WORKFLOW_REQUIREMENTS_SCHEMA = "aurora-workflow-requirements/0.1"
WORKFLOW_REQUIREMENTS_SCHEMA_V02 = "aurora-workflow-requirements/0.2"
_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}$")
_STEP_ID = re.compile(r"^[a-zA-Z][a-zA-Z0-9_-]{0,63}$")
_REF = re.compile(r"^([a-zA-Z][a-zA-Z0-9_-]{0,63})#((?:/.*)?)$")
_FIELDS = {"schema", "task_digest", "required_operations", "forbidden_operations", "output_operations",
           "argument_rules", "data_links", "max_steps", "max_calls_by_operation"}


class WorkflowRequirementError(ArgumentError):
    """A task contract is invalid or a proposal violates a fixed requirement."""


def _safe(value: Any, *, maximum: int = 128_000) -> Any:
    try:
        # Canonical validation must precede JSON conversion: json.dumps otherwise coerces
        # integer object keys and silently changes the caller's equality predicate.
        encoded = canonical_json(dict(value) if isinstance(value, Mapping) else value)
        if len(encoded.encode("utf-8")) > maximum:
            raise WorkflowRequirementError("workflow requirements exceed their byte bound")
        result = json.loads(encoded)
        _reject_secret_fields(result)
        return result
    except (ArgumentError, TypeError, ValueError, RecursionError, UnicodeError) as error:
        if isinstance(error, WorkflowRequirementError):
            raise
        raise WorkflowRequirementError("workflow requirements must be bounded canonical JSON without secret fields") from error


def _id(value: Any) -> str:
    if not isinstance(value, str) or not _ID.fullmatch(value):
        raise WorkflowRequirementError("workflow requirement operation identity is invalid")
    return value


def _ids(value: Any) -> list[str]:
    if not isinstance(value, (list, tuple)) or len(value) > 128:
        raise WorkflowRequirementError("workflow requirement identities exceed their bound")
    if any(not isinstance(item, str) or not _ID.fullmatch(item) for item in value) or len(set(value)) != len(value):
        raise WorkflowRequirementError("workflow requirement identities are invalid or duplicated")
    return list(value)


def _pointer(value: Any) -> str:
    if not isinstance(value, str) or len(value) > 512 or (value and not value.startswith("/")) or re.search(r"~(?![01])", value):
        raise WorkflowRequirementError("workflow requirement path must be a bounded JSON pointer")
    return value


def _descriptor(value: Mapping[str, Any], catalog: WorkflowOperationCatalog | None = None) -> dict[str, Any]:
    if not isinstance(value, Mapping) or (catalog is not None and not isinstance(catalog, WorkflowOperationCatalog)):
        raise WorkflowRequirementError("workflow requirements need an object and an optional operation catalogue")
    value = _safe(value)
    if set(value) != _FIELDS or not isinstance(value["schema"], str) or value["schema"] not in {WORKFLOW_REQUIREMENTS_SCHEMA, WORKFLOW_REQUIREMENTS_SCHEMA_V02}:
        raise WorkflowRequirementError("workflow requirement fields or schema are invalid")
    if not isinstance(value["task_digest"], str) or not re.fullmatch(r"[0-9a-f]{64}", value["task_digest"]):
        raise WorkflowRequirementError("workflow requirements require an exact task digest")
    for key in ("required_operations", "forbidden_operations", "output_operations"):
        value[key] = _ids(value[key])
    required, forbidden = set(value["required_operations"]), set(value["forbidden_operations"])
    if required & forbidden or not set(value["output_operations"]).issubset(required):
        raise WorkflowRequirementError("workflow required, forbidden and output operations conflict")
    if type(value["max_steps"]) is not int or not 1 <= value["max_steps"] <= 32:
        raise WorkflowRequirementError("workflow requirement step bound is invalid")
    if len(required) > value["max_steps"]:
        raise WorkflowRequirementError("workflow step bound cannot admit every required operation")
    caps = value["max_calls_by_operation"]
    if not isinstance(caps, dict) or len(caps) > 128:
        raise WorkflowRequirementError("workflow operation call bounds are invalid")
    for operation, cap in caps.items():
        _ids([operation])
        if type(cap) is not int or not 0 <= cap <= 32 or (operation in required and cap == 0):
            raise WorkflowRequirementError("workflow operation call bound conflicts with its requirements")
    rules = value["argument_rules"]
    if not isinstance(rules, list) or len(rules) > 128:
        raise WorkflowRequirementError("workflow argument rule count is invalid")
    for rule in rules:
        if not isinstance(rule, dict) or set(rule) != {"operation", "path", "comparison", "value"}:
            raise WorkflowRequirementError("workflow argument rule fields are invalid")
        if _id(rule["operation"]) not in required:
            raise WorkflowRequirementError("argument rule operations must also be required")
        _pointer(rule["path"])
        if not isinstance(rule["comparison"], str) or rule["comparison"] not in {"equals", "contains", "at_most", "at_least"}:
            raise WorkflowRequirementError("workflow argument comparison is unsupported")
        if rule["comparison"] in {"at_most", "at_least"} and (
            type(rule["value"]) not in {int, float} or not math.isfinite(rule["value"])
        ):
            raise WorkflowRequirementError("workflow numeric requirement must be finite and nonboolean")
    links = value["data_links"]
    if not isinstance(links, list) or len(links) > 128:
        raise WorkflowRequirementError("workflow value-flow link count is invalid")
    for link in links:
        required_fields = {"source_operation", "target_operation"}
        optional_fields = {"source_pointer", "target_path"} if value["schema"] == WORKFLOW_REQUIREMENTS_SCHEMA_V02 else set()
        if not isinstance(link, dict) or not required_fields.issubset(link) or set(link) - required_fields - optional_fields:
            raise WorkflowRequirementError("workflow value-flow link fields are invalid")
        source, target = _id(link["source_operation"]), _id(link["target_operation"])
        if source not in required or target not in required or source == target:
            raise WorkflowRequirementError("workflow value-flow links require distinct required operations")
        for field in ("source_pointer", "target_path"):
            if field in link:
                _pointer(link[field])
    if len({content_digest(rule) for rule in rules}) != len(rules) or len({content_digest(link) for link in links}) != len(links):
        raise WorkflowRequirementError("workflow contract contains duplicated rules")
    if catalog is not None:
        for operation in required | forbidden | set(caps):
            try:
                catalog.resolve(operation)
            except ArgumentError as error:
                raise WorkflowRequirementError("workflow requirements name an unregistered operation") from error
    return value


@dataclass(frozen=True, slots=True)
class WorkflowRequirements:
    _encoded: str

    def to_dict(self) -> dict[str, Any]:
        if not isinstance(self._encoded, str):
            raise WorkflowRequirementError("workflow requirements encoding must be text")
        try:
            if len(self._encoded.encode("utf-8")) > 128_000:
                raise WorkflowRequirementError("workflow requirements encoding exceeds its byte bound")
            value = _safe(json.loads(self._encoded, object_pairs_hook=_unique_object))
        except (ValueError, TypeError, RecursionError, UnicodeError) as error:
            raise WorkflowRequirementError("workflow requirements encoding is invalid") from error
        if not isinstance(value, dict):
            raise WorkflowRequirementError("workflow requirements encoding must contain an object")
        return value

    @property
    def requirements_digest(self) -> str:
        result = self.to_dict().get("requirements_digest")
        if not isinstance(result, str) or not re.fullmatch(r"[0-9a-f]{64}", result):
            raise WorkflowRequirementError("workflow requirements digest is missing or invalid")
        return result


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise WorkflowRequirementError("workflow requirements encoding repeats an object key")
        result[key] = value
    return result


def compile_workflow_requirements(
    task: str, *, required_operations: Sequence[str] = (), forbidden_operations: Sequence[str] = (),
    output_operations: Sequence[str] = (), argument_rules: Sequence[Mapping[str, Any]] = (),
    data_links: Sequence[Mapping[str, str]] = (), max_steps: int = 32,
    max_calls_by_operation: Mapping[str, int] | None = None,
) -> WorkflowRequirements:
    """Bind caller constraints without granting execution authority.

    A data link with ``source_pointer`` or ``target_path`` uses schema 0.2 and must name an
    actual direct reference leaf. ``source_pointer`` selects the producer's output value;
    ``target_path`` selects its exact consumer argument location. Empty pointers select the
    root. Unconstrained schema-0.1 links retain their transitive value-flow semantics and bytes.
    """
    try:
        valid_task = isinstance(task, str) and bool(task.strip()) and len(task.encode("utf-8")) <= 32_000 and "\x00" not in task
    except UnicodeError:
        valid_task = False
    if not valid_task:
        raise WorkflowRequirementError("workflow requirements task text is outside its bound")
    for items in (required_operations, forbidden_operations, output_operations, argument_rules, data_links):
        if not isinstance(items, (list, tuple)) or len(items) > 128:
            raise WorkflowRequirementError("workflow requirement sequences must be explicit lists or tuples")
    if max_calls_by_operation is not None and not isinstance(max_calls_by_operation, Mapping):
        raise WorkflowRequirementError("workflow call bounds must be an explicit mapping")
    implied = _ids(required_operations) + _ids(output_operations)
    _ids(forbidden_operations)
    for rule in argument_rules:
        if not isinstance(rule, Mapping):
            raise WorkflowRequirementError("workflow argument rules must be objects")
        implied.append(_id(rule.get("operation")))
    for link in data_links:
        if not isinstance(link, Mapping):
            raise WorkflowRequirementError("workflow value-flow links must be objects")
        implied.extend([_id(link.get("source_operation")), _id(link.get("target_operation"))])
    required = list(dict.fromkeys(implied))
    schema = WORKFLOW_REQUIREMENTS_SCHEMA_V02 if any("source_pointer" in link or "target_path" in link for link in data_links) else WORKFLOW_REQUIREMENTS_SCHEMA
    descriptor = _descriptor({"schema": schema, "task_digest": content_digest({"task": task}),
        "required_operations": required, "forbidden_operations": list(forbidden_operations),
        "output_operations": list(output_operations), "argument_rules": [dict(item) for item in argument_rules],
        "data_links": [dict(item) for item in data_links], "max_steps": max_steps,
        "max_calls_by_operation": {} if max_calls_by_operation is None else dict(max_calls_by_operation)})
    result = {**descriptor, "requirements_digest": content_digest(descriptor)}
    return WorkflowRequirements(json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":")))


def validate_workflow_requirements(
    value: WorkflowRequirements | Mapping[str, Any], catalog: WorkflowOperationCatalog | None = None,
    *, task_digest: str | None = None,
) -> WorkflowRequirements:
    if task_digest is not None and (not isinstance(task_digest, str) or not re.fullmatch(r"[0-9a-f]{64}", task_digest)):
        raise WorkflowRequirementError("workflow requirements task binding must be a SHA-256 digest")
    raw = value.to_dict() if isinstance(value, WorkflowRequirements) else _safe(value)
    if not isinstance(raw, dict):
        raise WorkflowRequirementError("workflow requirements must be an object")
    descriptor = dict(raw)
    digest = descriptor.pop("requirements_digest", None)
    if digest != content_digest(descriptor):
        raise WorkflowRequirementError("workflow requirements digest does not match")
    descriptor = _descriptor(descriptor, catalog)
    if task_digest is not None and descriptor["task_digest"] != task_digest:
        raise WorkflowRequirementError("workflow requirements belong to a different task")
    return WorkflowRequirements(json.dumps({**descriptor, "requirements_digest": digest}, ensure_ascii=False, sort_keys=True, separators=(",", ":")))


def merge_workflow_requirements(
    task: str, *requirements: WorkflowRequirements | Mapping[str, Any],
    catalog: WorkflowOperationCatalog | None = None,
) -> WorkflowRequirements:
    """Conjoin fixed contracts without replacing defaults, predicates or tighter bounds.

    Intent-derived constraints and additional operator requirements have equal standing. A
    conflict refuses compilation; choosing one contract would silently discard caller intent.
    General logical satisfiability is not claimed, but direct equality/range conflicts fail here.
    """
    if not 1 <= len(requirements) <= 16:
        raise WorkflowRequirementError("workflow contract merge needs one to sixteen contracts")
    task_digest = compile_workflow_requirements(task).to_dict()["task_digest"]
    contracts = [validate_workflow_requirements(value, catalog, task_digest=task_digest).to_dict() for value in requirements]
    def unique(values: Any) -> list[Any]:
        result, seen = [], set()
        for item in values:
            digest = content_digest(item)
            if digest not in seen:
                seen.add(digest)
                result.append(item)
        return result
    fields = {key: unique(item for contract in contracts for item in contract[key])
              for key in ("required_operations", "forbidden_operations", "output_operations", "argument_rules", "data_links")}
    caps: dict[str, int] = {}
    for contract in contracts:
        for operation, cap in contract["max_calls_by_operation"].items():
            caps[operation] = min(caps.get(operation, cap), cap)
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for rule in fields["argument_rules"]:
        grouped.setdefault((rule["operation"], rule["path"]), []).append(rule)
    for rules in grouped.values():
        equalities = [rule["value"] for rule in rules if rule["comparison"] == "equals"]
        if equalities and any(not _matches(equalities[0], rule["comparison"], rule["value"]) for rule in rules):
            raise WorkflowRequirementError("workflow contracts contain conflicting argument predicates")
        lower = [rule["value"] for rule in rules if rule["comparison"] == "at_least"]
        upper = [rule["value"] for rule in rules if rule["comparison"] == "at_most"]
        if lower and upper and max(lower) > min(upper):
            raise WorkflowRequirementError("workflow contracts contain conflicting argument bounds")
    merged = compile_workflow_requirements(task, **fields,
        max_steps=min(contract["max_steps"] for contract in contracts), max_calls_by_operation=caps)
    return validate_workflow_requirements(merged, catalog, task_digest=task_digest)


def _reference_leaves(value: Any, depth: int = 0, path: str = "") -> list[tuple[str, str, str]]:
    if depth > 24:
        raise WorkflowRequirementError("workflow argument references exceed their nesting bound")
    if isinstance(value, dict):
        if "$ref" in value:
            if set(value) != {"$ref"} or not isinstance(value["$ref"], str) or len(value["$ref"]) > 4096:
                raise WorkflowRequirementError("workflow reference requires exactly one bounded string $ref")
            match = _REF.fullmatch(value["$ref"])
            if match is None or re.search(r"~(?![01])", match[2]):
                raise WorkflowRequirementError("workflow reference is not a valid step JSON pointer")
            return [(path, match[1], match[2])]
        return [leaf for key, child in value.items()
                for leaf in _reference_leaves(child, depth + 1, path + "/" + key.replace("~", "~0").replace("/", "~1"))]
    if isinstance(value, list):
        return [leaf for index, child in enumerate(value) for leaf in _reference_leaves(child, depth + 1, path + "/" + str(index))]
    return []


def _references(value: Any, depth: int = 0) -> set[str]:
    return {source for _, source, _ in _reference_leaves(value, depth)}


def _value_at(arguments: Any, pointer: str) -> tuple[str, Any]:
    current = arguments
    for token in pointer.split("/")[1:] if pointer else ():
        if isinstance(current, dict) and set(current) == {"$ref"}:
            return "deferred", None
        key = token.replace("~1", "/").replace("~0", "~")
        if isinstance(current, dict) and key in current:
            current = current[key]
        elif isinstance(current, list) and re.fullmatch(r"0|[1-9][0-9]*", key) and int(key) < len(current):
            current = current[int(key)]
        else:
            return "missing", None
    if isinstance(current, dict) and set(current) == {"$ref"}:
        _references(current)
        return "deferred", None
    if _references(current):
        return "partial", current
    return "present", current


def _matches(actual: Any, comparison: str, expected: Any) -> bool:
    if comparison == "equals":
        return content_digest(actual) == content_digest(expected)
    if comparison == "contains":
        if isinstance(actual, list):
            return any(content_digest(item) == content_digest(expected) for item in actual)
        return isinstance(actual, str) and isinstance(expected, str) and expected in actual
    if type(actual) not in {int, float} or not math.isfinite(actual):
        return False
    return actual <= expected if comparison == "at_most" else actual >= expected


def _partial_equals(actual: Any, expected: Any) -> bool | None:
    if isinstance(actual, dict) and set(actual) == {"$ref"}:
        return None
    if isinstance(actual, dict):
        if not isinstance(expected, dict) or set(actual) != set(expected):
            return False
        matches = [_partial_equals(actual[key], expected[key]) for key in actual]
    elif isinstance(actual, list):
        if not isinstance(expected, list) or len(actual) != len(expected):
            return False
        matches = [_partial_equals(item, expected[index]) for index, item in enumerate(actual)]
    else:
        return _matches(actual, "equals", expected)
    return False if any(match is False for match in matches) else None if any(match is None for match in matches) else True


def _predicate(state: str, actual: Any, comparison: str, expected: Any) -> bool | None:
    if state == "missing":
        return False
    if state == "deferred":
        return None
    if comparison == "equals":
        return _partial_equals(actual, expected)
    if comparison == "contains" and isinstance(actual, list):
        matches = [_partial_equals(item, expected) for item in actual]
        return True if any(match is True for match in matches) else None if any(match is None for match in matches) else False
    return _matches(actual, comparison, expected)


def audit_workflow_requirements(proposal: Mapping[str, Any], requirements: WorkflowRequirements | Mapping[str, Any],
                                catalog: WorkflowOperationCatalog | None = None) -> dict[str, Any]:
    contract = validate_workflow_requirements(requirements, catalog).to_dict()
    if not isinstance(proposal, Mapping):
        raise WorkflowRequirementError("workflow requirements audit needs a proposal object")
    proposal = _safe(proposal, maximum=500_000)
    if "task_digest" in proposal and proposal["task_digest"] != contract["task_digest"]:
        raise WorkflowRequirementError("workflow proposal belongs to a different task")
    steps = proposal.get("steps")
    if not isinstance(steps, list) or not 1 <= len(steps) <= 32 or not all(isinstance(step, dict) and set(step) == {"id", "operation", "arguments", "depends_on"}
            and isinstance(step["id"], str) and _STEP_ID.fullmatch(step["id"])
            and isinstance(step["operation"], str) and _ID.fullmatch(step["operation"])
            and isinstance(step["arguments"], dict) and isinstance(step["depends_on"], list)
            and all(isinstance(dependency, str) for dependency in step["depends_on"])
            and len(step["depends_on"]) <= 32 for step in steps):
        raise WorkflowRequirementError("workflow requirements audit needs a structurally checked proposal")
    indexed = {step["id"]: step for step in steps}
    if len(indexed) != len(steps):
        raise WorkflowRequirementError("workflow requirements audit received duplicate steps")
    references = {step_id: _references(step["arguments"]) for step_id, step in indexed.items()}
    ancestors: dict[str, set[str]] = {}
    pending = list(indexed)
    for step in steps:
        if len(set(step["depends_on"])) != len(step["depends_on"]) or any(dependency not in indexed or dependency == step["id"] for dependency in step["depends_on"]):
            raise WorkflowRequirementError("workflow requirements audit received invalid dependencies")
        if catalog is not None:
            try:
                catalog.resolve(step["operation"])
            except ArgumentError as error:
                raise WorkflowRequirementError("workflow proposal names an unregistered operation") from error
    while pending:
        ready = [step_id for step_id in pending if all(dependency in ancestors for dependency in indexed[step_id]["depends_on"])]
        if not ready:
            raise WorkflowRequirementError("workflow requirements audit received a cyclic dependency graph")
        for step_id in ready:
            ancestors[step_id] = set(indexed[step_id]["depends_on"])
            for dependency in indexed[step_id]["depends_on"]:
                ancestors[step_id].update(ancestors[dependency])
            if not references[step_id].issubset(ancestors[step_id]):
                raise WorkflowRequirementError("workflow argument reference is outside dependency closure")
            pending.remove(step_id)
    issues: list[dict[str, Any]] = []
    deferred: list[dict[str, Any]] = []
    counts = {operation: sum(step["operation"] == operation for step in steps) for operation in {step["operation"] for step in steps}}
    if len(steps) > contract["max_steps"]:
        issues.append({"code": "step_budget_exceeded"})
    for operation in contract["required_operations"]:
        if not counts.get(operation):
            issues.append({"code": "required_operation_missing", "operation": operation})
    for operation in contract["forbidden_operations"]:
        if counts.get(operation):
            issues.append({"code": "forbidden_operation_used", "operation": operation})
    for operation, cap in contract["max_calls_by_operation"].items():
        if counts.get(operation, 0) > cap:
            issues.append({"code": "operation_call_budget_exceeded", "operation": operation})
    outputs = proposal.get("outputs")
    if not isinstance(outputs, list) or not outputs or any(not isinstance(item, str) or item not in indexed for item in outputs) or len(set(outputs)) != len(outputs):
        raise WorkflowRequirementError("workflow requirements audit received unknown outputs")
    consumed = set(outputs)
    for output in outputs:
        consumed.update(ancestors[output])
    if consumed != set(indexed):
        raise WorkflowRequirementError("workflow requirements audit received unused steps")
    for operation in contract["output_operations"]:
        if not any(indexed[step_id]["operation"] == operation for step_id in outputs):
            issues.append({"code": "required_output_missing", "operation": operation})
    for index, rule in enumerate(contract["argument_rules"]):
        for step in steps:
            if step["operation"] == rule["operation"]:
                state, actual = _value_at(step["arguments"], rule["path"])
                row = {"rule_index": index, "step_id": step["id"], "operation": rule["operation"], "path": rule["path"]}
                matched = _predicate(state, actual, rule["comparison"], rule["value"])
                if matched is None:
                    deferred.append(row)
                elif not matched:
                    issues.append({"code": "argument_requirement_unsatisfied", **row})
    def flow_ancestors(step_id: str, seen: set[str]) -> set[str]:
        if step_id in seen or step_id not in indexed:
            return set()
        seen = seen | {step_id}
        refs = references[step_id]
        return refs | set().union(*(flow_ancestors(source, seen) for source in refs)) if refs else set()
    for index, link in enumerate(contract["data_links"]):
        for step in steps:
            if step["operation"] == link["target_operation"]:
                if "source_pointer" in link or "target_path" in link:
                    # A constrained link names the actual direct reference leaf. An unrelated
                    # reference, or a transformed ancestor, cannot stand in for that value.
                    matched = any(indexed[source]["operation"] == link["source_operation"]
                        and ("source_pointer" not in link or pointer == link["source_pointer"])
                        and ("target_path" not in link or path == link["target_path"])
                        for path, source, pointer in _reference_leaves(step["arguments"]))
                else:
                    flow = flow_ancestors(step["id"], set())
                    matched = any(indexed[source]["operation"] == link["source_operation"] for source in flow)
                if not matched:
                    issues.append({"code": "required_value_flow_missing", "link_index": index, "step_id": step["id"]})
    result = {"schema": "aurora-workflow-requirement-audit/0.1", "requirements_digest": contract["requirements_digest"],
              "proposal_digest": content_digest(proposal), "blocking_issues": issues, "deferred_arguments": deferred,
              "contract_satisfied": False if issues else None if deferred else True,
              "interpretation": "explicit_caller_contract_only;natural_language_and_scientific_validity_unverified"}
    return {**result, "audit_digest": content_digest(result)}


def enforce_workflow_step_requirements(operation: str, arguments: Mapping[str, Any], requirements: WorkflowRequirements | Mapping[str, Any]) -> None:
    contract = validate_workflow_requirements(requirements).to_dict()
    _id(operation)
    if not isinstance(arguments, Mapping):
        raise WorkflowRequirementError("resolved workflow arguments must be an object")
    arguments = _safe(arguments, maximum=2_000_000)
    if operation in contract["forbidden_operations"] or contract["max_calls_by_operation"].get(operation) == 0:
        raise WorkflowRequirementError("operation dispatch is forbidden by the fixed task contract")
    for rule in contract["argument_rules"]:
        if rule["operation"] == operation:
            state, actual = _value_at(arguments, rule["path"])
            if _predicate(state, actual, rule["comparison"], rule["value"]) is not True:
                raise WorkflowRequirementError("resolved workflow arguments violate the fixed task contract")
