"""Explicitly reviewed OpenAPI GET adapters for the workflow operation catalogue.

This is a repository-defined extension, not a claim to implement an unavailable blueprint
module. The deliberately small supported subset is documented in WORKFLOW_OPENAPI.md. Import
never fetches a specification, discovers credentials, or authorizes a source request.
"""

from __future__ import annotations

from dataclasses import dataclass
import http.client
import ipaddress
import json
import math
import re
import socket
import ssl
from typing import Any, Callable, Mapping, Sequence
from urllib.parse import quote, urlencode, urlsplit

from .authoring import content_digest
from .tooling import ToolCatalogue, ToolDefinition
from .workflow_integrations import (
    MAX_WORKFLOW_INTEGRATIONS, MAX_WORKFLOW_OPERATIONS,
    WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult,
)


class OpenApiImportError(ValueError):
    """A document or response falls outside the explicitly checked import contract."""


@dataclass(frozen=True)
class OpenApiHttpResponse:
    """Bounded transport result; headers are used transiently and never enter a receipt."""

    status: int
    headers: Mapping[str, str]
    body: bytes
    final_url: str | None = None


_IDENTIFIER = re.compile(r"^[A-Za-z][A-Za-z0-9_.:-]{0,127}$")
_PARAMETER = re.compile(r"^[A-Za-z_][A-Za-z0-9_.-]{0,127}$")
_SECRET_NAMES = {"apikey", "key", "authorization", "password", "secret", "token", "accesstoken",
                 "refreshtoken", "clientsecret", "credential", "credentials", "privatekey", "bearer"}
_SCALAR_KEYS = {"type", "description", "title", "enum", "minimum", "maximum", "minLength", "maxLength"}
_CONTAINER_KEYS = {"properties", "required", "additionalProperties", "items", "minItems", "maxItems"}
_SCHEMA_TYPES = {"string", "integer", "number", "boolean", "object", "array"}
_MAX_DOCUMENT_BYTES = 4_000_000


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value = {}
    for key, child in pairs:
        if key in value:
            raise OpenApiImportError("duplicate JSON object keys are unsupported")
        value[key] = child
    return value


def _json(value: Any, label: str, *, maximum: int = _MAX_DOCUMENT_BYTES) -> Any:
    def check_keys(child: Any, depth: int = 0) -> None:
        if depth > 64:
            raise OpenApiImportError(f"{label} exceeds its nesting bound")
        if isinstance(child, Mapping):
            for key, nested in child.items():
                if not isinstance(key, str):
                    raise OpenApiImportError(f"{label} has a non-string JSON object key")
                check_keys(nested, depth + 1)
        elif isinstance(child, list):
            for nested in child:
                check_keys(nested, depth + 1)
    check_keys(value)
    try:
        encoded = json.dumps(value, ensure_ascii=False, allow_nan=False, separators=(",", ":"))
        if len(encoded.encode("utf-8")) > maximum:
            raise OpenApiImportError(f"{label} exceeds its byte bound")
        return json.loads(encoded)
    except (TypeError, ValueError, RecursionError):
        raise OpenApiImportError(f"{label} must be bounded finite JSON") from None


