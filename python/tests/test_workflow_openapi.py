"""Claims about bounded imports and HTTP mapping, exercised without external network access."""

import copy
import json
from unittest.mock import patch

import pytest

from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from prism_sdk.workflow_openapi import OpenApiHttpResponse, OpenApiImportError, import_openapi_operations


SERVER = "https://api.example.org/v1"


def _operation(operation_id="lookup"):
    return {
        "operationId": operation_id,
        "summary": "Retrieve research metadata",
        "parameters": [
            {"name": "record_id", "in": "path", "required": True, "schema": {"type": "string", "minLength": 1}},
            {"name": "limit", "in": "query", "schema": {"type": "integer", "minimum": 1, "maximum": 50}},
            {"name": "reviewed", "in": "query", "schema": {"type": "boolean"}},
        ],
        "responses": {"200": {"description": "Research metadata", "content": {"application/json": {
            "schema": {"type": "object", "properties": {
                "records": {"type": "array", "items": {"type": "string"}},
                "count": {"type": "integer", "minimum": 0}},
                "required": ["records", "count"], "additionalProperties": False},
        }}}},
    }


def _document(operation=None):
    return {"openapi": "3.0.3", "info": {"title": "Local test schema", "version": "1"},
            "servers": [{"url": SERVER}], "paths": {"/records/{record_id}": {"get": operation or _operation()}}}


class _FixtureTransport:
    def __init__(self, response=None):
        self.calls = []
        self.response = response or OpenApiHttpResponse(200, {"Content-Type": "application/json"},
                                                     b'{"records":["research"],"count":1}')

    def __call__(self, url, *, headers, timeout_seconds, max_response_bytes):
        self.calls.append({"url": url, "headers": dict(headers), "timeout": timeout_seconds,
                           "maximum": max_response_bytes})
        return self.response


def _import(document=None, catalog=None, **kwargs):
    catalog = catalog if catalog is not None else WorkflowOperationCatalog()
    options = {"integration_id": "research", "server_url": SERVER, "allowed_hosts": ["api.example.org"],
               "reviewed_operation_ids": ["lookup"], "approve_spec_import": True}
    options.update(kwargs)
    return catalog, import_openapi_operations(catalog, document or _document(), **options)


def test_import_is_inert_and_dispatch_maps_only_declared_path_and_query_arguments():
    fixture = _FixtureTransport()
    catalog, descriptors = _import(transport=fixture, timeout_seconds=3, max_response_bytes=256)
    assert len(descriptors) == 1 and fixture.calls == []
    result = catalog.execute("research.lookup", {"record_id": "a/b &?", "limit": 4, "reviewed": False},
                             approve_source_dispatch=True, retrieved_at="2026-10-26T12:00:00Z")
    assert fixture.calls == [{"url": SERVER + "/records/a%2Fb%20%26%3F?limit=4&reviewed=false",
                              "headers": {"Accept": "application/json"}, "timeout": 3, "maximum": 256}]
    assert result.value == {"records": ["research"], "count": 1}
    assert result.receipt["transport_mode"] == "application_injected"
    assert result.receipt["live_verification"] == "unassessed"
    assert result.receipt["verification"] == "schema_checked_response"
    assert result.receipt["retrieved_at"] == "2026-10-26T12:00:00Z"
    assert "a/b" not in json.dumps(result.receipt)


def test_hundreds_of_reviewed_schema_operations_have_distinct_executable_http_mappings():
    document = _document()
    document["paths"] = {f"/dataset_{index}/{{record_id}}": {"get": _operation(f"dataset_{index}")}
                         for index in range(225)}
    fixture = _FixtureTransport()
    catalog, descriptors = _import(document, reviewed_operation_ids=[f"dataset_{index}" for index in range(225)],
                                   transport=fixture)
    assert len(descriptors) == len(catalog.operations()) == 225
    assert len({item.integration_id for item in descriptors}) == 1
    for index in range(225):
        result = catalog.execute(f"research.dataset_{index}", {"record_id": str(index)}, approve_source_dispatch=True)
        assert result.value["count"] == 1
        assert fixture.calls[-1]["url"] == SERVER + f"/dataset_{index}/{index}"
    assert len(fixture.calls) == 225


