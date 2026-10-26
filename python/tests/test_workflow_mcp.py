"""The bridge uses real local stdio protocol peers; no external services are exercised."""

from contextlib import ExitStack
from dataclasses import replace
import json
from pathlib import Path
import sys

import pytest

from prism_sdk.client import Client
from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan
from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from prism_sdk.workflow_mcp import McpWorkflowError, mcp_workflow_server_identity, register_mcp_workflow_operations


_ERROR_TEXT = "REMOTE_ERROR_CONTENT_MUST_NOT_BE_EXPOSED"
_PRIVATE_VALUE = "PRIVATE_CLIENT_SETTING_MUST_NOT_BE_SERIALIZED"
_PEER = r'''
import json
from pathlib import Path
import sys

config_path, log_path = map(Path, sys.argv[1:3])
def send(request_id, result=None, error=None):
    message = {"jsonrpc": "2.0", "id": request_id}
    message["error" if error is not None else "result"] = error if error is not None else result
    print(json.dumps(message, separators=(",", ":")), flush=True)

for line in sys.stdin:
    request = json.loads(line)
    if "id" not in request:
        continue
    config = json.loads(config_path.read_text(encoding="utf-8"))
    with log_path.open("a", encoding="utf-8") as log:
        log.write(json.dumps({"method": request["method"], "params": request.get("params", {})}) + "\n")
    request_id, method = request["id"], request["method"]
    if method == "initialize":
        send(request_id, {"protocolVersion": "2025-06-18", "serverInfo": {"name": config["name"], "version": "fixture"},
                          "capabilities": {"tools": {"listChanged": True}}})
    elif method == "tools/list":
        if config.get("list_error"):
            send(request_id, error={"code": -32001, "message": "REMOTE_ERROR_CONTENT_MUST_NOT_BE_EXPOSED"})
            continue
        start = int(request.get("params", {}).get("cursor", "0"))
        size = config.get("page_size", 64)
        result = {"tools": config["tools"][start:start + size]}
        if start + size < len(config["tools"]):
            result["nextCursor"] = str(start + size)
        if config.get("repeat_cursor"):
            result["nextCursor"] = "0"
        send(request_id, result)
    elif method == "tools/call":
        mode = config.get("mode", "structured")
        if mode == "remote_error":
            send(request_id, error={"code": -32002, "message": "REMOTE_ERROR_CONTENT_MUST_NOT_BE_EXPOSED", "data": {"token": "secret"}})
            continue
        if mode == "exit":
            sys.stderr.write("REMOTE_ERROR_CONTENT_MUST_NOT_BE_EXPOSED")
            sys.stderr.flush()
            sys.exit(3)
        arguments = request["params"].get("arguments", {})
        value = config.get("value", {"echo": arguments, "server": config["name"]})
        if mode == "is_error":
            result = {"isError": True, "content": [{"type": "text", "text": "REMOTE_ERROR_CONTENT_MUST_NOT_BE_EXPOSED"}]}
        elif mode == "bad_error_flag":
            result = {"isError": "false", "structuredContent": value}
        elif mode == "array":
            result = {"isError": False, "structuredContent": [value]}
        elif mode == "text_array":
            result = {"isError": False, "content": [{"type": "text", "text": "[1,2]"}]}
        elif mode == "plain_text":
            result = {"isError": False, "content": [{"type": "text", "text": "a human-readable sentence"}]}
        elif mode == "duplicate_json":
            result = {"isError": False, "content": [{"type": "text", "text": '{"echo":{},"echo":{},"server":"fixture"}'}]}
        elif mode == "text":
            result = {"isError": False, "content": [{"type": "text", "text": json.dumps(value)}]}
        elif mode == "multiple_text":
            result = {"isError": False, "content": [{"type": "text", "text": "{}"}, {"type": "text", "text": "{}"}]}
        elif mode == "image":
            result = {"isError": False, "content": [{"type": "image", "mimeType": "image/png", "data": "fixture"}]}
        else:
            result = {"isError": False, "structuredContent": value, "content": [{"type": "text", "text": json.dumps(value)}]}
        send(request_id, result)
    else:
        send(request_id, error={"code": -32601, "message": "unsupported fixture method"})
'''


