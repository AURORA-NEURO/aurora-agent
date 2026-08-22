"""Policy-gated HTTP transport for caller-managed autonomous connectors.

The autonomous connector registry intentionally does not discover providers or retain keys. This
module supplies the missing transport seam without changing that boundary: an embedding supplies
an endpoint resolver and, when needed, a transient header resolver that closes over its own
credential session. The adapter enforces an explicit host/scheme/method policy, bounded request and
response bytes, a no-redirect default, and a finite timeout. It returns only JSON evidence or a
digest/status projection; raw headers, credentials, and response bytes never enter receipts.

It is provider-neutral rather than provider-naive. Domain-specific authentication, endpoint paths,
pagination, response interpretation, and claim validation remain caller-owned resolver logic.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import hashlib
import ipaddress
import json
from typing import Any, Callable, Mapping, Sequence
from urllib.error import HTTPError, URLError
from urllib.parse import parse_qsl, urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener

from .autonomous_connectors import AutonomousConnectorObservation
from .errors import ArgumentError


AUTONOMOUS_HTTP_CONNECTOR_ADAPTER_SCHEMA = "bioprism-python-autonomous-http-connector-adapter/0.1"
MAX_AUTONOMOUS_HTTP_REQUEST_BYTES = 2_000_000
MAX_AUTONOMOUS_HTTP_RESPONSE_BYTES = 2_000_000
MAX_AUTONOMOUS_HTTP_HEADERS = 64
MAX_AUTONOMOUS_HTTP_HEADER_BYTES = 8_192
MAX_AUTONOMOUS_HTTP_URL_BYTES = 8_192
MAX_AUTONOMOUS_HTTP_TIMEOUT_SECONDS = 120.0
MAX_AUTONOMOUS_HTTP_PAGES = 64
MAX_AUTONOMOUS_HTTP_ITEMS = 4_096
MAX_AUTONOMOUS_HTTP_PAGINATED_ITEM_BYTES = 1_500_000
AUTONOMOUS_HTTP_METHODS = ("GET", "POST", "PUT", "PATCH", "DELETE")
AUTONOMOUS_HTTP_FAILURE_CLASSES = (
    "auth_refused",
    "not_found",
    "rate_limited",
    "timeout",
    "transport_error",
    "http_4xx",
    "http_5xx",
    "invalid_json",
    "response_too_large",
)
AUTONOMOUS_HTTP_PAGINATION_FAILURE_CLASSES = (
    "page_shape",
    "page_limit",
    "item_limit",
    "item_bytes_limit",
    "cursor_cycle",
    "page_transport",
)
_SECRET_MARKERS = frozenset(
    {
        "apikey",
        "authorization",
        "bearer",
        "credential",
        "credentials",
        "password",
        "secret",
        "secretkey",
        "token",
        "accesstoken",
        "refreshtoken",
        "privatekey",
        "clientsecret",
        "gsk",
        "sk",
    }
)


def _normalized_field(value: str) -> str:
    return "".join(character for character in value.lower() if character.isalnum())


def _contains_secret_field(name: str) -> bool:
    normalized = _normalized_field(name)
    return normalized in _SECRET_MARKERS or normalized.startswith("gsk") or normalized.startswith("skproj")


def _safe_json(value: Any, *, name: str, maximum: int, depth: int = 0) -> Any:
    if depth > 32:
        raise ArgumentError(f"{name} is too deeply nested")
    if value is None or isinstance(value, (str, bool, int, float)):
        if isinstance(value, float) and (value != value or value in {float("inf"), float("-inf")}):
            raise ArgumentError(f"{name} contains a non-finite number")
        return value
    if isinstance(value, (list, tuple)):
        result = [_safe_json(item, name=name, maximum=maximum, depth=depth + 1) for item in value]
    elif isinstance(value, Mapping):
        result = {}
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip() or "\x00" in key:
                raise ArgumentError(f"{name} contains an invalid JSON field")
            if _contains_secret_field(key):
                raise ArgumentError(f"{name} contains credential-shaped fields")
            result[key] = _safe_json(child, name=name, maximum=maximum, depth=depth + 1)
    else:
        raise ArgumentError(f"{name} must be JSON-safe")
    try:
        encoded = json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} is not JSON-serializable") from error
    if len(encoded) > maximum:
        raise ArgumentError(f"{name} exceeds {maximum} bytes")
    return result


def _bounded_text(name: str, value: Any, maximum: int) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value


def _bounded_header(name: str, value: Any) -> str:
    value = _bounded_text(name, value, MAX_AUTONOMOUS_HTTP_HEADER_BYTES)
    if "\r" in value or "\n" in value:
        raise ArgumentError(f"{name} contains an unsafe header value")
    return value


def _loopback(host: str) -> bool:
    normalized = host.strip("[]").lower()
    if normalized == "localhost":
        return True
    try:
        return ipaddress.ip_address(normalized).is_loopback
    except ValueError:
        return False


def _host_allowed(host: str, allowed_hosts: tuple[str, ...], allow_loopback: bool) -> bool:
    normalized = host.lower().strip("[]")
    if allow_loopback and _loopback(normalized):
        return True
    for allowed in allowed_hosts:
        if allowed.startswith("*.") and normalized.endswith(allowed[1:]) and normalized != allowed[2:]:
            return True
        if normalized == allowed:
            return True
    return False


@dataclass(frozen=True, slots=True)
class AutonomousHttpConnectorPolicy:
    """Explicit transport admission; no network is opened by constructing this policy."""

    allowed_hosts: tuple[str, ...] = ()
    require_https: bool = True
    allow_loopback: bool = False
    timeout_seconds: float = 30.0
    max_request_bytes: int = MAX_AUTONOMOUS_HTTP_REQUEST_BYTES
    max_response_bytes: int = MAX_AUTONOMOUS_HTTP_RESPONSE_BYTES
    allowed_methods: tuple[str, ...] = AUTONOMOUS_HTTP_METHODS

    def __post_init__(self) -> None:
        if not isinstance(self.allowed_hosts, tuple) or len(self.allowed_hosts) > 128:
            raise ArgumentError("HTTP connector allowed_hosts is outside its bound")
        hosts: list[str] = []
        for host in self.allowed_hosts:
            host = _bounded_text("HTTP connector allowed host", host, 512).lower().strip("[]")
            if "://" in host or "/" in host or "@" in host:
                raise ArgumentError("HTTP connector allowed host must not contain a scheme or path")
            if host.startswith("*.") and len(host) <= 2:
                raise ArgumentError("HTTP connector wildcard host is malformed")
            hosts.append(host)
        if not isinstance(self.require_https, bool) or not isinstance(self.allow_loopback, bool):
            raise ArgumentError("HTTP connector scheme policy must be boolean")
        if isinstance(self.timeout_seconds, bool) or not isinstance(self.timeout_seconds, (int, float)) or not 0.1 <= float(self.timeout_seconds) <= MAX_AUTONOMOUS_HTTP_TIMEOUT_SECONDS:
            raise ArgumentError("HTTP connector timeout_seconds is outside its bound")
        for name, value, maximum in (
            ("max_request_bytes", self.max_request_bytes, MAX_AUTONOMOUS_HTTP_REQUEST_BYTES),
            ("max_response_bytes", self.max_response_bytes, MAX_AUTONOMOUS_HTTP_RESPONSE_BYTES),
        ):
            if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
                raise ArgumentError(f"HTTP connector {name} is outside its bound")
        if not isinstance(self.allowed_methods, tuple) or not self.allowed_methods:
            raise ArgumentError("HTTP connector allowed_methods must be non-empty")
        methods = tuple(_bounded_text("HTTP connector method", method, 16).upper() for method in self.allowed_methods)
        if any(method not in AUTONOMOUS_HTTP_METHODS for method in methods) or len(set(methods)) != len(methods):
            raise ArgumentError("HTTP connector allowed_methods contains an unsupported or duplicate method")
        object.__setattr__(self, "allowed_hosts", tuple(hosts))
        object.__setattr__(self, "timeout_seconds", float(self.timeout_seconds))
        object.__setattr__(self, "allowed_methods", methods)


@dataclass(frozen=True, slots=True)
class AutonomousHttpConnectorRequest:
    """Transient endpoint request produced by a caller-owned resolver.

    ``headers`` may contain an authorization header because it never enters a durable request or
    receipt. Applications should normally provide it through ``header_resolver`` instead of
    embedding it in a reusable endpoint object.
    """

    method: str
    url: str
    body: Any = None
    headers: Mapping[str, str] = field(default_factory=dict)

    def __post_init__(self) -> None:
        method = _bounded_text("HTTP connector method", self.method, 16).upper()
        if method not in AUTONOMOUS_HTTP_METHODS:
            raise ArgumentError("HTTP connector method is unsupported")
        url = _bounded_text("HTTP connector URL", self.url, MAX_AUTONOMOUS_HTTP_URL_BYTES)
        if any(character.isspace() for character in url):
            raise ArgumentError("HTTP connector URL contains whitespace")
        if not isinstance(self.headers, Mapping) or len(self.headers) > MAX_AUTONOMOUS_HTTP_HEADERS:
            raise ArgumentError("HTTP connector headers are outside their bound")
        headers: dict[str, str] = {}
        for raw_name, raw_value in self.headers.items():
            name = _bounded_text("HTTP connector header name", raw_name, 256)
            if any(character.isspace() for character in name) or ":" in name or "\r" in name or "\n" in name:
                raise ArgumentError("HTTP connector header name is unsafe")
            for existing in tuple(headers):
                if existing.lower() == name.lower():
                    del headers[existing]
            headers[name] = _bounded_header("HTTP connector header value", raw_value)
        body = None if self.body is None else _safe_json(self.body, name="HTTP connector request body", maximum=MAX_AUTONOMOUS_HTTP_REQUEST_BYTES)
        if method in {"GET", "DELETE"} and body is not None:
            raise ArgumentError("HTTP connector GET/DELETE requests cannot contain a body")
        object.__setattr__(self, "method", method)
        object.__setattr__(self, "url", url)
        object.__setattr__(self, "headers", headers)
        object.__setattr__(self, "body", body)


@dataclass(frozen=True, slots=True)
class AutonomousHttpConnectorPage:
    """A parser-owned, transient page projection.

    ``next_cursor`` is intentionally present only while the pagination loop is running. It is
    replaced by ``next_cursor_digest`` in the returned observation and never reaches a receipt.
    """

    items: Sequence[Any]
    next_cursor: str | None = None

    def __post_init__(self) -> None:
        if not isinstance(self.items, Sequence) or isinstance(self.items, (str, bytes)) or len(self.items) > MAX_AUTONOMOUS_HTTP_ITEMS:
            raise ArgumentError("HTTP connector page items are outside their bound")
        normalized = tuple(
            _safe_json(item, name="HTTP connector page item", maximum=MAX_AUTONOMOUS_HTTP_RESPONSE_BYTES)
            for item in self.items
        )
        if self.next_cursor is not None:
            _bounded_text("HTTP connector next cursor", self.next_cursor, MAX_AUTONOMOUS_HTTP_URL_BYTES)
        object.__setattr__(self, "items", normalized)


class _PageShapeError(ValueError):
    pass


EndpointResolver = Callable[[Any, Mapping[str, Any]], AutonomousHttpConnectorRequest]
HeaderResolver = Callable[[Any, Mapping[str, Any]], Mapping[str, str]]
OpenRequest = Callable[[Request, float], Any]
PageParser = Callable[[Any, int], AutonomousHttpConnectorPage]


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *_: Any, **__: Any) -> None:
        return None


def _failure_for_status(status: int) -> tuple[str, str]:
    if status in {401, 403}:
        return "refused", "auth_refused"
    if status == 404:
        return "refused", "not_found"
    if status in {408, 425, 429}:
        return "error", "rate_limited" if status == 429 else "timeout"
    if 400 <= status < 500:
        return "refused", "http_4xx"
    return "error", "http_5xx"


def _body_digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def create_autonomous_http_connector_executor(
    endpoint_resolver: EndpointResolver,
    *,
    policy: AutonomousHttpConnectorPolicy | None = None,
    header_resolver: HeaderResolver | None = None,
    opener: OpenRequest | None = None,
) -> Callable[[Any, Mapping[str, Any]], AutonomousConnectorObservation]:
    """Create a bounded executor suitable for ``AutonomousConnectorRegistration``.

    The endpoint and header resolvers are called only for the transient dispatch. The adapter does
    not copy them into manifests, checkpoints, receipts, or error messages. ``opener`` is an
    injectable transport hook for deterministic tests; production defaults to a no-redirect
    ``urllib`` opener.
    """

    if not callable(endpoint_resolver):
        raise ArgumentError("HTTP connector endpoint_resolver must be callable")
    if header_resolver is not None and not callable(header_resolver):
        raise ArgumentError("HTTP connector header_resolver must be callable")
    policy = policy or AutonomousHttpConnectorPolicy()
    if not isinstance(policy, AutonomousHttpConnectorPolicy):
        raise ArgumentError("HTTP connector policy is malformed")
    default_opener = build_opener(_NoRedirect())

    def execute(manifest: Any, request: Mapping[str, Any]) -> AutonomousConnectorObservation:
        if not isinstance(request, Mapping):
            raise ArgumentError("HTTP connector dispatch request must be a mapping")
        endpoint = endpoint_resolver(manifest, request)
        if not isinstance(endpoint, AutonomousHttpConnectorRequest):
            raise ArgumentError("HTTP connector endpoint resolver returned an invalid request")
        split = urlsplit(endpoint.url)
        host = split.hostname
        if split.scheme not in {"http", "https"} or host is None or split.username is not None or split.password is not None or split.fragment:
            raise ArgumentError("HTTP connector URL failed transport admission")
        if policy.require_https and split.scheme != "https":
            raise ArgumentError("HTTP connector requires HTTPS")
        if not _host_allowed(host, policy.allowed_hosts, policy.allow_loopback):
            raise ArgumentError("HTTP connector host is outside its allowlist")
        if any(_contains_secret_field(key) for key, _ in parse_qsl(split.query, keep_blank_values=True)):
            raise ArgumentError("HTTP connector URL query contains credential-shaped fields")
        if endpoint.method not in policy.allowed_methods:
            raise ArgumentError("HTTP connector method is outside its policy")
        extra_headers: Mapping[str, str] = {}
        if header_resolver is not None:
            extra_headers = header_resolver(manifest, request)
            if not isinstance(extra_headers, Mapping) or len(extra_headers) > MAX_AUTONOMOUS_HTTP_HEADERS:
                raise ArgumentError("HTTP connector resolved headers are outside their bound")
        headers = dict(endpoint.headers)
        for raw_name, raw_value in extra_headers.items():
            name = _bounded_text("HTTP connector resolved header name", raw_name, 256)
            if any(character.isspace() for character in name) or ":" in name or "\r" in name or "\n" in name:
                raise ArgumentError("HTTP connector resolved header name is unsafe")
            for existing in tuple(headers):
                if existing.lower() == name.lower():
                    del headers[existing]
            headers[name] = _bounded_header("HTTP connector resolved header value", raw_value)
        if len(headers) > MAX_AUTONOMOUS_HTTP_HEADERS:
            raise ArgumentError("HTTP connector resolved headers are outside their bound")
        body = None
        if endpoint.body is not None:
            body = json.dumps(endpoint.body, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
            if len(body) > policy.max_request_bytes:
                raise ArgumentError("HTTP connector request exceeds its byte bound")
        request_object = Request(endpoint.url, data=body, headers={**headers, "User-Agent": "aurora-autonomous-connector/0.1"}, method=endpoint.method)
        try:
            response = (opener or default_opener.open)(request_object, policy.timeout_seconds)
            with response:
                status = int(getattr(response, "status", response.getcode()))
                if not 200 <= status < 300:
                    result_status, failure = _failure_for_status(status)
                    return AutonomousConnectorObservation(
                        value={"status_code": status},
                        status=result_status,
                        failure_class=failure,
                    )
                chunks: list[bytes] = []
                total = 0
                while True:
                    chunk = response.read(min(64 * 1024, policy.max_response_bytes + 1 - total))
                    if not chunk:
                        break
                    remaining = policy.max_response_bytes + 1 - total
                    if len(chunk) > remaining:
                        chunk = chunk[:remaining]
                    chunks.append(chunk)
                    total += len(chunk)
                    if total > policy.max_response_bytes:
                        return AutonomousConnectorObservation(
                            value={"status_code": status, "body_digest": _body_digest(b"".join(chunks))},
                            status="error",
                            failure_class="response_too_large",
                        )
                raw = b"".join(chunks)
                if not raw:
                    return AutonomousConnectorObservation(value=None, status="observed")
                try:
                    value = json.loads(raw.decode("utf-8"))
                    return AutonomousConnectorObservation(value=value, status="observed")
                except (UnicodeDecodeError, json.JSONDecodeError):
                    return AutonomousConnectorObservation(
                        value={
                            "status_code": status,
                            "content_type": _bounded_text("HTTP connector content type", response.headers.get("Content-Type", "application/octet-stream"), 256),
                            "body_digest": _body_digest(raw),
                        },
                        status="partial",
                        failure_class="invalid_json",
                    )
        except HTTPError as error:
            result_status, failure = _failure_for_status(int(error.code))
            return AutonomousConnectorObservation(value={"status_code": int(error.code)}, status=result_status, failure_class=failure)
        except TimeoutError:
            return AutonomousConnectorObservation(value=None, status="error", failure_class="timeout")
        except (URLError, OSError):
            return AutonomousConnectorObservation(value=None, status="error", failure_class="transport_error")

    return execute


def default_autonomous_http_connector_page_parser(value: Any, _page_number: int) -> AutonomousHttpConnectorPage:
    """Parse the strict provider-neutral page shape ``[items]`` or ``{"items": [...]}``.

    Object pages may include a string/null ``next_cursor``. Provider-specific fields are ignored
    by this projection; a provider with a different shape must supply an explicit ``page_parser``.
    """

    if isinstance(value, list):
        return AutonomousHttpConnectorPage(items=tuple(value))
    if not isinstance(value, Mapping):
        raise _PageShapeError("HTTP connector page must be an array or object")
    raw_items = value.get("items")
    if not isinstance(raw_items, list):
        raise _PageShapeError("HTTP connector page object must contain an items array")
    cursor = value.get("next_cursor")
    if cursor is not None and not isinstance(cursor, str):
        raise _PageShapeError("HTTP connector page next_cursor must be a string or null")
    return AutonomousHttpConnectorPage(items=tuple(raw_items), next_cursor=cursor)


def create_autonomous_http_paginated_connector_executor(
    endpoint_resolver: EndpointResolver,
    *,
    policy: AutonomousHttpConnectorPolicy | None = None,
    header_resolver: HeaderResolver | None = None,
    opener: OpenRequest | None = None,
    page_parser: PageParser | None = None,
    max_pages: int = 8,
    max_items: int = 512,
) -> Callable[[Any, Mapping[str, Any]], AutonomousConnectorObservation]:
    """Create a bounded multi-page executor over the single-page HTTP transport.

    The endpoint resolver receives a private ``__autonomous_http_page_cursor`` field after the
    first page. It is an in-process continuation value only; returned observations expose its
    digest, not its contents. A page transport/parser failure after useful items have already
    been collected becomes a metadata-only ``partial`` result instead of discarding progress.
    """

    if page_parser is not None and not callable(page_parser):
        raise ArgumentError("HTTP connector page_parser must be callable")
    if isinstance(max_pages, bool) or not isinstance(max_pages, int) or not 1 <= max_pages <= MAX_AUTONOMOUS_HTTP_PAGES:
        raise ArgumentError("HTTP connector max_pages is outside its bound")
    if isinstance(max_items, bool) or not isinstance(max_items, int) or not 1 <= max_items <= MAX_AUTONOMOUS_HTTP_ITEMS:
        raise ArgumentError("HTTP connector max_items is outside its bound")
    single_page = create_autonomous_http_connector_executor(
        endpoint_resolver,
        policy=policy,
        header_resolver=header_resolver,
        opener=opener,
    )
    parse_page = page_parser or default_autonomous_http_connector_page_parser
    cursor_field = "__autonomous_http_page_cursor"

    def summary(items: list[Any], page_count: int, cursor: str | None, *, complete: bool) -> dict[str, Any]:
        return {
            "items": list(items),
            "item_count": len(items),
            "page_count": page_count,
            "complete": complete,
            "next_cursor_digest": None if cursor is None else _body_digest(cursor.encode("utf-8")),
        }

    def item_bytes(items: list[Any]) -> int:
        return len(json.dumps(items, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8"))

    def execute(manifest: Any, request: Mapping[str, Any]) -> AutonomousConnectorObservation:
        if cursor_field in request:
            raise ArgumentError("HTTP connector request uses a reserved pagination field")
        items: list[Any] = []
        cursor: str | None = None
        seen_cursors: set[str] = set()
        for page_number in range(max_pages):
            page_request = dict(request)
            if cursor is not None:
                page_request[cursor_field] = cursor
            observation = single_page(manifest, page_request)
            if observation.status != "observed":
                if not items:
                    return observation
                return AutonomousConnectorObservation(
                    value=summary(items, page_number + 1, cursor, complete=False),
                    status="partial",
                    failure_class=observation.failure_class or "page_transport",
                )
            try:
                page = parse_page(observation.value, page_number)
            except _PageShapeError:
                return AutonomousConnectorObservation(
                    value=summary(items, page_number + 1, cursor, complete=False),
                    status="partial",
                    failure_class="page_shape",
                )
            if not isinstance(page, AutonomousHttpConnectorPage):
                raise ArgumentError("HTTP connector page_parser returned an invalid page")
            remaining = max_items - len(items)
            if len(page.items) > remaining:
                limited = items + list(page.items[:remaining])
                if item_bytes(limited) > MAX_AUTONOMOUS_HTTP_PAGINATED_ITEM_BYTES:
                    return AutonomousConnectorObservation(
                        value=summary(items, page_number + 1, page.next_cursor, complete=False),
                        status="partial",
                        failure_class="item_bytes_limit",
                    )
                items.extend(page.items[:remaining])
                return AutonomousConnectorObservation(
                    value=summary(items, page_number + 1, page.next_cursor, complete=False),
                    status="partial",
                    failure_class="item_limit",
                )
            candidate = items + list(page.items)
            if item_bytes(candidate) > MAX_AUTONOMOUS_HTTP_PAGINATED_ITEM_BYTES:
                return AutonomousConnectorObservation(
                    value=summary(items, page_number + 1, page.next_cursor, complete=False),
                    status="partial",
                    failure_class="item_bytes_limit",
                )
            items.extend(page.items)
            cursor = page.next_cursor
            if cursor is None:
                return AutonomousConnectorObservation(value=summary(items, page_number + 1, None, complete=True), status="observed")
            if cursor in seen_cursors:
                return AutonomousConnectorObservation(
                    value=summary(items, page_number + 1, cursor, complete=False),
                    status="partial",
                    failure_class="cursor_cycle",
                )
            seen_cursors.add(cursor)
        return AutonomousConnectorObservation(
            value=summary(items, max_pages, cursor, complete=False),
            status="partial",
            failure_class="page_limit",
        )

    return execute


__all__ = [
    "AUTONOMOUS_HTTP_CONNECTOR_ADAPTER_SCHEMA",
    "MAX_AUTONOMOUS_HTTP_REQUEST_BYTES",
    "MAX_AUTONOMOUS_HTTP_RESPONSE_BYTES",
    "MAX_AUTONOMOUS_HTTP_HEADERS",
    "MAX_AUTONOMOUS_HTTP_HEADER_BYTES",
    "MAX_AUTONOMOUS_HTTP_URL_BYTES",
    "MAX_AUTONOMOUS_HTTP_TIMEOUT_SECONDS",
    "MAX_AUTONOMOUS_HTTP_PAGES",
    "MAX_AUTONOMOUS_HTTP_ITEMS",
    "MAX_AUTONOMOUS_HTTP_PAGINATED_ITEM_BYTES",
    "AUTONOMOUS_HTTP_METHODS",
    "AUTONOMOUS_HTTP_FAILURE_CLASSES",
    "AUTONOMOUS_HTTP_PAGINATION_FAILURE_CLASSES",
    "AutonomousHttpConnectorPolicy",
    "AutonomousHttpConnectorRequest",
    "AutonomousHttpConnectorPage",
    "default_autonomous_http_connector_page_parser",
    "create_autonomous_http_connector_executor",
    "create_autonomous_http_paginated_connector_executor",
]
