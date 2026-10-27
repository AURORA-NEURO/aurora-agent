"""Optional, bounded decoder hints compiled from reviewed schemas and caller-owned links.

This is a repository-defined application contract, not a blueprint capability. A provider
may implement only part of JSON Schema; the SDK runtime also does not validate every emitted
keyword. These hints never replace workflow compilation, exact requirement audits or resolved
argument checks. They neither generate a plan nor grant model/source execution authority.

Not implemented: arbitrary schema algebra, inferred natural-language requirements, provider
capability discovery, proof of source identity/graph correctness, or a generic fallback after
an unsupported transformation. Positional arrays require explicit caller capability knowledge.
"""
from __future__ import annotations

from dataclasses import dataclass
import json
import re
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .errors import ArgumentError
from .workflow_integrations import WorkflowOperationCatalog
from .workflow_requirements import WorkflowRequirements, validate_workflow_requirements

WORKFLOW_DECODER_SCHEMA = "aurora-workflow-decoder-schema/0.1"
MAX_WORKFLOW_DECODER_SCHEMA_BYTES = 256_000
MAX_WORKFLOW_DECODER_NODES = 20_000
MAX_WORKFLOW_DECODER_TUPLE_ITEMS = 128
MAX_WORKFLOW_DECODER_TUPLE_LENGTHS = 8
_STEP = r"[a-zA-Z][a-zA-Z0-9_-]{0,63}"
_INDEX = re.compile(r"0|[1-9][0-9]*")
_UNSUPPORTED = frozenset({"allOf", "anyOf", "oneOf", "not", "format"})
_OBJECT_KEYWORDS = frozenset({"properties", "required", "additionalProperties", "minProperties", "maxProperties"})
_ARRAY_KEYWORDS = frozenset({"items", "minItems", "maxItems", "uniqueItems"})


class WorkflowDecoderSchemaError(ArgumentError):
    """An opt-in decoder transformation cannot preserve its declared constraints."""


@dataclass(frozen=True, slots=True)
class WorkflowDecoderSchema:
    _encoded: str

    def to_dict(self) -> dict[str, Any]:
        return json.loads(self._encoded)

    @property
    def response_schema(self) -> dict[str, Any]:
        return self.to_dict()["response_schema"]

    @property
    def audit(self) -> dict[str, Any]:
        value = self.to_dict()
        return {key: child for key, child in value.items() if key != "response_schema"}


def _fail(code: str) -> None:
    raise WorkflowDecoderSchemaError(code)


def _tokens(pointer: str) -> tuple[str, ...]:
    return tuple(token.replace("~1", "/").replace("~0", "~") for token in pointer.split("/")[1:]) if pointer else ()


def _reference(pointer: str | None) -> dict[str, Any]:
    # The generic form deliberately leaves RFC 6901 escape validation to the compiler.
    suffix = re.escape(pointer) if pointer is not None else r"(/.*)?"
    return {"type": "object", "properties": {"$ref": {
        "type": "string", "pattern": "^" + _STEP + "#" + suffix + "$", "maxLength": 4225}},
        "required": ["$ref"], "additionalProperties": False}


def _trie_insert(root: dict[str, Any], path: str, link: Mapping[str, str]) -> None:
    node = root
    tokens = _tokens(path)
    if len(tokens) > 32:
        _fail("decoder_target_path_depth_bound")
    for token in tokens:
        if "leaf" in node:
            _fail("decoder_reference_parent_child_conflict")
        node = node.setdefault("children", {}).setdefault(token, {})
    if node.get("children"):
        _fail("decoder_reference_parent_child_conflict")
    if "leaf" in node:
        prior = node["leaf"]
        if prior["source_operation"] != link["source_operation"]:
            _fail("decoder_reference_source_identity_conflict")
        if "source_pointer" in prior and "source_pointer" in link and prior["source_pointer"] != link["source_pointer"]:
            _fail("decoder_reference_pointer_conflict")
        node["leaf"] = {**prior, **link}
    else:
        node["leaf"] = dict(link)