def _tool(name="echo", *, output_schema=True):
    result = {"name": name, "description": "Read metadata from the local protocol fixture",
              "inputSchema": {"type": "object", "properties": {"value": {"type": "integer", "minimum": 1}},
                              "required": ["value"], "additionalProperties": False},
              "annotations": {"readOnlyHint": True, "destructiveHint": False}}
    if output_schema:
        result["outputSchema"] = {"type": "object", "properties": {"echo": {"type": "object"}, "server": {"type": "string"}},
                                  "required": ["echo", "server"], "additionalProperties": False}
    return result


class _Peer:
    def __init__(self, client, config, log):
        self.client, self.config, self.log = client, config, log

    def update(self, **changes):
        value = json.loads(self.config.read_text(encoding="utf-8"))
        value.update(changes)
        self.config.write_text(json.dumps(value), encoding="utf-8")

    def requests(self, method=None):
        rows = [json.loads(line) for line in self.log.read_text(encoding="utf-8").splitlines()] if self.log.exists() else []
        return rows if method is None else [row for row in rows if row["method"] == method]


@pytest.fixture
def peer_factory(tmp_path):
    with ExitStack() as stack:
        index = 0
        def spawn(*, tools=None, mode="structured", name="fixture", env=None, **settings):
            nonlocal index
            directory = tmp_path / str(index)
            directory.mkdir()
            index += 1
            script, config, log = directory / "peer.py", directory / "config.json", directory / "requests.jsonl"
            script.write_text(_PEER, encoding="utf-8")
            config.write_text(json.dumps({"tools": tools if tools is not None else [_tool()], "mode": mode, "name": name, **settings}), encoding="utf-8")
            client = stack.enter_context(Client([sys.executable, "-u", str(script), str(config), str(log)], env=env, timeout=2))
            return _Peer(client, config, log)
        yield spawn


def _register(peer, *, catalog=None, names=("echo",), integration_id="fixture", **options):
    catalog = WorkflowOperationCatalog() if catalog is None else catalog
    descriptors = register_mcp_workflow_operations(catalog, peer.client, integration_id=integration_id,
                                                  reviewed_read_only_tools=names, approve_tool_discovery=True, **options)
    return catalog, descriptors


def test_existing_fake_mcp_server_json_text_projection_executes_over_real_stdio():
    fake = Path(__file__).parent / "fake_mcp_server.py"
    with Client([sys.executable, "-u", str(fake)], timeout=2) as client:
        catalog = WorkflowOperationCatalog()
        registered = register_mcp_workflow_operations(catalog, client, integration_id="existing_peer",
                                                      reviewed_read_only_tools=["echo"], approve_tool_discovery=True)
        result = catalog.execute("existing_peer.echo", {"value": 7}, approve_source_dispatch=True)
        assert len(registered) == 1 and result.value == {"ok": True, "echo": {"value": 7}}
        assert result.receipt["result_kind"] == "json_text_object"
        assert result.receipt["verification"] == "mcp_json_object_shape_checked"


def test_registration_only_discovers_and_structured_result_receipt_contains_no_values(peer_factory):
    peer = peer_factory()
    before = peer.requests()
    identity = mcp_workflow_server_identity(peer.client)
    assert peer.requests() == before
    catalog, descriptors = _register(peer)
    assert len(peer.requests("tools/list")) == 1 and peer.requests("tools/call") == []
    result = catalog.execute("fixture.echo", {"value": 7}, approve_source_dispatch=True, retrieved_at="2026-10-26T12:00:00Z")
    assert result.value == {"echo": {"value": 7}, "server": "fixture"}
    assert peer.requests("tools/call") == [{"method": "tools/call", "params": {"name": "echo", "arguments": {"value": 7}}}]
    assert len(peer.requests("tools/list")) == 2
    assert result.receipt["server_identity_digest"] == identity["server_identity_digest"]
    assert result.receipt["result_kind"] == "structured_content"
    assert result.receipt["unstructured_content_blocks_omitted"] == 1
    assert result.receipt["output_schema_declared"] is True
    assert result.receipt["verification"] == "declared_mcp_output_schema_checked"
    assert "echo" not in result.receipt and "value" not in result.receipt
    assert str(peer.config) not in json.dumps([identity, descriptors[0].to_dict(), result.receipt])


