"""Account/session boundaries with fixture credentials, never real account access."""

from http.server import BaseHTTPRequestHandler, HTTPServer
import http.client
import json
import threading
import traceback
from unittest.mock import patch
from urllib.parse import urlsplit

import pytest

from prism_sdk.llm_runtime import CredentialStore
from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan
from prism_sdk.workflow_connections import (
    WorkflowConnectionContext, WorkflowConnectionError, WorkflowConnectionResource,
    WorkflowConnectionSession,
)
from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from prism_sdk.workflow_openapi import OpenApiHttpResponse, OpenApiImportError, import_openapi_operations


SERVER = "https://api.example.org/v1"
CONTEXT = WorkflowConnectionContext("tenant-a", "actor-a", "session-a")
SENTINEL = "fixture-only-never-a-real-credential"


def _document():
    return {"openapi": "3.0.3", "info": {"title": "Public metadata fixture", "version": "1"},
            "servers": [{"url": SERVER}],
            "components": {"securitySchemes": {"auth": {"type": "http", "scheme": "bearer"}}},
            "security": [{"auth": []}],
            "paths": {"/records/{record_id}": {"get": {
                "operationId": "lookup", "parameters": [
                    {"name": "record_id", "in": "path", "required": True,
                     "schema": {"type": "string", "minLength": 1}},
                    {"name": "limit", "in": "query", "schema": {"type": "integer", "minimum": 1}}],
                "responses": {"200": {"description": "Public identifier", "content": {"application/json": {
                    "schema": {"type": "object", "properties": {"record_id": {"type": "string"}},
                               "required": ["record_id"], "additionalProperties": False}}}}}}}}}


class _Transport:
    def __init__(self):
        self.calls = []

    def __call__(self, url, **kwargs):
        self.calls.append((url, kwargs))
        return OpenApiHttpResponse(200, {"Content-Type": "application/json"}, b'{"record_id":"public-record"}')


def _session(*, ready=True, resources=None, **options):
    now = [1000.0]
    store = CredentialStore(clock=lambda: now[0])
    handle = store.register("research", SENTINEL)
    arguments = {"service_id": "research", "context": CONTEXT, "expected_account_id": "account-a",
                 "credential_store": store, "credential": handle, "server_url": SERVER,
                 "resources": resources if resources is not None else [WorkflowConnectionResource("research.lookup", {"record_id": ["public-record"]})],
                 "clock": lambda: now[0], "ttl_seconds": 60}
    arguments.update(options)
    session = WorkflowConnectionSession(**arguments)
    if ready:
        session.observe_account("account-a", approve_identity_observation=True)
        session.authorize(approve_connection_use=True)
    return session, store, handle, now


def _import(session=None, *, transport="fixture", **options):
    catalog = WorkflowOperationCatalog()
    transport = _Transport() if transport == "fixture" else transport
    arguments = {"integration_id": "research", "server_url": SERVER,
                 "allowed_hosts": ["api.example.org"], "reviewed_operation_ids": ["lookup"],
                 "approve_spec_import": True, "transport": transport}
    if session is not None:
        arguments.update(connection=session, connection_context=CONTEXT, approve_public_metadata_response=True)
    arguments.update(options)
    import_openapi_operations(catalog, _document(), **arguments)
    return catalog, transport


def _plan(catalog):
    return compile_workflow_plan({"name": "one public metadata read", "steps": [
        {"id": "lookup", "operation": "research.lookup", "arguments": {"record_id": "public-record"},
         "depends_on": []}], "outputs": ["lookup"]}, catalog, task="Read one public metadata record")


def test_unknown_account_configured_credential_and_authority_are_independent_states():
    session, _, _, _ = _session(ready=False)
    metadata = session.to_metadata()
    assert metadata["credential_state"] == "configured"
    assert metadata["account_state"] == "unknown"
    assert metadata["authorization_state"] == "not_authorized"
    assert metadata["live_verification"] == "unassessed"
    with pytest.raises(WorkflowConnectionError, match="identity"):
        session.authorize(approve_connection_use=True)
    with pytest.raises(WorkflowConnectionError):
        _import(session)
    session.observe_account("account-a", approve_identity_observation=True)
    assert session.to_metadata()["account_state"] == "application_attested"
    with pytest.raises(WorkflowConnectionError):
        _import(session)
    session.authorize(approve_connection_use=True)
    assert session.to_metadata()["authorization_state"] == "authorized"
    assert "not_provider_verification" in session.to_metadata()["account_observation"]