def test_hundreds_of_independent_operator_registrations_do_not_generate_service_claims():
    catalog = WorkflowOperationCatalog()
    fixture = _FixtureTransport()
    for index in range(210):
        _import(catalog=catalog, integration_id=f"fixture_{index}", transport=fixture)
    assert len(catalog.operations()) == 210
    assert len({item.integration_id for item in catalog.operations()}) == 210
    assert fixture.calls == []
    result = catalog.execute("fixture_209.lookup", {"record_id": "last"}, approve_source_dispatch=True)
    assert result.receipt["transport_mode"] == "application_injected"
    assert result.receipt["live_verification"] == "unassessed"
    assert len(fixture.calls) == 1


@pytest.mark.parametrize("arguments", [{}, {"record_id": "a", "url": "https://evil.example"},
                                      {"record_id": "a", "limit": 51}, {"record_id": "a", "limit": True},
                                      {"record_id": "a", "Authorization": "secret"}, {"record_id": ""}])
def test_missing_invalid_or_undeclared_arguments_never_reach_transport(arguments):
    fixture = _FixtureTransport()
    catalog, _ = _import(transport=fixture)
    with pytest.raises(ValueError):
        catalog.execute("research.lookup", arguments, approve_source_dispatch=True)
    assert fixture.calls == []


def test_import_approval_and_execution_approval_are_independent_literal_grants():
    fixture = _FixtureTransport()
    for grant in (False, 1, "true"):
        with pytest.raises(OpenApiImportError, match="import approval"):
            _import(approve_spec_import=grant, transport=fixture)
    catalog, _ = _import(transport=fixture)
    for grant in (False, 1, "true"):
        with pytest.raises(ValueError):
            catalog.execute("research.lookup", {"record_id": "a"}, approve_source_dispatch=grant)
    assert fixture.calls == []


def test_legacy_header_mapping_is_copied_and_never_serialized():
    document = _document()
    document["components"] = {"securitySchemes": {"researchAuth": {"type": "apiKey", "in": "header", "name": "X-API-Key"}}}
    document["security"] = [{"researchAuth": []}]
    fixture = _FixtureTransport()
    with pytest.raises(OpenApiImportError, match="header mapping or typed connection"):
        _import(document)
    headers = {"X-API-Key": "fixture-credential-do-not-persist"}
    catalog, descriptors = _import(document, auth_headers=headers, transport=fixture)
    headers["X-API-Key"] = "replacement-account-fixture"
    assert not fixture.calls and descriptors[0].auth_mode == "application_header"
    result = catalog.execute("research.lookup", {"record_id": "a"}, approve_source_dispatch=True)
    assert fixture.calls[0]["headers"]["X-API-Key"] == "fixture-credential-do-not-persist"
    assert "fixture-credential" not in json.dumps([descriptors[0].to_dict(), result.receipt])


@pytest.mark.parametrize("edit", [
    lambda doc: doc["paths"]["/records/{record_id}"]["get"]["parameters"][0]["schema"].update({"$ref": "https://evil.example/schema"}),
    lambda doc: doc["paths"]["/records/{record_id}"]["get"]["parameters"][1]["schema"].update({"format": "int32"}),
    lambda doc: doc["paths"]["/records/{record_id}"]["get"]["parameters"][1]["schema"].update({"nullable": True}),
    lambda doc: doc["paths"]["/records/{record_id}"]["get"].update({"servers": [{"url": "https://evil.example"}]}),
    lambda doc: doc["paths"]["/records/{record_id}"]["get"].update({"$ref": "https://evil.example/operation"}),
    lambda doc: doc["paths"]["/records/{record_id}"].update({"$ref": "#/components/pathItems/unsafe"}),
    lambda doc: doc["paths"]["/records/{record_id}"]["get"]["parameters"][1].update({"name": "api_key"}),
    lambda doc: doc["paths"]["/records/{record_id}"]["get"]["parameters"][1].update({"in": "header"}),
    lambda doc: doc["paths"]["/records/{record_id}"]["get"]["parameters"][1].update({"style": "deepObject"}),
    lambda doc: doc["paths"]["/records/{record_id}"]["get"]["parameters"][0].update({"required": False}),
    lambda doc: doc.update({"openapi": "3.1.0"}),
])
def test_unsupported_or_malicious_document_features_fail_before_any_registration(edit):
    document = _document()
    edit(document)
    catalog = WorkflowOperationCatalog()
    with pytest.raises(OpenApiImportError):
        _import(document, catalog=catalog)
    assert catalog.operations() == ()