@pytest.mark.parametrize("grant", [False, 1, "true"])
def test_discovery_requires_a_literal_approval_before_any_tools_list_request(peer_factory, grant):
    peer = peer_factory()
    catalog = WorkflowOperationCatalog()
    with pytest.raises(McpWorkflowError, match="discovery approval"):
        register_mcp_workflow_operations(catalog, peer.client, integration_id="fixture", reviewed_read_only_tools=["echo"], approve_tool_discovery=grant)
    assert catalog.operations() == () and peer.requests("tools/list") == []


@pytest.mark.parametrize("grant", [False, 1, "true"])
def test_source_approval_is_independent_and_denial_does_not_even_rediscover_tools(peer_factory, grant):
    peer = peer_factory()
    catalog, _ = _register(peer)
    before = peer.requests()
    with pytest.raises(ValueError):
        catalog.execute("fixture.echo", {"value": 1}, approve_source_dispatch=grant)
    assert peer.requests() == before


@pytest.mark.parametrize("names", [[], ["echo", "echo"], ["missing"], "echo"])
def test_server_read_only_annotations_never_supply_an_operator_allowlist(peer_factory, names):
    peer = peer_factory()
    catalog = WorkflowOperationCatalog()
    with pytest.raises(McpWorkflowError):
        _register(peer, catalog=catalog, names=names)
    assert catalog.operations() == () and peer.requests("tools/call") == []


def test_only_operator_classified_tools_are_registered_even_when_every_tool_claims_read_only(peer_factory):
    peer = peer_factory(tools=[_tool("echo"), _tool("other")])
    catalog, descriptors = _register(peer)
    assert [tool.operation_id for tool in descriptors] == ["fixture.echo"]
    with pytest.raises(ValueError):
        catalog.resolve("fixture.other")
    assert peer.requests("tools/call") == []


def test_hundreds_of_tools_are_discovered_through_real_paginated_protocol_without_claiming_hundreds_of_services(peer_factory):
    tools = [_tool(f"metadata_{index}") for index in range(225)]
    peer = peer_factory(tools=tools, page_size=40)
    catalog, descriptors = _register(peer, names=[tool["name"] for tool in tools])
    assert len(descriptors) == len(catalog.operations()) == 225
    assert len({tool.integration_id for tool in descriptors}) == 1
    assert len(peer.requests("tools/list")) == 6 and peer.requests("tools/call") == []
    for index in (0, 112, 224):
        result = catalog.execute(f"fixture.metadata_{index}", {"value": index + 1}, approve_source_dispatch=True)
        assert result.value["echo"] == {"value": index + 1}
    assert [call["params"]["name"] for call in peer.requests("tools/call")] == ["metadata_0", "metadata_112", "metadata_224"]


def test_independent_servers_with_the_same_tool_name_keep_distinct_live_session_bindings(peer_factory):
    first, second = peer_factory(name="first"), peer_factory(name="second")
    catalog = WorkflowOperationCatalog()
    _register(first, catalog=catalog, integration_id="one")
    _register(second, catalog=catalog, integration_id="two")
    one = catalog.execute("one.echo", {"value": 1}, approve_source_dispatch=True)
    two = catalog.execute("two.echo", {"value": 2}, approve_source_dispatch=True)
    assert one.value["server"] == "first" and two.value["server"] == "second"
    assert one.receipt["server_identity_digest"] != two.receipt["server_identity_digest"]
    assert len(catalog.operations()) == 2 and len(first.requests("tools/call")) == len(second.requests("tools/call")) == 1