def test_wrong_account_failure_is_latched_and_cannot_be_relabelled_as_correct():
    session, _, _, _ = _session(ready=False)
    with pytest.raises(WorkflowConnectionError, match="expected account"):
        session.observe_account("account-b", approve_identity_observation=True)
    with pytest.raises(WorkflowConnectionError, match="latched"):
        session.observe_account("account-a", approve_identity_observation=True)
    assert session.to_metadata()["failure"] == "account_mismatch"
    assert session.to_metadata()["account_state"] == "failed"


@pytest.mark.parametrize("grant", [False, 1, "true", None])
def test_identity_and_connection_authorization_require_separate_literal_grants(grant):
    session, _, _, _ = _session(ready=False)
    with pytest.raises(WorkflowConnectionError):
        session.observe_account("account-a", approve_identity_observation=grant)
    session.observe_account("account-a", approve_identity_observation=True)
    with pytest.raises(WorkflowConnectionError):
        session.authorize(approve_connection_use=grant)


@pytest.mark.parametrize("context", [WorkflowConnectionContext("tenant-b", "actor-a", "session-a"),
                                      WorkflowConnectionContext("tenant-a", "actor-b", "session-a"),
                                      WorkflowConnectionContext("tenant-a", "actor-a", "session-b"),
                                      {"tenant_id": "tenant-a", "actor_id": "actor-a", "session_id": "session-a"}])
def test_wrong_tenant_actor_or_application_session_fails_before_registration(context):
    session, _, _, _ = _session()
    with pytest.raises(ValueError, match="context|tenant"):
        _import(session, connection_context=context)


def test_credential_resolution_follows_approval_schema_url_and_exact_resource_checks():
    session, store, _, _ = _session()
    catalog, transport = _import(session)
    with patch.object(store, "_resolve", wraps=store._resolve) as resolve:
        assert catalog.catalog_digest and catalog.to_dict()
        assert resolve.call_count == 0
        for arguments, approved in (({"record_id": "public-record"}, False),
                                    ({"record_id": "public-record", "limit": True}, True),
                                    ({"record_id": "another-record"}, True),
                                    ({"record_id": ".."}, True)):
            with pytest.raises(ValueError):
                catalog.execute("research.lookup", arguments, approve_source_dispatch=approved)
        assert not transport.calls and resolve.call_count == 0
        result = catalog.execute("research.lookup", {"record_id": "public-record"}, approve_source_dispatch=True)
        assert resolve.call_count == 1
    assert transport.calls[0][1]["headers"]["Authorization"] == "Bearer " + SENTINEL
    assert result.receipt["account_identity"] == "application_attested;not_provider_verified"
    assert SENTINEL not in json.dumps([catalog.to_dict(), result.receipt, session.to_metadata()])
    assert "account-a" not in json.dumps([catalog.to_dict(), result.receipt, session.to_metadata()])


def test_fixed_server_operation_resource_and_auth_policy_must_match_import():
    session, _, _, _ = _session(resources=[WorkflowConnectionResource("research.other", {})])
    with pytest.raises(WorkflowConnectionError, match="operation"):
        _import(session)
    session, _, _, _ = _session(resources=[WorkflowConnectionResource("research.lookup", {})])
    with pytest.raises(WorkflowConnectionError, match="every path"):
        _import(session)
    session, _, _, _ = _session(resources=[WorkflowConnectionResource("research.lookup", {"record_id": ["public-record"], "unknown": [1]})])
    with pytest.raises(WorkflowConnectionError, match="declared arguments"):
        _import(session)
    session, _, _, _ = _session(server_url="https://api.example.org/other")
    with pytest.raises(WorkflowConnectionError, match="destination"):
        _import(session)
    session, _, _, _ = _session(auth_policy="api_key_header", header_name="X-API-Key")
    with pytest.raises(OpenApiImportError, match="authentication"):
        _import(session)


@pytest.mark.parametrize("change", ["revoke", "close", "expiry", "credential_revoke", "credential_substitute"])
def test_invalidated_session_refuses_dispatch_and_same_catalog_completed_checkpoint_replay(change):
    session, store, handle, now = _session()
    catalog, transport = _import(session)
    plan = _plan(catalog)
    completed = execute_workflow_plan(plan, catalog, approved=True)
    assert completed.status == "completed" and len(transport.calls) == 1
    if change == "revoke":
        session.revoke()
    elif change == "close":
        session.close()
    elif change == "expiry":
        now[0] += 60
    elif change == "credential_revoke":
        store.revoke(handle)
    else:
        handle.credential_id = store.register("research", "replacement-fixture").credential_id
        assert session.to_metadata()["credential_state"] == "unavailable"
    with pytest.raises(WorkflowConnectionError):
        catalog.execute("research.lookup", {"record_id": "public-record"}, approve_source_dispatch=True)
    with pytest.raises(WorkflowConnectionError):
        execute_workflow_plan(plan, catalog, approved=True, checkpoint=completed.checkpoint)
    assert len(transport.calls) == 1