@pytest.mark.parametrize("server,hosts", [
    ("http://api.example.org/v1", ["api.example.org"]),
    ("https://evil.example/v1", ["api.example.org"]),
    ("https://api.example.org/v1?api_key=secret", ["api.example.org"]),
    ("https://user:secret@api.example.org/v1", ["api.example.org"]),
    ("https://127.0.0.1/v1", ["127.0.0.1"]),
    ("https://10.0.0.1/v1", ["10.0.0.1"]),
    ("https://[::1]/v1", ["::1"]),
    ("https://localhost/v1", ["localhost"]),
    ("https://api.example.org/{tenant}", ["api.example.org"]),
    ("https://api.example.org/%2e%2e", ["api.example.org"]),
])
def test_fixed_https_server_host_policy_rejects_unsafe_destinations(server, hosts):
    document = _document()
    document["servers"] = [{"url": server}]
    with pytest.raises(OpenApiImportError):
        _import(document, server_url=server, allowed_hosts=hosts)


def test_default_transport_checks_private_dns_without_opening_a_socket():
    catalog, _ = _import()
    address = (2, 1, 6, "", ("127.0.0.1", 443))
    with patch("prism_sdk.workflow_openapi.socket.getaddrinfo", return_value=[address]), \
         patch("prism_sdk.workflow_openapi.socket.socket") as socket_constructor:
        with pytest.raises(OpenApiImportError, match="DNS resolved"):
            catalog.execute("research.lookup", {"record_id": "a"}, approve_source_dispatch=True)
        socket_constructor.assert_not_called()


@pytest.mark.parametrize("response", [
    OpenApiHttpResponse(302, {"Location": "https://evil.example"}, b""),
    OpenApiHttpResponse(200, {"Content-Type": "application/json"}, b'{}', "https://evil.example"),
    OpenApiHttpResponse(200, {"Content-Type": "text/html"}, b'{}'),
    OpenApiHttpResponse(200, {"Content-Type": "application/json"}, b'{"records":[],"count":"unknown"}'),
    OpenApiHttpResponse(200, {"Content-Type": "application/json"}, b'{"records":[]}'),
    OpenApiHttpResponse(200, {"Content-Type": "application/json"}, b'[]'),
    OpenApiHttpResponse(200, {"Content-Type": "application/json"}, b'not JSON'),
    OpenApiHttpResponse(200, {"Content-Type": "application/json"}, b'x' * 257),
    OpenApiHttpResponse(500, {"Content-Type": "application/json"}, b'{}'),
])
def test_redirects_undeclared_status_and_invalid_or_unbounded_responses_are_refusals(response):
    fixture = _FixtureTransport(response)
    catalog, _ = _import(transport=fixture, max_response_bytes=256)
    with pytest.raises(OpenApiImportError):
        catalog.execute("research.lookup", {"record_id": "a"}, approve_source_dispatch=True)
    assert len(fixture.calls) == 1


def test_every_selected_operation_is_checked_before_catalogue_mutation():
    document = _document()
    invalid = _operation("invalid")
    invalid["parameters"][1]["schema"]["$ref"] = "#/components/schemas/unsupported"
    document["paths"]["/other/{record_id}"] = {"get": invalid}
    catalog = WorkflowOperationCatalog()
    with pytest.raises(OpenApiImportError):
        _import(document, catalog=catalog, reviewed_operation_ids=["lookup", "invalid"])
    assert catalog.operations() == ()


def test_reviewed_ids_must_exist_and_mutating_methods_are_rejected():
    with pytest.raises(OpenApiImportError, match="missing"):
        _import(reviewed_operation_ids=["unknown"])
    document = _document()
    document["paths"]["/records/{record_id}"]["post"] = document["paths"]["/records/{record_id}"].pop("get")
    with pytest.raises(OpenApiImportError, match="GET"):
        _import(document)