def _bound(schema: Mapping[str, Any], name: str, default: int) -> int:
    value = schema.get(name, default)
    if type(value) is not int or value < 0:
        _fail("decoder_invalid_container_bound")
    return value


def _adapt(schema: Any, overlay: Mapping[str, Any], budget: list[int], positional: list[dict[str, Any]],
           *, prefix_supported: bool, path: str = "", depth: int = 0, allow_reference: bool = True) -> Any:
    budget[0] -= 1
    if budget[0] < 0 or depth > 32:
        _fail("decoder_schema_traversal_bound")
    if schema is False:
        _fail("decoder_false_input_schema_unsupported")
    if schema is True:
        schema = {}
    if not isinstance(schema, Mapping) or set(schema).intersection(_UNSUPPORTED):
        _fail("decoder_input_schema_feature_unsupported")
    kind = schema.get("type")
    if isinstance(kind, list) or kind not in {None, "object", "array", "string", "integer", "number", "boolean", "null"}:
        _fail("decoder_input_type_unsupported")
    object_keywords = set(schema).intersection(_OBJECT_KEYWORDS)
    array_keywords = set(schema).intersection(_ARRAY_KEYWORDS)
    if kind is None and (object_keywords or array_keywords):
        _fail("decoder_untyped_container_keywords_unsupported")
    if (object_keywords and kind != "object") or (array_keywords and kind != "array"):
        _fail("decoder_container_keywords_conflict_with_declared_type")
    if "leaf" in overlay:
        return _reference(overlay["leaf"].get("source_pointer"))
    children = overlay.get("children", {})
    result = json.loads(canonical_json(dict(schema)))
    if children and ("const" in schema or "enum" in schema):
        _fail("decoder_container_constant_with_reference_descendants")
    if children and kind not in {"object", "array"}:
        _fail("decoder_target_path_type_ambiguous_or_scalar")
    if kind == "object" or "properties" in schema:
        properties = schema.get("properties", {})
        if not isinstance(properties, Mapping) or "$ref" in properties:
            _fail("decoder_reserved_or_invalid_input_property")
        additional = schema.get("additionalProperties", True)
        if not isinstance(additional, (Mapping, bool)):
            _fail("decoder_invalid_additional_properties")
        required_value = schema.get("required", [])
        if (not isinstance(required_value, list) or any(not isinstance(name, str) for name in required_value)
                or len(set(required_value)) != len(required_value)):
            _fail("decoder_invalid_required_properties")
        required = list(required_value)
        if _bound(schema, "minProperties", 0) > _bound(schema, "maxProperties", MAX_WORKFLOW_DECODER_NODES):
            _fail("decoder_container_bounds_conflict")
        if additional is False and set(required)-set(properties):
            _fail("decoder_required_property_excluded_by_closed_schema")
        if additional is False and _bound(schema, "minProperties", 0) > len(properties):
            _fail("decoder_closed_object_minimum_unsatisfiable")
        output_properties = {}
        for name in dict.fromkeys([*properties, *children]):
            if name not in properties and additional is False:
                _fail("decoder_target_path_excluded_by_closed_schema")
            child = properties[name] if name in properties else additional
            child_path = path + "/" + name.replace("~", "~0").replace("/", "~1")
            output_properties[name] = _adapt(child, children.get(name, {}), budget, positional,
                prefix_supported=prefix_supported, path=child_path, depth=depth+1)
            if name in children and name not in required:
                required.append(name)
        if len(required) > _bound(schema, "maxProperties", MAX_WORKFLOW_DECODER_NODES):
            _fail("decoder_required_properties_exceed_bound")
        result.update(type="object", properties=output_properties, required=required)
        if isinstance(additional, Mapping):
            result["additionalProperties"] = _adapt(additional, {}, budget, positional,
                prefix_supported=prefix_supported, path=path+"/*", depth=depth+1)
        else:
            result["additionalProperties"] = additional
    elif kind == "array":
        item_schema = schema.get("items", True)
        if _bound(schema, "minItems", 0) > _bound(schema, "maxItems", MAX_WORKFLOW_DECODER_NODES):
            _fail("decoder_container_bounds_conflict")
        if children:
            if not prefix_supported:
                _fail("decoder_prefix_items_capability_required")
            if any(_INDEX.fullmatch(token) is None for token in children):
                _fail("decoder_target_array_index_invalid")
            minimum = max(_bound(schema, "minItems", 0), max(int(token) for token in children)+1)
            if "maxItems" not in schema:
                _fail("decoder_positional_array_finite_maximum_required")
            maximum = _bound(schema, "maxItems", MAX_WORKFLOW_DECODER_TUPLE_ITEMS)
            if minimum > maximum:
                _fail("decoder_target_array_index_exceeds_bound")
            if maximum > MAX_WORKFLOW_DECODER_TUPLE_ITEMS or maximum-minimum+1 > MAX_WORKFLOW_DECODER_TUPLE_LENGTHS:
                _fail("decoder_positional_array_bound")
            variants = []
            for count in range(minimum, maximum+1):
                # Separate bounded tuples avoid ambiguous provider handling of homogeneous
                # items beside prefixItems. Their union retains every admitted native length.
                variant = dict(result)
                variant.pop("items", None)
                variant["prefixItems"] = [_adapt(item_schema, children.get(str(index), {}), budget, positional,
                    prefix_supported=prefix_supported, path=path+"/"+str(index), depth=depth+1) for index in range(count)]
                variant.update(minItems=count, maxItems=count)
                variants.append(variant)
            result = variants[0] if len(variants) == 1 else {"anyOf": variants}
            positional.append({"argument_path": path, "admitted_lengths": list(range(minimum, maximum+1)),
                "native_min_items": schema.get("minItems", 0), "native_max_items": schema.get("maxItems"),
                "strategy": "all_lengths_in_bounded_native_range;each_slot_retains_native_item_schema"})
        else:
            result["items"] = _adapt(item_schema, {}, budget, positional,
                prefix_supported=prefix_supported, path=path+"/*", depth=depth+1)
    elif children:
        _fail("decoder_target_path_type_ambiguous_or_scalar")
    if allow_reference and not children:
        return {"anyOf": [result, _reference(None)]}
    return result