def test_replacement_account_or_permission_session_has_new_catalog_and_rejects_old_plan_and_checkpoint():
    first, store, handle, _ = _session()
    first_catalog, first_transport = _import(first)
    plan = _plan(first_catalog)
    checkpoint = execute_workflow_plan(plan, first_catalog, approved=True).checkpoint
    replacement = WorkflowConnectionSession(service_id="research", context=CONTEXT, expected_account_id="account-b",
        credential_store=store, credential=handle, server_url=SERVER,
        resources=[WorkflowConnectionResource("research.lookup", {"record_id": ["public-record", "other"]})])
    replacement.observe_account("account-b", approve_identity_observation=True)
    replacement.authorize(approve_connection_use=True)
    second_catalog, second_transport = _import(replacement)
    assert first_catalog.catalog_digest != second_catalog.catalog_digest
    with pytest.raises(ValueError, match="identity"):
        execute_workflow_plan(plan, second_catalog, approved=True, checkpoint=checkpoint)
    assert len(first_transport.calls) == 1 and not second_transport.calls


def test_legacy_header_mapping_snapshot_cannot_rotate_credentials_under_existing_catalog_identity():
    headers = {"Authorization": "Bearer first-fixture"}
    first, transport = _import(auth_headers=headers)
    digest = first.catalog_digest
    headers["Authorization"] = "Bearer replacement-fixture"
    result = first.execute("research.lookup", {"record_id": "public-record"}, approve_source_dispatch=True)
    assert transport.calls[0][1]["headers"]["Authorization"] == "Bearer first-fixture"
    assert first.catalog_digest == digest and result.receipt["account_identity"] == "unknown"
    second, _ = _import(auth_headers=headers)
    assert second.catalog_digest != digest
    with pytest.raises(ValueError, match="identity"):
        execute_workflow_plan(_plan(first), second, approved=True)
    assert "first-fixture" not in json.dumps(first.to_dict())


def test_legacy_callable_is_refused_without_invocation_rather_than_allowing_in_place_account_rotation():
    calls = []
    def resolver():
        calls.append(True)
        return {"Authorization": "Bearer fixture"}
    with pytest.raises(OpenApiImportError, match="callable resolvers are refused"):
        _import(auth_headers=resolver)
    assert calls == []


@pytest.mark.parametrize("options", [{"approve_public_metadata_response": False},
                                      {"approve_public_metadata_response": 1},
                                      {"auth_headers": {"Authorization": "x"}},
                                      {"connection_context": None}])
def test_typed_connection_import_requires_explicit_public_metadata_retention_review(options):
    session, _, _, _ = _session()
    with pytest.raises(OpenApiImportError):
        _import(session, **options)


@pytest.mark.parametrize("options", [{"ttl_seconds": True}, {"ttl_seconds": 0}, {"ttl_seconds": float("nan")},
                                      {"ttl_seconds": 86401}, {"clock": lambda: True},
                                      {"header_name": "Host", "auth_policy": "api_key_header"},
                                      {"header_name": "X-API-Key"}, {"auth_policy": "oauth"}])
def test_invalid_lifetime_clock_or_auth_policy_never_creates_session(options):
    with pytest.raises(WorkflowConnectionError):
        _session(**options)


def test_strict_json_resource_types_do_not_treat_false_as_zero_and_public_mutation_cannot_widen_scope():
    resource = WorkflowConnectionResource("research.lookup", {"record_id": ["public-record"], "limit": [False]})
    resource.argument_values["limit"] = [0]
    session, _, _, _ = _session(resources=[resource])
    with pytest.raises(WorkflowConnectionError, match="resource scope"):
        session.authorize_arguments(operation_id="research.lookup", arguments={"record_id": "public-record", "limit": 0},
                                    context=CONTEXT, server_url=SERVER)
    session.authorize_arguments(operation_id="research.lookup", arguments={"record_id": "public-record", "limit": False},
                                context=CONTEXT, server_url=SERVER)