@pytest.mark.parametrize("drift", ["input", "output", "description", "annotations", "missing"])
def test_selected_tool_definition_drift_refuses_before_tools_call(peer_factory, drift):
    peer = peer_factory()
    catalog, _ = _register(peer)
    changed = _tool()
    if drift == "input":
        changed["inputSchema"]["properties"]["value"]["minimum"] = 2
    elif drift == "output":
        changed["outputSchema"]["required"] = ["echo"]
    elif drift == "description":
        changed["description"] = "A newly advertised operation"
    elif drift == "annotations":
        changed["annotations"]["readOnlyHint"] = False
    peer.update(tools=[] if drift == "missing" else [changed])
    with pytest.raises(McpWorkflowError, match="changed|disappeared"):
        catalog.execute("fixture.echo", {"value": 3}, approve_source_dispatch=True)
    assert peer.requests("tools/call") == []


@pytest.mark.parametrize("drift", ["command", "cwd", "timeout", "env", "server_info", "capabilities"])
def test_connected_client_configuration_or_handshake_drift_refuses_before_discovery(peer_factory, drift):
    peer = peer_factory(env={"MCP_PRIVATE_SETTING": _PRIVATE_VALUE})
    catalog, descriptors = _register(peer)
    original = mcp_workflow_server_identity(peer.client)
    before = peer.requests()
    if drift == "command":
        peer.client.config = replace(peer.client.config, command=(*peer.client.config.command, _PRIVATE_VALUE))
    elif drift == "cwd":
        peer.client.config = replace(peer.client.config, cwd=_PRIVATE_VALUE)
    elif drift == "timeout":
        peer.client.config = replace(peer.client.config, timeout=3)
    elif drift == "env":
        peer.client._env["MCP_PRIVATE_SETTING"] = "a changed private setting"
    elif drift == "server_info":
        peer.client.session.server_info["version"] = "changed"
    else:
        peer.client.session.capabilities["tools"]["listChanged"] = False
    with pytest.raises(McpWorkflowError, match="configuration changed") as error:
        catalog.execute("fixture.echo", {"value": 1}, approve_source_dispatch=True)
    assert peer.requests() == before
    assert _PRIVATE_VALUE not in json.dumps([original, descriptors[0].to_dict()]) + str(error.value)


def test_disconnected_and_reconnected_sessions_cannot_reuse_a_reviewed_registration(peer_factory):
    peer = peer_factory()
    catalog, _ = _register(peer)
    original = mcp_workflow_server_identity(peer.client)
    peer.client.close()
    with pytest.raises(McpWorkflowError, match="connected"):
        catalog.execute("fixture.echo", {"value": 1}, approve_source_dispatch=True)
    peer.client.connect()
    before = peer.requests()
    assert mcp_workflow_server_identity(peer.client)["server_identity_digest"] != original["server_identity_digest"]
    with pytest.raises(McpWorkflowError, match="configuration changed"):
        catalog.execute("fixture.echo", {"value": 1}, approve_source_dispatch=True)
    assert peer.requests() == before


@pytest.mark.parametrize("mode", ["is_error", "remote_error", "bad_error_flag", "exit"])
def test_execution_and_protocol_errors_remain_refusals_without_remote_error_payloads(peer_factory, mode):
    peer = peer_factory(mode=mode)
    catalog, _ = _register(peer)
    with pytest.raises(McpWorkflowError) as error:
        catalog.execute("fixture.echo", {"value": 1}, approve_source_dispatch=True)
    assert _ERROR_TEXT not in str(error.value)
    assert len(peer.requests("tools/call")) == 1


@pytest.mark.parametrize("mode", ["array", "text_array", "plain_text", "multiple_text", "image", "duplicate_json"])
def test_nonobject_or_ambiguous_unstructured_results_are_not_invented_into_workflow_objects(peer_factory, mode):
    peer = peer_factory(tools=[_tool(output_schema=False)], mode=mode)
    catalog, _ = _register(peer)
    with pytest.raises(McpWorkflowError):
        catalog.execute("fixture.echo", {"value": 1}, approve_source_dispatch=True)


