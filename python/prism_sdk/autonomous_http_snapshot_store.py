"""Bounded HTTP persistence for metadata-only JSON snapshot text stores.

The store is intentionally schema-neutral.  Learning, run traces, evaluator state, goals, and
queues validate their own JSON before this adapter sees it.  This layer contributes endpoint
admission, bounded request/response handling, transient header resolution, no redirects, timeout
classification, and conditional-write fencing; it does not claim database durability or
distributed consensus.
"""

from __future__ import annotations

import ipaddress
import json
from typing import Any, Callable, Mapping, Sequence
from urllib.error import HTTPError, URLError
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener

from .authoring import canonical_json
from .errors import ArgumentError, TransportError


AUTONOMOUS_HTTP_SNAPSHOT_STORE_SCHEMA = "bioprism-python-autonomous-http-snapshot-store/0.1"
MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_RESOURCE_BYTES = 512
MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_REQUEST_BYTES = 4_000_000
MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_RESPONSE_BYTES = 4_000_000
MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_TIMEOUT_SECONDS = 120.0
MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_HEADER_COUNT = 64
MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_HEADER_BYTES = 8_192


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *_args: Any, **_kwargs: Any) -> None:
        return None


def _bounded_text(name: str, value: Any, maximum: int) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
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
    normalized = host.strip("[]").lower()
    if allow_loopback and _loopback(normalized):
        return True
    return any(
        (allowed.startswith("*.") and normalized.endswith(allowed[1:]) and normalized != allowed[2:])
        or normalized == allowed
        for allowed in allowed_hosts
    )


def _validate_snapshot_text(value: str, maximum: int) -> str:
    if not isinstance(value, str) or len(value.encode("utf-8")) > maximum:
        raise ArgumentError("HTTP snapshot store snapshot exceeds its request bound")
    try:
        parsed = json.loads(value)
    except json.JSONDecodeError as error:
        raise ArgumentError("HTTP snapshot store snapshot must be valid JSON") from error
    if not isinstance(parsed, dict):
        raise ArgumentError("HTTP snapshot store snapshot must be a JSON object")
    if canonical_json(parsed) != value:
        raise ArgumentError("HTTP snapshot store snapshot must use canonical JSON")
    return value


def _headers(value: Mapping[str, str]) -> dict[str, str]:
    if not isinstance(value, Mapping) or len(value) > MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_HEADER_COUNT:
        raise ArgumentError("HTTP snapshot store headers are outside their bound")
    result: dict[str, str] = {}
    for raw_name, raw_value in value.items():
        name = _bounded_text("HTTP snapshot store header name", raw_name, 256)
        if any(character.isspace() for character in name) or ":" in name or "\r" in name or "\n" in name:
            raise ArgumentError("HTTP snapshot store header name is unsafe")
        text = _bounded_text("HTTP snapshot store header value", raw_value, MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_HEADER_BYTES)
        if "\r" in text or "\n" in text:
            raise ArgumentError("HTTP snapshot store header value is unsafe")
        for existing in tuple(result):
            if existing.lower() == name.lower():
                del result[existing]
        result[name] = text
    return result