@pytest.mark.parametrize("attribute,replacement", [
    ("_resources", (WorkflowConnectionResource("research.lookup", {"record_id": ["outside-original-scope"]}),)),
    ("_expected_account", "account-b"), ("_context", WorkflowConnectionContext("other", "actor-a", "session-a")),
    ("_server", "https://api.example.org/other"), ("_service_id", "different"),
    ("_auth_policy", "api_key_header"), ("_header_name", "X-API-Key"),
    ("_expires_at", 10_000), ("_clock", lambda: 1000), ("_nonce", "replacement"),
])
def test_accidental_private_configuration_changes_refuse_dispatch_and_completed_replay(attribute, replacement):
    session, _, _, _ = _session()
    catalog, transport = _import(session)
    plan = _plan(catalog)
    checkpoint = execute_workflow_plan(plan, catalog, approved=True).checkpoint
    setattr(session, attribute, replacement)
    with pytest.raises(WorkflowConnectionError, match="configuration changed"):
        catalog.execute("research.lookup", {"record_id": "public-record"}, approve_source_dispatch=True)
    with pytest.raises(WorkflowConnectionError, match="configuration changed"):
        execute_workflow_plan(plan, catalog, approved=True, checkpoint=checkpoint)
    assert len(transport.calls) == 1


def test_credential_errors_with_secret_text_have_no_formatted_exception_chain_or_metadata_leak():
    session, store, _, _ = _session()
    catalog, transport = _import(session)
    def fail(*args, **kwargs):
        raise RuntimeError(SENTINEL)
    with patch.object(store, "_resolve", side_effect=fail):
        with pytest.raises(WorkflowConnectionError) as error:
            catalog.execute("research.lookup", {"record_id": "public-record"}, approve_source_dispatch=True)
    assert SENTINEL not in "".join(traceback.format_exception(error.value))
    assert not transport.calls


def test_default_transport_refuses_private_dns_before_resolving_credentials_or_creating_connection():
    session, store, _, _ = _session()
    catalog, _ = _import(session, transport=None)
    private = (2, 1, 6, "", ("127.0.0.1", 443))
    with patch("prism_sdk.workflow_openapi_http.socket.getaddrinfo", return_value=[private]), \
         patch("prism_sdk.workflow_openapi_http.socket.socket") as connection, \
         patch.object(store, "_resolve", wraps=store._resolve) as resolve:
        with pytest.raises(OpenApiImportError, match="dns_address_is_not_public_or_allowed"):
            catalog.execute("research.lookup", {"record_id": "public-record"}, approve_source_dispatch=True)
        assert resolve.call_count == connection.call_count == 0


def test_default_transport_pins_destination_before_secret_resolution_and_closes_after_failure():
    session, store, _, _ = _session()
    catalog, _ = _import(session, transport=None)
    address = (2, 1, 6, "", ("93.184.216.34", 443))
    order = []
    def resolve(handle):
        order.append("resolve")
        raise RuntimeError(SENTINEL)
    with patch("prism_sdk.workflow_openapi_http.socket.getaddrinfo", return_value=[address]), \
         patch("prism_sdk.workflow_openapi_http.socket.socket") as connection_type, \
         patch("prism_sdk.workflow_openapi_http.ssl.create_default_context") as context, \
         patch.object(store, "_resolve", side_effect=resolve):
        connection_type.side_effect = lambda *args: order.append("pin") or connection_type.return_value
        context.return_value.wrap_socket.side_effect = lambda *args, **kwargs: order.append("tls") or context.return_value.wrap_socket.return_value
        with pytest.raises(OpenApiImportError) as error:
            catalog.execute("research.lookup", {"record_id": "public-record"}, approve_source_dispatch=True)
        assert order == ["pin", "tls", "resolve"]
        assert SENTINEL not in "".join(traceback.format_exception(error.value))
        context.return_value.wrap_socket.return_value.send.assert_not_called()
        context.return_value.wrap_socket.return_value.close.assert_called_once()


def test_slow_credential_callback_exhausts_total_injected_deadline_before_transport_admission():
    session, _, _, _ = _session()
    catalog, transport = _import(session, timeout_seconds=1)
    with patch("prism_sdk.workflow_openapi.time.monotonic", side_effect=[0.0, 1.0]):
        with pytest.raises(OpenApiImportError, match="total_deadline_exceeded"):
            catalog.execute("research.lookup", {"record_id": "public-record"}, approve_source_dispatch=True)
    assert transport.calls == []