def test_declared_output_schema_requires_structured_content_and_is_checked(peer_factory):
    peer = peer_factory(mode="text")
    catalog, _ = _register(peer)
    with pytest.raises(McpWorkflowError, match="structuredContent"):
        catalog.execute("fixture.echo", {"value": 1}, approve_source_dispatch=True)
    peer.update(mode="structured", value={"echo": {}, "server": 3})
    with pytest.raises(McpWorkflowError, match="output schema"):
        catalog.execute("fixture.echo", {"value": 1}, approve_source_dispatch=True)


@pytest.mark.parametrize("field", ["inputSchema", "outputSchema"])
@pytest.mark.parametrize("schema", [
    {"type": "object", "properties": {"optional": {"$ref": "https://unsafe.example/schema"}}},
    {"type": "object", "properties": {"optional": {"type": "string", "format": "uri"}}},
    {"type": "object", "properties": {"api_key": {"type": "string"}}},
])
def test_unsupported_or_credential_schema_rejects_the_whole_registration(peer_factory, schema, field):
    invalid = _tool("unsupported")
    invalid[field] = schema
    peer = peer_factory(tools=[_tool(), invalid])
    catalog = WorkflowOperationCatalog()
    with pytest.raises(McpWorkflowError):
        _register(peer, catalog=catalog, names=["echo", "unsupported"])
    assert catalog.operations() == () and peer.requests("tools/call") == []


def test_response_bytes_and_credential_fields_are_refused_before_workflow_retention(peer_factory):
    peer = peer_factory(tools=[_tool(output_schema=False)], value={"public": "x" * 1024})
    catalog, _ = _register(peer, max_response_bytes=256)
    with pytest.raises(McpWorkflowError, match="bounded"):
        catalog.execute("fixture.echo", {"value": 1}, approve_source_dispatch=True)
    peer.update(value={"access_token": "remote-secret"})
    with pytest.raises(McpWorkflowError, match="credential-shaped"):
        catalog.execute("fixture.echo", {"value": 1}, approve_source_dispatch=True)


def test_pagination_cycles_discovery_errors_and_oversized_tool_counts_never_register_partial_support(peer_factory):
    for settings in ({"repeat_cursor": True}, {"list_error": True}, {"tools": [_tool(f"entry_{index}") for index in range(513)]}):
        peer = peer_factory(**settings)
        catalog = WorkflowOperationCatalog()
        with pytest.raises(McpWorkflowError) as error:
            _register(peer, catalog=catalog)
        assert catalog.operations() == () and _ERROR_TEXT not in str(error.value)
        assert peer.requests("tools/call") == []


def test_registered_mcp_tools_compose_into_approved_workflow_dependency_execution(peer_factory):
    peer = peer_factory()
    catalog, _ = _register(peer)
    proposal = {"name": "Reviewed MCP dependency workflow", "steps": [
        {"id": "first", "operation": "fixture.echo", "arguments": {"value": 4}, "depends_on": []},
        {"id": "second", "operation": "fixture.echo", "arguments": {"value": {"$ref": "first#/echo/value"}}, "depends_on": ["first"]},
    ], "outputs": ["second"]}
    plan = compile_workflow_plan(proposal, catalog, task="Review public fixture metadata")
    hold = execute_workflow_plan(plan, catalog)
    assert hold.status == "approval_required" and peer.requests("tools/call") == []
    result = execute_workflow_plan(plan, catalog, approved=True)
    assert result.status == "completed" and result.outputs["second"]["echo"] == {"value": 4}
    assert len(peer.requests("tools/call")) == 2
    resumed = execute_workflow_plan(plan, catalog, approved=True, checkpoint=result.checkpoint)
    assert resumed.status == "completed" and len(peer.requests("tools/call")) == 2


def test_unconnected_clients_are_never_started_or_discovered_implicitly():
    client = Client([sys.executable, "-c", "raise AssertionError('must not launch')"], timeout=2)
    with pytest.raises(McpWorkflowError, match="connected"):
        mcp_workflow_server_identity(client)
    with pytest.raises(McpWorkflowError, match="connected"):
        register_mcp_workflow_operations(WorkflowOperationCatalog(), client, integration_id="fixture", reviewed_read_only_tools=["echo"], approve_tool_discovery=True)
    assert client._process is None and not client.running
