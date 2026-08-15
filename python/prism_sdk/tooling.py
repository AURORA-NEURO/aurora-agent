"""Schema-aware invocation for the complete live MCP tool catalogue.

The Rust server owns every domain schema and every scientific or safety decision.  This module
does not duplicate those decisions.  It turns the authoritative ``tools/list`` definitions into a
bounded, digestable catalogue, performs conservative JSON-shape checks before transport, and
keeps unsupported schema features visible as warnings rather than treating them as validated.
That gives callers a useful checked path for tools that do not yet have a handwritten Python
convenience method without turning transport validation into domain approval.
"""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .authoring import canonical_bytes, content_digest
from .errors import ArgumentError


TOOL_CATALOGUE_SCHEMA = "bioprism-python-tool-catalogue/0.1"
MAX_TOOL_DEFINITIONS = 512
MAX_TOOL_SCHEMA_BYTES = 1_000_000
MAX_TOOL_CATALOGUE_BYTES = 20_000_000
MAX_TOOL_ARGUMENT_DEPTH = 100
MAX_TOOL_NAME_BYTES = 256

_IGNORED_SCHEMA_KEYWORDS = {
    "$comment",
    "$id",
    "$schema",
    "default",
    "description",
    "examples",
    "title",
}
_SUPPORTED_SCHEMA_KEYWORDS = {
    "additionalProperties",
    "allOf",
    "anyOf",
    "const",
    "enum",
    "exclusiveMaximum",
    "exclusiveMinimum",
    "format",
    "items",
    "maxItems",
    "maxLength",
    "maxProperties",
    "maximum",
    "minItems",
    "minLength",
    "minProperties",
    "minimum",
    "not",
    "oneOf",
    "pattern",
    "properties",
    "required",
    "type",
    "uniqueItems",
}


class ToolSchemaError(ArgumentError):
    """A checked tool call failed the live input-schema shape boundary."""


@dataclass(frozen=True)
class ToolDefinition:
    """One authoritative MCP tool definition retained without domain reinterpretation."""

    name: str
    input_schema: Mapping[str, Any]
    description: str = ""

    def __post_init__(self) -> None:
        if not isinstance(self.name, str) or not self.name.strip():
            raise ArgumentError("tool definition name must be a non-empty string")
        if len(self.name.encode("utf-8")) > MAX_TOOL_NAME_BYTES:
            raise ArgumentError(f"tool definition name exceeds {MAX_TOOL_NAME_BYTES} bytes")
        if any(ord(character) < 32 for character in self.name):
            raise ArgumentError("tool definition name must not contain control characters")
        if not isinstance(self.input_schema, Mapping):
            raise ArgumentError("tool definition inputSchema must be a JSON object")
        if not isinstance(self.description, str):
            raise ArgumentError("tool definition description must be a string")
        try:
            encoded = canonical_bytes(dict(self.input_schema))
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"tool definition inputSchema is not canonical JSON: {error}") from error
        if len(encoded) > MAX_TOOL_SCHEMA_BYTES:
            raise ArgumentError(f"tool definition inputSchema exceeds {MAX_TOOL_SCHEMA_BYTES} bytes")

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "ToolDefinition":
        if not isinstance(value, Mapping):
            raise ArgumentError("tool definition must be a mapping")
        name = value.get("name")
        schema = value.get("inputSchema")
        if not isinstance(name, str):
            raise ArgumentError("tool definition requires a string name")
        if not isinstance(schema, Mapping):
            raise ArgumentError(f"tool definition {name!r} requires an object inputSchema")
        description = value.get("description", "")
        return cls(name, dict(schema), description)

    @property
    def schema_digest(self) -> str:
        """Digest of the exact input schema, not a digest of caller arguments or results."""

        return content_digest(dict(self.input_schema))

    def to_mapping(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "description": self.description,
            "inputSchema": dict(self.input_schema),
        }


@dataclass(frozen=True)
class ToolValidationIssue:
    """One local shape finding; it carries no claim about the remote domain result."""

    path: str
    code: str
    message: str


@dataclass(frozen=True)
class ToolValidationReport:
    """Conservative preflight output for one tool call."""

    tool: str
    schema_digest: str
    issues: tuple[ToolValidationIssue, ...] = ()
    warnings: tuple[ToolValidationIssue, ...] = ()

    @property
    def ok(self) -> bool:
        return not self.issues

    @property
    def fully_checked(self) -> bool:
        """Whether this report used only schema features understood by this SDK."""

        return self.ok and not self.warnings

    def raise_if_invalid(self) -> None:
        if self.issues:
            detail = "; ".join(f"{issue.path}: {issue.message}" for issue in self.issues)
            raise ToolSchemaError(f"tool {self.tool!r} arguments failed schema preflight: {detail}")