def prepare_workflow_decoder_schema(
    catalog: WorkflowOperationCatalog, operation_ids: Sequence[str], *,
    requirements: WorkflowRequirements | Mapping[str, Any] | None = None, max_steps: int = 16,
    prefix_items_supported: bool = False, max_schema_bytes: int = MAX_WORKFLOW_DECODER_SCHEMA_BYTES,
) -> WorkflowDecoderSchema:
    """Prepare provider hints and explicit coverage, refusing unsupported transformations.

    This is pure metadata compilation. The capability boolean is application-supplied
    knowledge, never an automatic probe or a claim about all provider versions. Returned
    schema admission is not a workflow acceptance decision.
    """
    if not isinstance(catalog, WorkflowOperationCatalog):
        _fail("decoder_typed_catalog_required")
    if (not isinstance(operation_ids, (list, tuple)) or not 1 <= len(operation_ids) <= 128
            or any(not isinstance(name, str) for name in operation_ids) or len(set(operation_ids)) != len(operation_ids)):
        _fail("decoder_operation_allowlist_invalid")
    if type(prefix_items_supported) is not bool:
        _fail("decoder_prefix_capability_must_be_boolean")
    if type(max_steps) is not int or not 1 <= max_steps <= 32:
        _fail("decoder_step_bound_invalid")
    if type(max_schema_bytes) is not int or not 1 <= max_schema_bytes <= MAX_WORKFLOW_DECODER_SCHEMA_BYTES:
        _fail("decoder_schema_byte_bound_invalid")
    operations = [catalog.resolve(name) for name in operation_ids]
    if any(operation.read_only is not True for operation in operations):
        _fail("decoder_read_only_operations_required")
    contract = validate_workflow_requirements(requirements, catalog).to_dict() if requirements is not None else None
    if contract and (set(contract["required_operations"])-set(operation_ids) or set(contract["forbidden_operations"]).intersection(operation_ids)):
        _fail("decoder_operation_contract_conflict")
    if contract:
        max_steps = min(max_steps, contract["max_steps"])
        if len(contract["required_operations"]) > max_steps:
            _fail("decoder_required_operations_exceed_step_bound")
    overlays: dict[str, dict[str, Any]] = {name: {} for name in operation_ids}
    links = []
    for index, link in enumerate(contract["data_links"] if contract else []):
        if "target_path" in link:
            _trie_insert(overlays[link["target_operation"]], link["target_path"], link)
            links.append({"link_index": index, "target_operation": link["target_operation"], "target_path": link["target_path"],
                "decoder_hint": "reference_only_leaf_and_exact_suffix" if "source_pointer" in link else "reference_only_leaf",
                "source_pointer": link.get("source_pointer"), "source_identity": "compiler_only"})
        else:
            links.append({"link_index": index, "decoder_hint": "not_encoded", "reason": "no_exact_target_path;compiler_only"})
    budget, positional, branches = [MAX_WORKFLOW_DECODER_NODES], [], []
    for operation in operations:
        prior_arrays = len(positional)
        arguments = _adapt(operation.to_dict()["input_schema"], overlays[operation.operation_id], budget, positional,
            prefix_supported=prefix_items_supported)
        for row in positional[prior_arrays:]:
            row["operation_id"] = operation.operation_id
        branches.append({"type": "object", "properties": {
            "id": {"type": "string", "pattern": "^"+_STEP+"$"},
            "operation": {"const": operation.operation_id}, "arguments": arguments,
            "depends_on": {"type": "array", "maxItems": max_steps, "items": {"type": "string", "pattern": "^"+_STEP+"$"}}},
            "required": ["id", "operation", "arguments", "depends_on"], "additionalProperties": False})
    response = {"type": "object", "properties": {
        "name": {"type": "string", "minLength": 1, "maxLength": 160},
        "steps": {"type": "array", "minItems": 1, "maxItems": max_steps, "items": {"anyOf": branches}},
        "outputs": {"type": "array", "minItems": 1, "maxItems": max_steps,
                    "items": {"type": "string", "pattern": "^"+_STEP+"$"}}},
        "required": ["name", "steps", "outputs"], "additionalProperties": False}
    # Match ProviderRequest's actual schema serialization bound, including ASCII escaping.
    size = len(json.dumps(response, allow_nan=False).encode("utf-8"))
    if size > max_schema_bytes:
        _fail("decoder_schema_byte_bound_exceeded")
    descriptor = {"schema": WORKFLOW_DECODER_SCHEMA, "catalog_digest": catalog.catalog_digest,
        "operation_ids": list(operation_ids), "requirements_digest": contract["requirements_digest"] if contract else None,
        "response_schema": response, "response_schema_digest": content_digest(response), "response_schema_bytes": size,
        "coverage": {"link_hints": links, "positional_array_strategy": positional,
            "prefix_items_capability": "caller_declared" if prefix_items_supported else "not_declared",
            "compiler_only": ["source_operation_identity", "dependency_closure_and_cycles", "required_operations_and_call_caps",
                "argument_predicates", "resolved_value_types_and_pointer_existence", "RFC6901_escapes_in_generic_references",
                "full_native_input_schema_semantics"],
            "runtime_schema_validation": "partial;anyOf_const_pattern_prefixItems_and_other_constraints_not_fully_checked",
            "interpretation": "provider_decoder_hints_only;workflow_compiler_and_requirement_audit_are_authority"}}
    descriptor["decoder_digest"] = content_digest(descriptor)
    return WorkflowDecoderSchema(canonical_json(descriptor))


__all__ = ["WORKFLOW_DECODER_SCHEMA", "MAX_WORKFLOW_DECODER_SCHEMA_BYTES", "WorkflowDecoderSchemaError",
           "WorkflowDecoderSchema", "prepare_workflow_decoder_schema"]