class AutonomousHttpSnapshotTextStore:
    """A caller-owned remote text store with atomic conditional PUT support."""

    def __init__(
        self,
        endpoint: str,
        resource: str,
        *,
        allowed_hosts: Sequence[str] = (),
        require_https: bool = True,
        allow_loopback: bool = False,
        timeout_seconds: float = 30.0,
        max_request_bytes: int = MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_REQUEST_BYTES,
        max_response_bytes: int = MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_RESPONSE_BYTES,
        header_resolver: Callable[[Mapping[str, Any]], Mapping[str, str]] | None = None,
        opener: Callable[[Request, float], Any] | None = None,
    ) -> None:
        if not isinstance(require_https, bool) or not isinstance(allow_loopback, bool):
            raise ArgumentError("HTTP snapshot store HTTPS policy must be boolean")
        endpoint_text = _bounded_text("HTTP snapshot store endpoint", endpoint, 8_192)
        try:
            parsed = urlsplit(endpoint_text)
        except ValueError as error:
            raise ArgumentError("HTTP snapshot store endpoint is not a valid URL") from error
        if parsed.scheme not in {"http", "https"} or not parsed.hostname or parsed.username or parsed.password or parsed.fragment:
            raise ArgumentError("HTTP snapshot store endpoint failed URL admission")
        if require_https and parsed.scheme != "https" and not (allow_loopback and parsed.scheme == "http" and _loopback(parsed.hostname)):
            raise ArgumentError("HTTP snapshot store endpoint must use HTTPS unless loopback development is enabled")
        if not isinstance(allowed_hosts, Sequence) or isinstance(allowed_hosts, (str, bytes)) or len(allowed_hosts) > 128:
            raise ArgumentError("HTTP snapshot store allowed_hosts are outside their bound")
        normalized_hosts: list[str] = []
        for host in allowed_hosts:
            normalized = _bounded_text("HTTP snapshot store allowed host", host, 512).lower().strip("[]")
            if "://" in normalized or "/" in normalized or "@" in normalized or (normalized.startswith("*.") and len(normalized) <= 2):
                raise ArgumentError("HTTP snapshot store allowed host is malformed")
            normalized_hosts.append(normalized)
        if not _host_allowed(parsed.hostname, tuple(normalized_hosts), allow_loopback):
            raise ArgumentError("HTTP snapshot store endpoint host is outside its allowlist")
        self.endpoint = endpoint_text
        self.resource = _bounded_text("HTTP snapshot store resource", resource, MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_RESOURCE_BYTES)
        if not all(character.isalnum() or character in "_.:/+-" for character in self.resource):
            raise ArgumentError("HTTP snapshot store resource contains unsafe characters")
        if isinstance(timeout_seconds, bool) or not isinstance(timeout_seconds, (int, float)) or not 0.1 <= float(timeout_seconds) <= MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_TIMEOUT_SECONDS:
            raise ArgumentError("HTTP snapshot store timeout_seconds is outside its bound")
        for name, value, maximum in (
            ("max_request_bytes", max_request_bytes, MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_REQUEST_BYTES),
            ("max_response_bytes", max_response_bytes, MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_RESPONSE_BYTES),
        ):
            if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
                raise ArgumentError(f"HTTP snapshot store {name} is outside its bound")
        if header_resolver is not None and not callable(header_resolver):
            raise ArgumentError("HTTP snapshot store header_resolver must be callable")
        self.require_https = require_https
        self.allow_loopback = allow_loopback
        self.timeout_seconds = float(timeout_seconds)
        self.max_request_bytes = max_request_bytes
        self.max_response_bytes = max_response_bytes
        self._allowed_hosts = tuple(normalized_hosts)
        self.header_resolver = header_resolver
        self._opener = opener or build_opener(_NoRedirect()).open
        if not callable(self._opener):
            raise ArgumentError("HTTP snapshot store opener must be callable")

    def describe(self) -> dict[str, Any]:
        parsed = urlsplit(self.endpoint)
        return {
            "schema": AUTONOMOUS_HTTP_SNAPSHOT_STORE_SCHEMA,
            "resource": self.resource,
            "host": parsed.hostname,
            "scheme": parsed.scheme,
            "require_https": self.require_https,
            "allow_loopback": self.allow_loopback,
            "timeout_seconds": self.timeout_seconds,
            "max_request_bytes": self.max_request_bytes,
            "max_response_bytes": self.max_response_bytes,
            "cas": "if_match_digest_or_if_none_match_star",
            "credentials": "transient_header_resolver;never_returned",
            "retention": "metadata_only;caller_schema_validation_required",
            "secret_material": "never_returned",
        }

    def read(self) -> str | None:
        status, body = self._request("read", "GET", None, None)
        if status == 404:
            return None
        if not 200 <= status < 300:
            raise TransportError(f"HTTP snapshot store read returned status {status}")
        return _validate_snapshot_text(body, self.max_response_bytes)

    def write(self, value: str) -> None:
        encoded = _validate_snapshot_text(value, self.max_request_bytes)
        status, _body = self._request("write", "PUT", encoded, None)
        if not 200 <= status < 300:
            raise TransportError(f"HTTP snapshot store write returned status {status}")

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool:
        if expected_snapshot_digest is not None and (not isinstance(expected_snapshot_digest, str) or len(expected_snapshot_digest) != 64 or any(character not in "0123456789abcdef" for character in expected_snapshot_digest)):
            raise ArgumentError("HTTP snapshot store expected snapshot digest must be a lowercase SHA-256 digest or null")
        encoded = _validate_snapshot_text(value, self.max_request_bytes)
        status, _body = self._request("write_if_unchanged", "PUT", encoded, expected_snapshot_digest)
        if status in {409, 412}:
            return False
        if not 200 <= status < 300:
            raise TransportError(f"HTTP snapshot store write_if_unchanged returned status {status}")
        return True

    def _request(self, operation: str, method: str, body: str | None, expected_snapshot_digest: str | None) -> tuple[int, str]:
        context = {
            "operation": operation,
            "resource": self.resource,
            "expected_snapshot_digest": expected_snapshot_digest,
        }
        resolved: Mapping[str, str] = {}
        if self.header_resolver is not None:
            resolved = self.header_resolver(context)
        request_headers = _headers(resolved)
        request_headers["Accept"] = "application/json"
        request_headers["X-Aurora-Snapshot-Resource"] = self.resource
        data = None
        if body is not None:
            data = body.encode("utf-8")
            if len(data) > self.max_request_bytes:
                raise ArgumentError("HTTP snapshot store request exceeds its byte bound")
            request_headers["Content-Type"] = "application/json"
        if operation == "write_if_unchanged":
            request_headers["If-None-Match" if expected_snapshot_digest is None else "If-Match"] = "*" if expected_snapshot_digest is None else f'"{expected_snapshot_digest}"'
        request = Request(self.endpoint, data=data, headers=request_headers, method=method)
        try:
            response = self._opener(request, self.timeout_seconds)
            with response:
                raw_status = getattr(response, "status", None)
                status = int(raw_status if raw_status is not None else response.getcode())
                return status, self._read_response(response)
        except HTTPError as error:
            return int(error.code), ""
        except (TimeoutError, URLError, OSError) as error:
            raise TransportError(f"HTTP snapshot store {operation} transport failed") from error

    def _read_response(self, response: Any) -> str:
        chunks: list[bytes] = []
        total = 0
        while True:
            chunk = response.read(min(64 * 1024, self.max_response_bytes + 1 - total))
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            if total > self.max_response_bytes:
                raise TransportError("HTTP snapshot store response exceeded its byte bound")
        try:
            return b"".join(chunks).decode("utf-8")
        except UnicodeDecodeError as error:
            raise TransportError("HTTP snapshot store response was not valid UTF-8") from error


__all__ = [
    "AUTONOMOUS_HTTP_SNAPSHOT_STORE_SCHEMA",
    "MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_RESOURCE_BYTES",
    "MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_REQUEST_BYTES",
    "MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_RESPONSE_BYTES",
    "MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_TIMEOUT_SECONDS",
    "MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_HEADER_COUNT",
    "MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_HEADER_BYTES",
    "AutonomousHttpSnapshotTextStore",
]