@dataclass(frozen=True)
class ToolCallPlan:
    """A checked, no-side-effect call plan that can be inspected before execution."""

    definition: ToolDefinition
    arguments: dict[str, Any]
    report: ToolValidationReport

    @property
    def tool(self) -> str:
        return self.definition.name

    @property
    def schema_digest(self) -> str:
        return self.definition.schema_digest

    def to_mcp_arguments(self) -> dict[str, Any]:
        return dict(self.arguments)


@dataclass(frozen=True)
class ToolCatalogue:
    """A bounded, duplicate-free snapshot of the server's live tool definitions."""

    definitions: tuple[ToolDefinition, ...]
    digest: str

    @classmethod
    def from_definitions(cls, values: Sequence[Mapping[str, Any] | ToolDefinition]) -> "ToolCatalogue":
        if isinstance(values, (str, bytes)) or not isinstance(values, Sequence):
            raise ArgumentError("tool definitions must be a sequence")
        if len(values) > MAX_TOOL_DEFINITIONS:
            raise ArgumentError(f"tool definitions may contain at most {MAX_TOOL_DEFINITIONS} items")
        definitions: list[ToolDefinition] = []
        names: set[str] = set()
        for index, value in enumerate(values):
            definition = value if isinstance(value, ToolDefinition) else ToolDefinition.from_mapping(value)
            if definition.name in names:
                raise ArgumentError(f"duplicate tool definition name: {definition.name}")
            names.add(definition.name)
            definitions.append(definition)
        payload = [definition.to_mapping() for definition in sorted(definitions, key=lambda item: item.name)]
        try:
            encoded = canonical_bytes(payload)
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"tool catalogue is not canonical JSON: {error}") from error
        if len(encoded) > MAX_TOOL_CATALOGUE_BYTES:
            raise ArgumentError(f"tool catalogue exceeds {MAX_TOOL_CATALOGUE_BYTES} bytes")
        return cls(tuple(definitions), content_digest(payload))

    def __post_init__(self) -> None:
        if len(self.definitions) > MAX_TOOL_DEFINITIONS:
            raise ArgumentError(f"tool definitions may contain at most {MAX_TOOL_DEFINITIONS} items")
        definitions = tuple(self.definitions)
        if any(not isinstance(definition, ToolDefinition) for definition in definitions):
            raise ArgumentError("tool catalogue definitions must be ToolDefinition values")
        names = [definition.name for definition in definitions]
        if len(names) != len(set(names)):
            raise ArgumentError("tool catalogue definitions must have unique names")
        payload = [definition.to_mapping() for definition in sorted(definitions, key=lambda item: item.name)]
        encoded = canonical_bytes(payload)
        if len(encoded) > MAX_TOOL_CATALOGUE_BYTES:
            raise ArgumentError(f"tool catalogue exceeds {MAX_TOOL_CATALOGUE_BYTES} bytes")
        expected_digest = content_digest(payload)
        if self.digest != expected_digest:
            raise ArgumentError("tool catalogue digest does not match its definitions")
        object.__setattr__(self, "definitions", definitions)

    def get(self, name: str) -> ToolDefinition:
        if not isinstance(name, str) or not name.strip():
            raise ArgumentError("tool name must be a non-empty string")
        for definition in self.definitions:
            if definition.name == name:
                return definition
        raise ToolSchemaError(f"tool {name!r} is absent from the live tools/list catalogue")

    def validate(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolValidationReport:
        definition = self.get(name)
        if arguments is None:
            arguments = {}
        if not isinstance(arguments, Mapping):
            return ToolValidationReport(
                name,
                definition.schema_digest,
                (ToolValidationIssue("$", "object_required", "tool arguments must be a JSON object"),),
            )
        issues: list[ToolValidationIssue] = []
        warnings: list[ToolValidationIssue] = []
        _check_schema_value(dict(arguments), definition.input_schema, "$", issues, warnings, 0)
        return ToolValidationReport(name, definition.schema_digest, tuple(issues), tuple(warnings))

    def plan(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolCallPlan:
        report = self.validate(name, arguments)
        report.raise_if_invalid()
        raw = {} if arguments is None else dict(arguments)
        try:
            canonical_bytes(raw)
        except (TypeError, ValueError) as error:
            raise ToolSchemaError(f"tool {name!r} arguments are not canonical JSON: {error}") from error
        return ToolCallPlan(self.get(name), raw, report)


def _check_schema_value(
    value: Any,
    schema: Any,
    path: str,
    issues: list[ToolValidationIssue],
    warnings: list[ToolValidationIssue],
    depth: int,
) -> bool:
    if depth > MAX_TOOL_ARGUMENT_DEPTH:
        issues.append(ToolValidationIssue(path, "nesting_limit", f"JSON nesting exceeds {MAX_TOOL_ARGUMENT_DEPTH} levels"))
        return False
    if schema is True:
        return True
    if schema is False:
        issues.append(ToolValidationIssue(path, "schema_false", "the authoritative schema rejects this value"))
        return False
    if not isinstance(schema, Mapping):
        issues.append(ToolValidationIssue(path, "invalid_schema", "schema branch is not a JSON object or boolean"))
        return False

    for keyword in schema:
        if keyword not in _SUPPORTED_SCHEMA_KEYWORDS and keyword not in _IGNORED_SCHEMA_KEYWORDS:
            warnings.append(ToolValidationIssue(path, "unsupported_schema_keyword", f"schema keyword {keyword!r} was not evaluated"))

    if "allOf" in schema:
        branches = schema["allOf"]
        if not isinstance(branches, list):
            issues.append(ToolValidationIssue(path, "invalid_allOf", "allOf must be an array"))
        else:
            for branch in branches:
                _check_schema_value(value, branch, path, issues, warnings, depth + 1)

    for combinator in ("anyOf", "oneOf"):
        if combinator not in schema:
            continue
        branches = schema[combinator]
        if not isinstance(branches, list) or not branches:
            issues.append(ToolValidationIssue(path, f"invalid_{combinator}", f"{combinator} must be a non-empty array"))
            continue
        matches = 0
        branch_warnings: list[ToolValidationIssue] = []
        for branch in branches:
            branch_issues: list[ToolValidationIssue] = []
            branch_local_warnings: list[ToolValidationIssue] = []
            _check_schema_value(value, branch, path, branch_issues, branch_local_warnings, depth + 1)
            if not branch_issues:
                matches += 1
                branch_warnings.extend(branch_local_warnings)
        if combinator == "anyOf" and matches == 0:
            issues.append(ToolValidationIssue(path, "anyOf_no_match", "value matched none of the schema alternatives"))
        elif combinator == "oneOf" and matches != 1:
            issues.append(ToolValidationIssue(path, "oneOf_cardinality", f"value matched {matches} schema alternatives, expected exactly one"))
        warnings.extend(branch_warnings)

    if "not" in schema:
        rejected_issues: list[ToolValidationIssue] = []
        rejected_warnings: list[ToolValidationIssue] = []
        _check_schema_value(value, schema["not"], path, rejected_issues, rejected_warnings, depth + 1)
        if not rejected_issues:
            issues.append(ToolValidationIssue(path, "not_rejected", "value matches a forbidden schema"))

    expected = schema.get("type")
    if expected is not None and not _matches_type(value, expected):
        issues.append(ToolValidationIssue(path, "type", f"expected JSON type {expected!r}, got {_json_type(value)}"))
        return False

    if "enum" in schema:
        choices = schema["enum"]
        if not isinstance(choices, list):
            issues.append(ToolValidationIssue(path, "invalid_enum", "enum must be an array"))
        elif not any(type(value) is type(choice) and value == choice for choice in choices):
            issues.append(ToolValidationIssue(path, "enum", "value is not one of the permitted enum members"))
    if "const" in schema:
        constant = schema["const"]
        if type(value) is not type(constant) or value != constant:
            issues.append(ToolValidationIssue(path, "const", "value does not equal the required constant"))

    if isinstance(value, Mapping):
        _check_object(value, schema, path, issues, warnings, depth)
    elif isinstance(value, list):
        _check_array(value, schema, path, issues, warnings, depth)
    elif isinstance(value, str):
        _check_string(value, schema, path, issues, warnings)
    elif isinstance(value, (int, float)) and not isinstance(value, bool):
        _check_number(value, schema, path, issues)
    return not issues


def _check_object(value: Mapping[str, Any], schema: Mapping[str, Any], path: str, issues: list[ToolValidationIssue], warnings: list[ToolValidationIssue], depth: int) -> None:
    required = schema.get("required", [])
    if not isinstance(required, list):
        issues.append(ToolValidationIssue(path, "invalid_required", "required must be an array"))
    else:
        for name in required:
            if not isinstance(name, str):
                issues.append(ToolValidationIssue(path, "invalid_required_member", "required members must be strings"))
            elif name not in value:
                issues.append(ToolValidationIssue(f"{path}.{name}", "required", "required property is missing"))
    properties = schema.get("properties", {})
    if properties is not None and not isinstance(properties, Mapping):
        issues.append(ToolValidationIssue(path, "invalid_properties", "properties must be an object"))
        properties = {}
    known = set(properties) if isinstance(properties, Mapping) else set()
    additional = schema.get("additionalProperties", True)
    for name, child in value.items():
        child_path = f"{path}.{name}" if isinstance(name, str) and name.isidentifier() else f"{path}[{name!r}]"
        if name in known:
            _check_schema_value(child, properties[name], child_path, issues, warnings, depth + 1)
        elif additional is False:
            issues.append(ToolValidationIssue(child_path, "additional_property", "property is not allowed by the schema"))
        elif isinstance(additional, (Mapping, bool)):
            _check_schema_value(child, additional, child_path, issues, warnings, depth + 1)
    _check_count(len(value), schema, path, "properties", issues)


def _check_array(value: list[Any], schema: Mapping[str, Any], path: str, issues: list[ToolValidationIssue], warnings: list[ToolValidationIssue], depth: int) -> None:
    _check_count(len(value), schema, path, "items", issues)
    if schema.get("uniqueItems") is True:
        encoded: set[bytes] = set()
        for index, item in enumerate(value):
            try:
                marker = canonical_bytes(item)
            except (TypeError, ValueError):
                continue
            if marker in encoded:
                issues.append(ToolValidationIssue(path, "uniqueItems", "array items must be unique"))
                break
            encoded.add(marker)
    if "items" in schema:
        item_schema = schema["items"]
        for index, item in enumerate(value):
            _check_schema_value(item, item_schema, f"{path}[{index}]", issues, warnings, depth + 1)


def _check_string(value: str, schema: Mapping[str, Any], path: str, issues: list[ToolValidationIssue], warnings: list[ToolValidationIssue]) -> None:
    _check_count(len(value), schema, path, "length", issues)
    pattern = schema.get("pattern")
    if pattern is not None:
        if not isinstance(pattern, str):
            warnings.append(ToolValidationIssue(path, "unsupported_pattern", "non-string pattern was not evaluated"))
        else:
            try:
                if re.search(pattern, value) is None:
                    issues.append(ToolValidationIssue(path, "pattern", "value does not match the schema pattern"))
            except re.error:
                warnings.append(ToolValidationIssue(path, "unsupported_pattern", "pattern is not valid in the Python regex engine"))


def _check_number(value: int | float, schema: Mapping[str, Any], path: str, issues: list[ToolValidationIssue]) -> None:
    for keyword, relation in (("minimum", lambda actual, bound: actual < bound), ("exclusiveMinimum", lambda actual, bound: actual <= bound), ("maximum", lambda actual, bound: actual > bound), ("exclusiveMaximum", lambda actual, bound: actual >= bound)):
        bound = schema.get(keyword)
        if isinstance(bound, (int, float)) and not isinstance(bound, bool) and relation(value, bound):
            issues.append(ToolValidationIssue(path, keyword, f"value violates {keyword}={bound}"))


def _check_count(value: int, schema: Mapping[str, Any], path: str, label: str, issues: list[ToolValidationIssue]) -> None:
    minimum_key = {"items": "minItems", "length": "minLength", "properties": "minProperties"}[label]
    maximum_key = {"items": "maxItems", "length": "maxLength", "properties": "maxProperties"}[label]
    minimum = schema.get(minimum_key)
    maximum = schema.get(maximum_key)
    if isinstance(minimum, int) and not isinstance(minimum, bool) and value < minimum:
        issues.append(ToolValidationIssue(path, minimum_key, f"count {value} is below {minimum}"))
    if isinstance(maximum, int) and not isinstance(maximum, bool) and value > maximum:
        issues.append(ToolValidationIssue(path, maximum_key, f"count {value} exceeds {maximum}"))


def _matches_type(value: Any, expected: Any) -> bool:
    expected_types = expected if isinstance(expected, list) else [expected]
    return any(
        (kind == "null" and value is None)
        or (kind == "object" and isinstance(value, Mapping))
        or (kind == "array" and isinstance(value, list))
        or (kind == "string" and isinstance(value, str))
        or (kind == "boolean" and isinstance(value, bool))
        or (kind == "integer" and isinstance(value, int) and not isinstance(value, bool))
        or (kind == "number" and isinstance(value, (int, float)) and not isinstance(value, bool))
        for kind in expected_types
    )


def _json_type(value: Any) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float):
        return "number"
    if isinstance(value, str):
        return "string"
    if isinstance(value, list):
        return "array"
    if isinstance(value, Mapping):
        return "object"
    return type(value).__name__


__all__ = [
    "MAX_TOOL_ARGUMENT_DEPTH",
    "MAX_TOOL_CATALOGUE_BYTES",
    "MAX_TOOL_DEFINITIONS",
    "MAX_TOOL_NAME_BYTES",
    "MAX_TOOL_SCHEMA_BYTES",
    "TOOL_CATALOGUE_SCHEMA",
    "ToolCallPlan",
    "ToolCatalogue",
    "ToolDefinition",
    "ToolSchemaError",
    "ToolValidationIssue",
    "ToolValidationReport",
]
