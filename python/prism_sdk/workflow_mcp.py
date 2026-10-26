"""Reviewed MCP tools as executable workflow operations over caller-owned stdio clients.

This repository-defined bridge makes no unavailable blueprint-coverage claim. Importing this
module opens no connection. Registration separately authorizes bounded tools/list discovery;
every call requires source approval and rechecks the connected session and reviewed definitions.
Server annotations are evidence to review, never authority to classify a tool as read-only.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import json
import math
import re
import secrets
import threading
from typing import Any, Mapping, Sequence
import weakref

from .authoring import content_digest
from .client import Client
from .domain_tools import _reject_secret_fields
from .models import ToolResult
from .tooling import MAX_TOOL_CATALOGUE_BYTES, MAX_TOOL_DEFINITIONS, ToolCatalogue, ToolDefinition
from .workflow_integrations import (
    MAX_WORKFLOW_ARGUMENT_BYTES, MAX_WORKFLOW_INTEGRATIONS, MAX_WORKFLOW_OPERATIONS,
    WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult,
)


MCP_WORKFLOW_SERVER_SCHEMA = "aurora-workflow-mcp-server-binding/0.1"
MAX_MCP_WORKFLOW_LIST_PAGES = 16
MAX_MCP_WORKFLOW_RESPONSE_BYTES = 4_000_000
_IDENTIFIER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$")


class McpWorkflowError(ValueError):
    """A reviewed server, tool contract, or response could not be established safely."""


@dataclass(repr=False)
class _SessionBinding:
    # Exact private equality snapshots avoid publishing digests of low-entropy credentials.
    settings: tuple[Any, ...] = field(repr=False)
    session: Any = field(repr=False)
    process: Any = field(repr=False)
    handshake_json: str = field(repr=False)
    nonce: str = field(default_factory=lambda: secrets.token_hex(32), repr=False)


_BINDINGS: weakref.WeakKeyDictionary[Client, _SessionBinding] = weakref.WeakKeyDictionary()
_BINDINGS_LOCK = threading.RLock()


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result = {}
    for key, value in pairs:
        if key in result:
            raise McpWorkflowError("MCP JSON contains duplicate object fields")
        result[key] = value
    return result


def _json(value: Any, *, maximum: int, label: str) -> Any:
    def check_keys(child: Any, depth: int = 0) -> None:
        if depth > 64:
            raise McpWorkflowError(f"{label} exceeds its nesting bound")
        if isinstance(child, Mapping):
            for key, nested in child.items():
                if not isinstance(key, str):
                    raise McpWorkflowError(f"{label} contains a non-string JSON object key")
                check_keys(nested, depth + 1)
        elif isinstance(child, (list, tuple)):
            for nested in child:
                check_keys(nested, depth + 1)
    try:
        check_keys(value)
        encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
        if len(encoded.encode("utf-8")) > maximum:
            raise McpWorkflowError(f"{label} exceeds its byte bound")
        return json.loads(encoded)
    except (TypeError, ValueError, RecursionError, UnicodeError):
        raise McpWorkflowError(f"{label} must be bounded finite JSON") from None


def _connected_identity(client: Client) -> tuple[dict[str, Any], _SessionBinding]:
    if not isinstance(client, Client):
        raise McpWorkflowError("MCP workflow requires a caller-owned Client")
    session = client.session
    process = client._process
    if not client.running or session is None or process is None:
        raise McpWorkflowError("MCP workflow client must already be connected and initialized")
    if (not isinstance(session.protocol_version, str) or not session.protocol_version
            or not isinstance(session.server_info, Mapping) or not isinstance(session.capabilities, Mapping)
            or not isinstance(session.raw, Mapping)):
        raise McpWorkflowError("MCP connected session identity is malformed")
    config = client.config
    if (isinstance(config.timeout, bool) or not isinstance(config.timeout, (int, float))
            or not math.isfinite(config.timeout) or not 0.1 <= config.timeout <= 120
            or type(config.max_frame_bytes) is not int or not 1 <= config.max_frame_bytes <= MAX_TOOL_CATALOGUE_BYTES):
        raise McpWorkflowError("MCP client timeout or frame bounds are outside the workflow contract")
    if (not isinstance(config.command, tuple) or not config.command
            or any(not isinstance(part, str) for part in config.command)
            or (config.cwd is not None and not isinstance(config.cwd, str))):
        raise McpWorkflowError("MCP client settings are invalid")
    env = client._env
    if env is not None and (not isinstance(env, Mapping)
                            or any(not isinstance(key, str) or not isinstance(value, str) for key, value in env.items())):
        raise McpWorkflowError("MCP client environment override settings are invalid")
    settings = (tuple(config.command), config.cwd, config.timeout, config.max_frame_bytes,
                tuple(sorted(env.items())) if env is not None else None)
    handshake = _json({"protocol_version": session.protocol_version, "server_info": session.server_info,
                       "capabilities": session.capabilities, "initialize_result": session.raw},
                      maximum=1_000_000, label="MCP server identity")
    try:
        _reject_secret_fields(handshake)
    except ValueError:
        raise McpWorkflowError("MCP handshake contains credential-shaped identity fields") from None
    if not isinstance(session.capabilities.get("tools"), Mapping):
        raise McpWorkflowError("MCP server has not declared the tools capability")
    handshake_json = json.dumps(handshake, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    with _BINDINGS_LOCK:
        binding = _BINDINGS.get(client)
        if (binding is None or binding.settings != settings or binding.session is not session
                or binding.process is not process or binding.handshake_json != handshake_json):
            binding = _SessionBinding(settings, session, process, handshake_json)
            _BINDINGS[client] = binding
        metadata = {
            "schema": MCP_WORKFLOW_SERVER_SCHEMA, "transport": "caller_owned_stdio_client",
            "binding_scope": "connected_client_session", "protocol_version": session.protocol_version,
            "server_info_digest": content_digest(handshake["server_info"]),
            "capabilities_digest": content_digest(handshake["capabilities"]),
            "transport_limits": {"timeout_seconds": config.timeout, "max_frame_bytes": config.max_frame_bytes},
        }
        metadata["server_identity_digest"] = content_digest({**metadata, "session_binding": binding.nonce})
    return metadata, binding


def mcp_workflow_server_identity(client: Client) -> dict[str, Any]:
    """Inspect a connected session without a request, exposing no argv or environment values.

    Identity is stable for the exact live client session. Reconnection or configuration changes
    produce a new opaque binding. This digest is a local binding, not server authentication.
    """
    if not isinstance(client, Client):
        raise McpWorkflowError("MCP workflow requires a caller-owned Client")
    with client._lock:
        metadata, _ = _connected_identity(client)
        return metadata


def _same_binding(client: Client, expected: _SessionBinding, digest: str) -> None:
    current, binding = _connected_identity(client)
    if binding is not expected or current["server_identity_digest"] != digest:
        raise McpWorkflowError("MCP connected session or configuration changed after review")


def _discover(client: Client) -> tuple[dict[str, dict[str, Any]], ToolCatalogue]:
    tools: list[dict[str, Any]] = []
    cursor: str | None = None
    seen: set[str] = set()
    for _ in range(MAX_MCP_WORKFLOW_LIST_PAGES):
        try:
            response = client.request("tools/list", {} if cursor is None else {"cursor": cursor})
        except Exception:
            raise McpWorkflowError("MCP tool discovery failed; remote details withheld") from None
        response = _json(response, maximum=MAX_TOOL_CATALOGUE_BYTES, label="MCP tool discovery")
        result = response.get("result") if isinstance(response, dict) else None
        if not isinstance(result, dict) or not isinstance(result.get("tools"), list):
            raise McpWorkflowError("MCP tools/list did not return a tools array")
        if any(not isinstance(tool, dict) for tool in result["tools"]):
            raise McpWorkflowError("MCP tools/list contains a non-object tool definition")
        tools.extend(result["tools"])
        if len(tools) > MAX_TOOL_DEFINITIONS:
            raise McpWorkflowError("MCP tools/list exceeds its tool-count bound")
        next_cursor = result.get("nextCursor")
        if next_cursor is None:
            break
        if not isinstance(next_cursor, str) or not next_cursor or len(next_cursor.encode("utf-8")) > 1024 or next_cursor in seen:
            raise McpWorkflowError("MCP tools/list pagination cursor is invalid or repeats")
        seen.add(next_cursor)
        cursor = next_cursor
    else:
        raise McpWorkflowError("MCP tools/list exceeds its page-count bound")
    tools = _json(tools, maximum=MAX_TOOL_CATALOGUE_BYTES, label="MCP tool catalogue")
    try:
        catalogue = ToolCatalogue.from_definitions(tools)
    except ValueError:
        raise McpWorkflowError("MCP tools/list definitions are invalid or duplicated") from None
    return {tool["name"]: tool for tool in tools}, catalogue


def _value(result: ToolResult, *, requires_structured: bool, maximum: int) -> tuple[dict[str, Any], str, int]:
    if not isinstance(result, ToolResult):
        raise McpWorkflowError("MCP tool call returned an invalid result type")
    envelope = _json(result.envelope, maximum=maximum, label="MCP tool response")
    if not isinstance(envelope, dict) or type(envelope.get("isError", False)) is not bool:
        raise McpWorkflowError("MCP tool result has an invalid error flag")
    if envelope.get("isError") is True:
        raise McpWorkflowError("MCP tool reported an execution error; remote details withheld")
    content = envelope.get("content", [])
    if not isinstance(content, list) or any(not isinstance(block, dict) for block in content):
        raise McpWorkflowError("MCP tool result content must contain objects")
    if "structuredContent" in envelope:
        value = envelope["structuredContent"]
        kind = "structured_content"
        omitted = len(content)
    else:
        if requires_structured:
            raise McpWorkflowError("MCP declared output schema requires structuredContent")
        if (len(content) != 1 or content[0].get("type") != "text"
                or not isinstance(content[0].get("text"), str)):
            raise McpWorkflowError("MCP legacy result requires one JSON object text block")
        try:
            value = json.loads(content[0]["text"], object_pairs_hook=_unique_object)
        except (ValueError, RecursionError):
            raise McpWorkflowError("MCP legacy text result is not unambiguous JSON") from None
        kind, omitted = "json_text_object", 0
    if not isinstance(value, dict):
        raise McpWorkflowError("MCP workflow results must be JSON objects")
    value = _json(value, maximum=maximum, label="MCP workflow value")
    try:
        _reject_secret_fields(value)
    except ValueError:
        raise McpWorkflowError("MCP workflow result contains credential-shaped fields") from None
    return value, kind, omitted


def register_mcp_workflow_operations(
    catalog: WorkflowOperationCatalog,
    client: Client,
    *,
    integration_id: str,
    reviewed_read_only_tools: Sequence[str],
    approve_tool_discovery: bool = False,
    max_response_bytes: int = 2_000_000,
) -> tuple[WorkflowOperation, ...]:
    """Bind an operator-classified read-only allowlist to an already connected MCP client.

    ``reviewed_read_only_tools`` is the operator's classification. Server hints never add tools
    or confer permission. Discovery is separately approved, and registration never calls a tool.
    """
    if approve_tool_discovery is not True:
        raise McpWorkflowError("MCP workflow registration requires explicit discovery approval")
    if not isinstance(catalog, WorkflowOperationCatalog) or not isinstance(client, Client):
        raise McpWorkflowError("MCP registration requires an operation catalogue and connected Client")
    if not isinstance(integration_id, str) or not _IDENTIFIER.fullmatch(integration_id):
        raise McpWorkflowError("MCP workflow integration_id must be a bounded identifier")
    if (isinstance(reviewed_read_only_tools, (str, bytes)) or not isinstance(reviewed_read_only_tools, Sequence)
            or not 1 <= len(reviewed_read_only_tools) <= MAX_TOOL_DEFINITIONS
            or any(not isinstance(name, str) or not _IDENTIFIER.fullmatch(name) for name in reviewed_read_only_tools)
            or len(set(reviewed_read_only_tools)) != len(reviewed_read_only_tools)):
        raise McpWorkflowError("MCP workflow requires an explicit unique reviewed read-only tool allowlist")
    if type(max_response_bytes) is not int or not 1 <= max_response_bytes <= MAX_MCP_WORKFLOW_RESPONSE_BYTES:
        raise McpWorkflowError("MCP workflow response limit is outside its byte bound")
    existing = catalog.operations()
    if len(existing) + len(reviewed_read_only_tools) > MAX_WORKFLOW_OPERATIONS:
        raise McpWorkflowError("MCP registration would exceed operation catalogue capacity")
    integrations = {operation.integration_id for operation in existing}
    if integration_id not in integrations and len(integrations) >= MAX_WORKFLOW_INTEGRATIONS:
        raise McpWorkflowError("MCP registration would exceed integration catalogue capacity")
    existing_ids = {operation.operation_id for operation in existing}
    if any(f"{integration_id}.{name}" in existing_ids for name in reviewed_read_only_tools):
        raise McpWorkflowError("MCP workflow operation is already registered")
    with client._lock:
        metadata, binding = _connected_identity(client)
        identity_digest = metadata["server_identity_digest"]
        tools, _ = _discover(client)
        _same_binding(client, binding, identity_digest)
    if not set(reviewed_read_only_tools) <= set(tools):
        raise McpWorkflowError("MCP reviewed tool allowlist contains an unavailable tool")
    reviewed = {name: tools[name] for name in reviewed_read_only_tools}
    reviewed_digest = content_digest(reviewed)
    prepared = []
    for name in reviewed_read_only_tools:
        tool = reviewed[name]
        description = tool.get("description", tool.get("title", name))
        if not isinstance(description, str) or len(description.encode("utf-8")) > 7_000:
            raise McpWorkflowError("MCP tool description is invalid or exceeds its byte bound")
        description += f"\nReviewed MCP session: {identity_digest}; reviewed tools: {reviewed_digest}"
        try:
            _reject_secret_fields(tool["inputSchema"])
            _reject_secret_fields(tool.get("outputSchema", {}))
            descriptor = WorkflowOperation(f"{integration_id}.{name}", description, tool["inputSchema"],
                                           tool.get("outputSchema", {"type": "object"}), integration_id,
                                           auth_mode="caller_owned_mcp_session", read_only=True, implementation="live_adapter")
        except (ValueError, KeyError):
            raise McpWorkflowError("MCP reviewed tool has an unsupported input or output schema") from None
        prepared.append((name, descriptor))

    def executor_for(name: str, descriptor: WorkflowOperation):
        schemas = descriptor.to_dict()
        checker = ToolCatalogue.from_definitions((ToolDefinition(name, schemas["input_schema"]),))
        output_checker = ToolCatalogue.from_definitions((ToolDefinition(name, schemas["output_schema"]),))
        def execute(arguments, *, approve_source_dispatch=False, retrieved_at=None):
            if approve_source_dispatch is not True:
                raise McpWorkflowError("MCP workflow source dispatch requires explicit approval")
            safe = _json(arguments, maximum=MAX_WORKFLOW_ARGUMENT_BYTES, label="MCP workflow arguments")
            if not isinstance(safe, dict) or not checker.validate(name, safe).fully_checked:
                raise McpWorkflowError("MCP workflow arguments fail the reviewed input schema")
            try:
                _reject_secret_fields(safe)
            except ValueError:
                raise McpWorkflowError("MCP workflow arguments contain credential-shaped fields") from None
            if retrieved_at is not None and not isinstance(retrieved_at, str):
                raise McpWorkflowError("MCP retrieval clock must be text when supplied")
            with client._lock:
                _same_binding(client, binding, identity_digest)
                current, _ = _discover(client)
                _same_binding(client, binding, identity_digest)
                if any(tool_name not in current for tool_name in reviewed):
                    raise McpWorkflowError("MCP reviewed tool disappeared after registration")
                if content_digest({tool_name: current[tool_name] for tool_name in reviewed}) != reviewed_digest:
                    raise McpWorkflowError("MCP reviewed tool definition changed after registration")
                try:
                    result = client.call_tool(name, safe)
                except Exception:
                    raise McpWorkflowError("MCP tool transport failed; remote details withheld") from None
                _same_binding(client, binding, identity_digest)
            value, value_kind, omitted = _value(result, requires_structured="outputSchema" in reviewed[name], maximum=max_response_bytes)
            if not output_checker.validate(name, value).fully_checked:
                raise McpWorkflowError("MCP result fails the reviewed output schema")
            return WorkflowOperationResult(value=value, receipt={
                "adapter": "reviewed_mcp_tool", "operation_id": descriptor.operation_id,
                "integration_id": integration_id, "tool_name": name,
                "server_identity_digest": identity_digest, "reviewed_tools_digest": reviewed_digest,
                "tool_definition_digest": content_digest(reviewed[name]),
                "arguments_digest": content_digest(safe), "response_digest": content_digest(value),
                "result_kind": value_kind, "unstructured_content_blocks_omitted": omitted,
                "output_schema_declared": "outputSchema" in reviewed[name],
                "retrieved_at": retrieved_at,
                "verification": ("declared_mcp_output_schema_checked" if "outputSchema" in reviewed[name]
                                 else "mcp_json_object_shape_checked"),
            })
        return execute

    for name, descriptor in prepared:
        catalog.register(descriptor, executor_for(name, descriptor))
    return tuple(descriptor for _, descriptor in prepared)