@pytest.mark.parametrize("invalidation", ["revoke", "close", "configuration"])
def test_reentrant_clock_invalidation_during_final_header_admission_prevents_transport(invalidation):
    armed = [False]
    calls = [0]
    session = None
    def clock():
        if armed[0]:
            calls[0] += 1
            if calls[0] == 2:
                if invalidation == "configuration":
                    session._expected_account = "replacement-account"
                else:
                    getattr(session, invalidation)()
        return 1000.0
    session, _, _, _ = _session(clock=clock)
    catalog, transport = _import(session)
    armed[0] = True
    with pytest.raises(WorkflowConnectionError):
        catalog.execute("research.lookup", {"record_id": "public-record"}, approve_source_dispatch=True)
    assert calls[0] == 2 and not transport.calls


@pytest.mark.parametrize("invalidation", ["revoke", "close", "configuration", "expiry", "credential_revoke", "account_mismatch"])
def test_reentrant_credential_resolution_invalidation_prevents_transmitting_resolved_headers(invalidation):
    session, store, _, now = _session()
    catalog, transport = _import(session)
    original = store._resolve
    def resolve(handle):
        value = original(handle)
        if invalidation == "configuration":
            session._expected_account = "replacement-account"
        elif invalidation == "expiry":
            now[0] += 60
        elif invalidation == "credential_revoke":
            store.revoke(handle)
        elif invalidation == "account_mismatch":
            with pytest.raises(WorkflowConnectionError, match="expected account"):
                session.observe_account("wrong-account", approve_identity_observation=True)
        else:
            getattr(session, invalidation)()
        return value
    with patch.object(store, "_resolve", side_effect=resolve):
        with pytest.raises(WorkflowConnectionError):
            catalog.execute("research.lookup", {"record_id": "public-record"}, approve_source_dispatch=True)
    assert not transport.calls


def test_credential_store_service_and_expiry_must_match_exact_existing_handle():
    other = CredentialStore()
    wrong_store = other.register("research", SENTINEL)
    with pytest.raises(WorkflowConnectionError, match="credential is unavailable"):
        _session(credential=wrong_store)
    wrong_service = other.register("different-service", SENTINEL)
    with pytest.raises(WorkflowConnectionError, match="credential service"):
        _session(credential_store=other, credential=wrong_service)
    now = [1000.0]
    store = CredentialStore(clock=lambda: now[0])
    expiring = store.register("research", SENTINEL, ttl_seconds=5)
    session, _, _, _ = _session(credential_store=store, credential=expiring)
    catalog, transport = _import(session)
    now[0] += 5
    with pytest.raises(WorkflowConnectionError, match="credential is unavailable"):
        catalog.execute("research.lookup", {"record_id": "public-record"}, approve_source_dispatch=True)
    assert not transport.calls


@pytest.mark.parametrize("constraints", [{"record_id": [1]}, {"record_id": ["public-record"], "limit": [False]},
                                         {"record_id": ["public-record"], "limit": [0]}])
def test_resource_permission_values_must_satisfy_reviewed_operation_schema_at_import(constraints):
    session, _, _, _ = _session(resources=[WorkflowConnectionResource("research.lookup", constraints)])
    with pytest.raises(OpenApiImportError):
        _import(session)


def test_real_local_http_peer_receives_only_scoped_fixture_request_and_public_result_receipt():
    received = []
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            received.append((self.path, self.headers.get("Authorization")))
            body = b'{"record_id":"public-record"}'
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        def log_message(self, *args):
            pass
    server = HTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    def fixture_transport(url, *, headers, timeout_seconds, max_response_bytes):
        assert url.startswith(SERVER + "/records/")
        parts = urlsplit(url)
        connection = http.client.HTTPConnection("127.0.0.1", server.server_port, timeout=timeout_seconds)
        try:
            connection.request("GET", parts.path + (("?" + parts.query) if parts.query else ""), headers=dict(headers))
            response = connection.getresponse()
            return OpenApiHttpResponse(response.status, dict(response.getheaders()), response.read(max_response_bytes + 1), url)
        finally:
            connection.close()
    try:
        session, _, _, _ = _session()
        catalog, _ = _import(session, transport=fixture_transport)
        with pytest.raises(WorkflowConnectionError):
            catalog.execute("research.lookup", {"record_id": "other"}, approve_source_dispatch=True)
        result = catalog.execute("research.lookup", {"record_id": "public-record", "limit": 2}, approve_source_dispatch=True)
        assert received == [("/v1/records/public-record?limit=2", "Bearer " + SENTINEL)]
        assert result.value == {"record_id": "public-record"}
        assert result.receipt["transport_mode"] == "application_injected"
        assert result.receipt["live_verification"] == "unassessed"
        assert SENTINEL not in json.dumps(result.receipt)
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
        assert not thread.is_alive()