def test_server_and_document_identity_change_the_catalogue_digest():
    first, _ = _import()
    changed = _document()
    changed["info"]["title"] = "A separately reviewed schema"
    second, _ = _import(changed)
    third_document = copy.deepcopy(changed)
    third_document["servers"] = [{"url": "https://api.example.org/other"}]
    third, _ = _import(third_document, server_url="https://api.example.org/other")
    assert len({first.catalog_digest, second.catalog_digest, third.catalog_digest}) == 3


def test_default_http_transport_uses_get_exact_target_checked_address_and_bounded_read():
    catalog, _ = _import()
    address = (2, 1, 6, "", ("93.184.216.34", 443))
    with patch("prism_sdk.workflow_openapi.socket.getaddrinfo", return_value=[address]), \
         patch("prism_sdk.workflow_openapi._PinnedHttpsConnection") as connection_type:
        connection = connection_type.return_value
        response = connection.getresponse.return_value
        response.status = 200
        response.getheaders.return_value = [("Content-Type", "application/json")]
        response.read.return_value = b'{"records":[],"count":0}'
        result = catalog.execute("research.lookup", {"record_id": "item", "limit": 3}, approve_source_dispatch=True)
        connection_type.assert_called_once_with("api.example.org", 443, address, 15.0)
        connection.request.assert_called_once_with("GET", "/v1/records/item?limit=3", headers={"Accept": "application/json"})
        response.read.assert_called_once_with(1_048_577)
        connection.close.assert_called_once()
    assert result.receipt["transport_mode"] == "pinned_https"
    assert result.receipt["live_verification"] == "unassessed"
    assert "live_verified" not in result.receipt


def test_path_scope_parameters_can_be_overridden_but_never_make_missing_fields_optional():
    document = _document()
    item = document["paths"]["/records/{record_id}"]
    item["parameters"] = [item["get"]["parameters"].pop(0)]
    item["get"]["parameters"].append({"name": "limit", "in": "query", "required": True,
                                        "schema": {"type": "integer", "enum": [2, 4]}})
    # Same-scope duplicate parameters are invalid rather than interpreted as overrides.
    with pytest.raises(OpenApiImportError, match="duplicated"):
        _import(document)
    item["parameters"].append(item["get"]["parameters"].pop(0))
    catalog, _ = _import(document, transport=_FixtureTransport())
    with pytest.raises(ValueError):
        catalog.execute("research.lookup", {"record_id": "a"}, approve_source_dispatch=True)
    assert catalog.execute("research.lookup", {"record_id": "a", "limit": 2}, approve_source_dispatch=True).value["count"] == 1


def test_duplicate_keys_and_nonfinite_json_never_become_successful_responses():
    for body in (b'{"records":[],"count":0,"count":1}', b'{"records":[],"count":NaN}'):
        fixture = _FixtureTransport(OpenApiHttpResponse(200, {"Content-Type": "application/json"}, body))
        catalog, _ = _import(transport=fixture)
        with pytest.raises(OpenApiImportError):
            catalog.execute("research.lookup", {"record_id": "a"}, approve_source_dispatch=True)
    text = json.dumps(_document()).replace('"openapi": "3.0.3"', '"openapi": "3.0.3", "openapi": "3.1.0"')
    with pytest.raises(OpenApiImportError):
        _import(text)


def test_application_exception_details_and_unsafe_headers_cannot_escape_into_receipts():
    sentinel = "fixture-secret-value"
    def fail(*args, **kwargs):
        raise OpenApiImportError(sentinel)
    catalog, _ = _import(transport=fail)
    with pytest.raises(OpenApiImportError) as error:
        catalog.execute("research.lookup", {"record_id": "a"}, approve_source_dispatch=True)
    assert sentinel not in str(error.value)
    fixture = _FixtureTransport()
    for headers in ({"Host": "evil.example"}, {"Authorization": "Bearer token\r\nHost: evil.example"}):
        with pytest.raises(OpenApiImportError, match="header mapping"):
            _import(auth_headers=headers, transport=fixture)
    assert fixture.calls == []