def _mapping(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise OpenApiImportError(f"{label} must be an object")
    return dict(value)


def _secret_name(name: str) -> bool:
    normalized = re.sub(r"[^a-z0-9]", "", name.lower())
    return normalized in _SECRET_NAMES or normalized.endswith("apikey") or normalized.endswith("password")


def _schema(value: Any, *, scalar: bool = False, depth: int = 0) -> dict[str, Any]:
    schema = _mapping(value, "schema")
    if depth > 16:
        raise OpenApiImportError("schema nesting exceeds 16")
    kind = schema.get("type")
    if not isinstance(kind, str) or kind not in _SCHEMA_TYPES or (scalar and kind in {"object", "array"}):
        raise OpenApiImportError("schema requires a supported explicit type")
    permitted = _SCALAR_KEYS | (_CONTAINER_KEYS if not scalar else set())
    if set(schema) - permitted:
        raise OpenApiImportError("schema contains an unsupported keyword or reference")
    for annotation in ("description", "title"):
        if annotation in schema and not isinstance(schema[annotation], str):
            raise OpenApiImportError("schema annotations must be strings")
    for bound in ("minimum", "maximum"):
        if bound in schema and (kind not in {"number", "integer"} or isinstance(schema[bound], bool)
                                or not isinstance(schema[bound], (int, float))):
            raise OpenApiImportError("numeric bounds require a numeric schema")
    for bound in ("minLength", "maxLength", "minItems", "maxItems"):
        if bound in schema and (type(schema[bound]) is not int or schema[bound] < 0
                                or kind != ("string" if "Length" in bound else "array")):
            raise OpenApiImportError("length bounds require their matching schema type")
    if "enum" in schema:
        if kind in {"object", "array"} or not isinstance(schema["enum"], list) or not schema["enum"]:
            raise OpenApiImportError("enum must be a nonempty scalar list")
        for item in schema["enum"]:
            _validate({"type": kind}, item, "enum member")
    if kind == "object":
        properties = _mapping(schema.get("properties", {}), "schema properties")
        if len(properties) > 256:
            raise OpenApiImportError("object schema has too many properties")
        for name, item in properties.items():
            if not isinstance(name, str) or _secret_name(name):
                raise OpenApiImportError("credential-shaped schema fields are not workflow inputs or outputs")
            properties[name] = _schema(item, depth=depth + 1)
        required = schema.get("required", [])
        if not isinstance(required, list) or any(not isinstance(item, str) for item in required):
            raise OpenApiImportError("required must be a list of property names")
        if len(required) != len(set(required)) or not set(required) <= set(properties):
            raise OpenApiImportError("required names must be unique declared properties")
        if "additionalProperties" in schema and type(schema["additionalProperties"]) is not bool:
            raise OpenApiImportError("additionalProperties schemas are unsupported")
        schema["properties"] = properties
    elif kind == "array":
        if "items" not in schema:
            raise OpenApiImportError("array requires an item schema")
        schema["items"] = _schema(schema["items"], depth=depth + 1)
    elif set(schema) & _CONTAINER_KEYS:
        raise OpenApiImportError("container keywords require matching container types")
    if kind != "object" and set(schema) & {"properties", "required", "additionalProperties"}:
        raise OpenApiImportError("object keywords require object type")
    if kind != "array" and "items" in schema:
        raise OpenApiImportError("items requires array type")
    return schema


def _validate(schema: Mapping[str, Any], value: Any, label: str) -> None:
    wrapper = {"type": "object", "properties": {"value": dict(schema)}, "required": ["value"]}
    checker = ToolCatalogue.from_definitions([ToolDefinition("openapi_value", wrapper)])
    report = checker.validate("openapi_value", {"value": value})
    if not report.fully_checked:
        raise OpenApiImportError(f"{label} failed strict schema validation")


def _host_allowed(host: str, allowed_hosts: Sequence[str], allow_private_hosts: bool) -> None:
    if host not in allowed_hosts:
        raise OpenApiImportError("selected server host must appear in the exact host allowlist")
    if host in {"localhost", "localhost.localdomain"} or host.endswith(".localhost"):
        if not allow_private_hosts:
            raise OpenApiImportError("private or loopback server is disabled")
    try:
        address = ipaddress.ip_address(host)
    except ValueError:
        return
    if not allow_private_hosts and not address.is_global:
        raise OpenApiImportError("private or loopback server is disabled")


def _server(value: Any, allowed_hosts: Sequence[str], allow_private_hosts: bool) -> str:
    if not isinstance(value, str) or len(value) > 2048 or any(ord(ch) < 33 for ch in value):
        raise OpenApiImportError("server URL must be bounded HTTPS text")
    try:
        parts = urlsplit(value)
        port = parts.port
    except ValueError:
        raise OpenApiImportError("server URL is invalid") from None
    if (parts.scheme != "https" or not parts.hostname or parts.username or parts.password
            or parts.query or parts.fragment or "{" in value or "}" in value or "\\" in value):
        raise OpenApiImportError("server must be a fixed HTTPS URL without credentials, query, or variables")
    if port not in {None, 443} and not allow_private_hosts:
        raise OpenApiImportError("nonstandard server ports require explicit private-host policy")
    if any(segment in {".", ".."} for segment in parts.path.split("/")) or "%" in parts.path:
        raise OpenApiImportError("server path cannot contain encoded or relative segments")
    _host_allowed(parts.hostname.lower(), allowed_hosts, allow_private_hosts)
    return value.rstrip("/")


class _PinnedHttpsConnection(http.client.HTTPSConnection):
    """Pin a checked DNS result while retaining TLS certificate and hostname verification."""

    def __init__(self, host: str, port: int, address: tuple[Any, ...], timeout: float):
        super().__init__(host, port=port, timeout=timeout, context=ssl.create_default_context())
        self._checked_address = address

    def connect(self) -> None:
        family, socktype, protocol, _, sockaddr = self._checked_address
        raw = socket.socket(family, socktype, protocol)
        try:
            raw.settimeout(self.timeout)
            raw.connect(sockaddr)
            self.sock = self._context.wrap_socket(raw, server_hostname=self.host)
        except BaseException:
            raw.close()
            raise


def _http_get(url: str, *, headers: Mapping[str, str], timeout_seconds: float,
              max_response_bytes: int, allow_private_hosts: bool) -> OpenApiHttpResponse:
    parts = urlsplit(url)
    host, port = parts.hostname, parts.port or 443
    try:
        addresses = socket.getaddrinfo(host, port, type=socket.SOCK_STREAM)
        if not addresses or len(addresses) > 64:
            raise OpenApiImportError("server DNS resolution is empty or exceeds its bound")
        if not allow_private_hosts and any(not ipaddress.ip_address(item[4][0]).is_global for item in addresses):
            raise OpenApiImportError("server DNS resolved to a private or loopback address")
        connection = _PinnedHttpsConnection(host, port, addresses[0], timeout_seconds)
        try:
            target = parts.path or "/"
            if parts.query:
                target += "?" + parts.query
            connection.request("GET", target, headers=dict(headers))
            response = connection.getresponse()
            body = response.read(max_response_bytes + 1)
            return OpenApiHttpResponse(response.status, dict(response.getheaders()), body, url)
        finally:
            connection.close()
    except OpenApiImportError:
        raise
    except (OSError, http.client.HTTPException, ValueError):
        raise OpenApiImportError("OpenAPI transport failed; credentials and endpoint details withheld") from None


def _parameters(path: str, item: Mapping[str, Any], operation: Mapping[str, Any]) -> tuple[dict, tuple[dict, ...]]:
    merged: dict[tuple[str, str], dict] = {}
    for owner in (item, operation):
        entries = owner.get("parameters", [])
        if not isinstance(entries, list) or len(entries) > 64:
            raise OpenApiImportError("parameters must be a bounded list")
        seen = set()
        for raw in entries:
            parameter = _mapping(raw, "parameter")
            if set(parameter) - {"name", "in", "required", "description", "schema", "style", "explode"}:
                raise OpenApiImportError("parameter contains unsupported fields or reference")
            name, location = parameter.get("name"), parameter.get("in")
            if not isinstance(name, str) or not _PARAMETER.fullmatch(name) or _secret_name(name):
                raise OpenApiImportError("parameter name is invalid or credential-shaped")
            if location not in {"path", "query"}:
                raise OpenApiImportError("only path and query parameters are supported")
            if type(parameter.get("required", False)) is not bool:
                raise OpenApiImportError("parameter required must be boolean")
            if location == "path" and parameter.get("required") is not True:
                raise OpenApiImportError("path parameters must be explicitly required")
            expected_style = "simple" if location == "path" else "form"
            if parameter.get("style", expected_style) != expected_style:
                raise OpenApiImportError("parameter serialization style is unsupported")
            if "explode" in parameter and type(parameter["explode"]) is not bool:
                raise OpenApiImportError("parameter explode must be boolean")
            parameter["schema"] = _schema(parameter.get("schema"), scalar=True)
            key = (location, name)
            if key in seen:
                raise OpenApiImportError("parameter is duplicated within one scope")
            seen.add(key)
            merged[key] = parameter
    values = tuple(merged.values())
    names = [entry["name"] for entry in values]
    if len(names) != len(set(names)):
        raise OpenApiImportError("path and query parameters cannot share an argument name")
    placeholders = re.findall(r"\{([^{}]+)\}", path)
    if set(placeholders) != {entry["name"] for entry in values if entry["in"] == "path"}:
        raise OpenApiImportError("path placeholders must exactly match declared path parameters")
    stripped = re.sub(r"\{[^{}]+\}", "segment", path)
    if "{" in stripped or "}" in stripped:
        raise OpenApiImportError("malformed path placeholder")
    schema = {"type": "object", "properties": {entry["name"]: entry["schema"] for entry in values},
              "required": [entry["name"] for entry in values if entry.get("required")], "additionalProperties": False}
    return schema, values


def _response_schemas(operation: Mapping[str, Any]) -> dict[str, dict]:
    responses = _mapping(operation.get("responses"), "responses")
    schemas = {}
    for status, raw in responses.items():
        if not isinstance(status, str) or not re.fullmatch(r"[1-5][0-9][0-9]|default", status):
            raise OpenApiImportError("response status must be explicit; wildcard status is unsupported")
        response = _mapping(raw, "response")
        if "$ref" in response:
            raise OpenApiImportError("response references are unsupported")
        if not status.startswith("2"):
            continue
        if set(response) - {"description", "content", "headers"} or response.get("headers"):
            raise OpenApiImportError("success response contains unsupported fields")
        content = _mapping(response.get("content"), "success response content")
        if set(content) != {"application/json"}:
            raise OpenApiImportError("success response must declare only application/json")
        media = _mapping(content["application/json"], "response media")
        if set(media) != {"schema"}:
            raise OpenApiImportError("response media supports a schema only")
        schema = _schema(media["schema"])
        if schema["type"] != "object":
            raise OpenApiImportError("success response must be a JSON object")
        schemas[status] = schema
    if not schemas or len({content_digest(schema) for schema in schemas.values()}) != 1:
        raise OpenApiImportError("all declared success statuses require the same object schema")
    return schemas


def _security(document: Mapping[str, Any], operation: Mapping[str, Any], auth_headers: Any) -> None:
    requirements = operation.get("security", document.get("security", []))
    if not isinstance(requirements, list):
        raise OpenApiImportError("security requirements must be a list")
    components = _mapping(document.get("components", {}), "components")
    schemes = _mapping(components.get("securitySchemes", {}), "security schemes")
    for requirement in requirements:
        requirement = _mapping(requirement, "security requirement")
        for name, scopes in requirement.items():
            scheme = _mapping(schemes.get(name), "declared security scheme")
            if "$ref" in scheme:
                raise OpenApiImportError("security scheme references are unsupported")
            if not isinstance(scopes, list) or scopes:
                raise OpenApiImportError("OAuth scopes are unsupported")
            if scheme.get("type") == "apiKey":
                if scheme.get("in") != "header" or not isinstance(scheme.get("name"), str):
                    raise OpenApiImportError("API credentials must use application-owned headers")
            elif scheme.get("type") != "http" or scheme.get("scheme") not in {"basic", "bearer"}:
                raise OpenApiImportError("security scheme is unsupported")
            if auth_headers is None:
                raise OpenApiImportError("secured operation requires an application-owned header resolver")


def import_openapi_operations(
    catalog: WorkflowOperationCatalog,
    document: Mapping[str, Any] | str,
    *,
    integration_id: str,
    server_url: str,
    allowed_hosts: Sequence[str],
    reviewed_operation_ids: Sequence[str],
    approve_spec_import: bool = False,
    auth_headers: Callable[[], Mapping[str, str]] | None = None,
    transport: Callable[..., OpenApiHttpResponse] | None = None,
    timeout_seconds: float = 15.0,
    max_response_bytes: int = 1_048_576,
    allow_private_hosts: bool = False,
) -> tuple[WorkflowOperation, ...]:
    """Register only operator-reviewed GET operations; import approval never authorizes dispatch.

    ``transport`` and ``auth_headers`` are trusted application code, never model arguments. An
    injected transport owns network admission itself; response bounds and redirect checks still
    apply. The default transport validates DNS addresses and pins the chosen TLS connection.
    """
    if approve_spec_import is not True:
        raise OpenApiImportError("explicit specification import approval is required")
    if not isinstance(integration_id, str) or not _IDENTIFIER.fullmatch(integration_id):
        raise OpenApiImportError("integration_id must be a bounded identifier")
    if isinstance(allowed_hosts, (str, bytes)) or not isinstance(allowed_hosts, Sequence) or not allowed_hosts:
        raise OpenApiImportError("an exact host allowlist is required")
    if any(not isinstance(host, str) or host != host.lower() or not re.fullmatch(r"[a-z0-9.:-]+", host)
           or "*" in host for host in allowed_hosts):
        raise OpenApiImportError("allowed_hosts must contain lowercase exact hostnames")
    if type(allow_private_hosts) is not bool:
        raise OpenApiImportError("allow_private_hosts must be boolean")
    if isinstance(timeout_seconds, bool) or not isinstance(timeout_seconds, (int, float)) or not math.isfinite(timeout_seconds) or not 0.1 <= timeout_seconds <= 120:
        raise OpenApiImportError("timeout_seconds must be finite and between 0.1 and 120")
    if type(max_response_bytes) is not int or not 1 <= max_response_bytes <= 4_000_000:
        raise OpenApiImportError("max_response_bytes must be between 1 and 4000000")
    if auth_headers is not None and not callable(auth_headers):
        raise OpenApiImportError("auth_headers must be an application-owned callable")
    if transport is not None and not callable(transport):
        raise OpenApiImportError("transport must be an application-owned callable")
    selected_server = _server(server_url, allowed_hosts, allow_private_hosts)
    if isinstance(document, str):
        if len(document.encode("utf-8")) > _MAX_DOCUMENT_BYTES:
            raise OpenApiImportError("document exceeds its byte bound")
        try:
            document = json.loads(document, object_pairs_hook=_unique_object)
        except (ValueError, RecursionError):
            raise OpenApiImportError("document must be valid JSON") from None
    document = _mapping(_json(document, "document"), "document")
    if not re.fullmatch(r"3\.0\.\d+", str(document.get("openapi", ""))):
        raise OpenApiImportError("only OpenAPI 3.0.x JSON is supported")
    info = _mapping(document.get("info"), "OpenAPI info")
    if any(not isinstance(info.get(name), str) or not info[name].strip() for name in ("title", "version")):
        raise OpenApiImportError("OpenAPI info requires title and version")
    servers = document.get("servers")
    if not isinstance(servers, list) or not any(isinstance(item, dict) and isinstance(item.get("url"), str) and item["url"].rstrip("/") == selected_server for item in servers):
        raise OpenApiImportError("operator-selected server must exactly match a declared fixed server")
    if document.get("webhooks") or "$ref" in document:
        raise OpenApiImportError("webhooks and root references are unsupported")
    if isinstance(reviewed_operation_ids, (str, bytes)) or not isinstance(reviewed_operation_ids, Sequence) or not 1 <= len(reviewed_operation_ids) <= 4096:
        raise OpenApiImportError("reviewed_operation_ids must be an explicit bounded nonempty sequence")
    if any(not isinstance(item, str) or not _IDENTIFIER.fullmatch(item) for item in reviewed_operation_ids) or len(set(reviewed_operation_ids)) != len(reviewed_operation_ids):
        raise OpenApiImportError("reviewed operation identifiers must be valid and unique")
    wanted = set(reviewed_operation_ids)
    paths = _mapping(document.get("paths"), "paths")
    if len(paths) > 4096:
        raise OpenApiImportError("document has too many paths")
    prepared = []
    found = set()
    existing_operations = catalog.operations()
    existing = {operation.operation_id for operation in existing_operations}
    existing_integrations = {operation.integration_id for operation in existing_operations}
    if len(existing) + len(wanted) > MAX_WORKFLOW_OPERATIONS:
        raise OpenApiImportError("import would exceed operation catalogue capacity")
    if integration_id not in existing_integrations and len(existing_integrations) >= MAX_WORKFLOW_INTEGRATIONS:
        raise OpenApiImportError("import would exceed integration catalogue capacity")
    document_digest = content_digest(document)
    for path, raw_item in paths.items():
        item = _mapping(raw_item, "path item")
        for method, raw_operation in item.items():
            if method in {"parameters", "summary", "description", "servers", "$ref"} or method.startswith("x-"):
                continue
            if not isinstance(raw_operation, Mapping) or not isinstance(raw_operation.get("operationId"), str) or raw_operation["operationId"] not in wanted:
                continue
            operation = dict(raw_operation)
            operation_id = operation["operationId"]
            if operation_id in found:
                raise OpenApiImportError("reviewed operationId is duplicated")
            found.add(operation_id)
            if method != "get":
                raise OpenApiImportError("reviewed operations must use GET")
            if "$ref" in item or "$ref" in operation or "servers" in item or "servers" in operation:
                raise OpenApiImportError("path references and per-operation server overrides are unsupported")
            if (not isinstance(path, str) or not path.startswith("/") or path.startswith("//")
                    or not re.fullmatch(r"/[A-Za-z0-9_./{}~!$&'()*+,;=:@-]*", path)
                    or any(segment in {".", ".."} for segment in path.split("/"))):
                raise OpenApiImportError("operation path must be fixed absolute path without encoded or relative segments")
            if operation.get("requestBody") is not None or operation.get("callbacks"):
                raise OpenApiImportError("request bodies and callbacks are unsupported")
            _security(document, operation, auth_headers)
            input_schema, parameters = _parameters(path, item, operation)
            responses = _response_schemas(operation)
            registered_id = f"{integration_id}.{operation_id}"
            if registered_id in existing:
                raise OpenApiImportError("operation is already registered")
            description = operation.get("description", operation.get("summary", operation_id))
            if not isinstance(description, str):
                raise OpenApiImportError("operation description must be text")
            description += f"\nReviewed OpenAPI document: {document_digest}; fixed server: {selected_server}"
            descriptor = WorkflowOperation(registered_id, description, input_schema,
                                           next(iter(responses.values())), integration_id,
                                           auth_mode="application_header" if auth_headers else "none",
                                           read_only=True, implementation="live_adapter")
            prepared.append((descriptor, path, parameters, responses))
    if found != wanted:
        raise OpenApiImportError("one or more reviewed operation IDs are missing from the document")

    def executor_for(descriptor, path, parameters, responses):
        checked_input = descriptor.to_dict()["input_schema"]
        def execute(arguments, *, approve_source_dispatch=False, retrieved_at=None):
            if approve_source_dispatch is not True:
                raise OpenApiImportError("explicit source dispatch approval is required")
            _validate(checked_input, arguments, "arguments")
            if retrieved_at is not None and not isinstance(retrieved_at, str):
                raise OpenApiImportError("retrieved_at must be text when supplied")
            target = path
            query = []
            for parameter in parameters:
                name = parameter["name"]
                if name not in arguments:
                    continue
                value = arguments[name]
                text = ("true" if value else "false") if isinstance(value, bool) else str(value)
                if parameter["in"] == "path":
                    if text in {".", ".."}:
                        raise OpenApiImportError("path argument cannot be a relative segment")
                    target = target.replace("{" + name + "}", quote(text, safe=""))
                else:
                    query.append((name, text))
            url = selected_server + target + (("?" + urlencode(query)) if query else "")
            if len(url.encode("utf-8")) > 8192:
                raise OpenApiImportError("request URL exceeds its byte bound")
            headers = {"Accept": "application/json"}
            if auth_headers:
                try:
                    resolved = auth_headers()
                    if not isinstance(resolved, Mapping) or not resolved or len(resolved) > 32:
                        raise ValueError()
                    for key, value in resolved.items():
                        if (not isinstance(key, str) or not re.fullmatch(r"[A-Za-z0-9-]{1,128}", key)
                                or key.lower() in {"host", "connection", "content-length", "transfer-encoding", "accept"}
                                or not isinstance(value, str) or not value or len(value) > 8192
                                or any(ord(ch) < 32 or ord(ch) == 127 for ch in value)):
                            raise ValueError()
                        headers[key] = value
                except Exception:
                    raise OpenApiImportError("application header resolution failed; credentials withheld") from None
            if transport is None:
                response = _http_get(url, headers=headers, timeout_seconds=timeout_seconds,
                                     max_response_bytes=max_response_bytes, allow_private_hosts=allow_private_hosts)
            else:
                try:
                    response = transport(url, headers=headers, timeout_seconds=timeout_seconds,
                                         max_response_bytes=max_response_bytes)
                except Exception:
                    raise OpenApiImportError("application transport failed; credentials and endpoint details withheld") from None
            if not isinstance(response, OpenApiHttpResponse) or type(response.status) is not int:
                raise OpenApiImportError("transport returned an invalid response")
            if 300 <= response.status < 400 or (response.final_url is not None and response.final_url != url):
                raise OpenApiImportError("redirected responses are forbidden")
            if str(response.status) not in responses:
                raise OpenApiImportError("HTTP status is not a declared success response")
            if not isinstance(response.body, bytes) or len(response.body) > max_response_bytes:
                raise OpenApiImportError("response exceeds its byte bound or is not bytes")
            if not isinstance(response.headers, Mapping) or any(not isinstance(key, str) for key in response.headers):
                raise OpenApiImportError("transport returned invalid response headers")
            content_type = next((v for k, v in response.headers.items() if k.lower() == "content-type"), "")
            if not isinstance(content_type, str) or content_type.split(";", 1)[0].strip().lower() != "application/json":
                raise OpenApiImportError("response content type must be application/json")
            try:
                value = json.loads(response.body, object_pairs_hook=_unique_object)
            except (ValueError, RecursionError):
                raise OpenApiImportError("response must be valid JSON") from None
            value = _json(value, "response", maximum=max_response_bytes)
            _validate(responses[str(response.status)], value, "response")
            return WorkflowOperationResult(value=value, receipt={
                "adapter": "reviewed_openapi_get", "operation_id": descriptor.operation_id,
                "integration_id": integration_id, "document_digest": document_digest,
                "request_digest": content_digest({"url": url, "arguments": dict(arguments)}),
                "response_digest": content_digest(value), "http_status": response.status,
                "response_bytes": len(response.body), "retrieved_at": retrieved_at,
                "verification": "schema_checked_response", "live_verified": transport is None,
            })
        return execute

    for descriptor, path, parameters, responses in prepared:
        catalog.register(descriptor, executor_for(descriptor, path, parameters, responses))
    return tuple(entry[0] for entry in prepared)
