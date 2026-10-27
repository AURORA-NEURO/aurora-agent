"""Credentialless HTTPS for application-installed, exact public research routes.

This repository-defined transport has no available blueprint identifier. It does not discover
integrations, accept model-selected URLs, authenticate providers beyond TLS, parse JSON trees,
follow redirects, retrieve credentials, retry, or claim scientific validity. Existing source
readers and their policy digests are intentionally unchanged.
"""
from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field
import hashlib
import http.client
import ipaddress
import json
import math
import queue
import re
import socket
import ssl
import threading
import time
from types import MappingProxyType
from typing import Any
from urllib.parse import parse_qsl, unquote, urlsplit


REVIEWED_PUBLIC_HTTP_POLICY_ID = "aurora.reviewed-public-https"
REVIEWED_PUBLIC_HTTP_POLICY_VERSION = "1"
REVIEWED_PUBLIC_HTTP_CONFIG_SCHEMA = "aurora-reviewed-public-http-config/0.1"
MAX_REVIEWED_PUBLIC_HTTP_DNS_JOBS = 4
MAX_REVIEWED_PUBLIC_HTTP_RESPONSE_BYTES = 4_000_000
MAX_REVIEWED_PUBLIC_HTTP_HEADER_BYTES = 65_536
MAX_REVIEWED_PUBLIC_HTTP_FRAMING_BYTES = 65_536
_DNS_SLOTS = threading.BoundedSemaphore(MAX_REVIEWED_PUBLIC_HTTP_DNS_JOBS)
_ROUTE_ID = re.compile(r"^[A-Za-z][A-Za-z0-9_.:-]{0,127}$")
_HOST = re.compile(r"^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$")
_MEDIA = re.compile(r"^application/(?:json|[a-z0-9!#$&^_.+-]+\+json)$")
_DIGEST = re.compile(r"^[a-f0-9]{64}$")
_SECRET_KEYS = {"apikey", "key", "token", "accesstoken", "authorization", "password", "secret",
                "signature", "sig", "auth", "clientsecret", "refreshtoken", "oauthtoken", "xapikey"}


def _canonical(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, allow_nan=False, sort_keys=True, separators=(",", ":"))


def _digest(value: Any) -> str:
    return hashlib.sha256(_canonical(value).encode("utf-8")).hexdigest()


_POLICY = {"policy_id": REVIEWED_PUBLIC_HTTP_POLICY_ID, "policy_version": REVIEWED_PUBLIC_HTTP_POLICY_VERSION,
    "method": "GET", "route_authority": "application_installed_exact_https_443_urls",
    "headers": "fixed_accept_user_agent_identity_encoding_no_credentials", "proxies": "not_used",
    "dns": "all_answers_public_numeric_pinned_tls_original_hostname", "dns_jobs_max": 4,
    "deadline": "absolute_dns_connect_tls_each_send_recv_and_http_framing",
    "framing": "strict_crlf_unique_headers_exact_length_or_http11_chunks_no_extensions_or_trailers",
    "redirects": "refused", "success_status": 200, "charset": "strict_utf8",
    "json_tree_validation": "source_adapter_owned", "automatic_retry": False}
REVIEWED_PUBLIC_HTTP_POLICY_DIGEST = _digest(_POLICY)


class ReviewedPublicHttpError(ValueError):
    """Immutable, sanitized failure metadata; exception traceback machinery remains usable."""

    def __init__(self, failure_class: str, *, status_code: int | None = None,
                 retry_after_seconds: float | None = None) -> None:
        if not isinstance(failure_class, str) or not re.fullmatch(r"[a-z][a-z0-9_]{0,127}", failure_class):
            raise ValueError("public HTTP failure class is invalid")
        if status_code is not None and (type(status_code) is not int or not 100 <= status_code <= 599):
            raise ValueError("public HTTP failure status is invalid")
        if retry_after_seconds is not None and (type(retry_after_seconds) not in {int, float}
                or not math.isfinite(retry_after_seconds) or not 0 <= retry_after_seconds <= 86_400):
            raise ValueError("public HTTP retry metadata is invalid")
        object.__setattr__(self, "_metadata", (failure_class, status_code, retry_after_seconds))
        super().__init__("Reviewed public HTTPS refused: " + failure_class)

    def __setattr__(self, name: str, value: Any) -> None:
        if name in {"_metadata", "failure_class", "status_code", "retry_after_seconds"}:
            raise AttributeError("public HTTP failure metadata is immutable")
        super().__setattr__(name, value)

    @property
    def failure_class(self) -> str:
        return self._metadata[0]

    @property
    def status_code(self) -> int | None:
        return self._metadata[1]

    @property
    def retry_after_seconds(self) -> float | None:
        return self._metadata[2]

    @property
    def automatic_retry(self) -> bool:
        return False

    def to_dict(self) -> dict[str, Any]:
        return {"failure_class": self.failure_class, "status_code": self.status_code,
                "retry_after_seconds": self.retry_after_seconds, "automatic_retry": False}


def _fail(code: str) -> None:
    raise ReviewedPublicHttpError(code)


def _integer(value: Any, minimum: int, maximum: int, code: str) -> int:
    if type(value) is not int or not minimum <= value <= maximum:
        _fail(code)
    return value


def _url(value: Any, hosts: tuple[str, ...]) -> str:
    if (not isinstance(value, str) or not 1 <= len(value) <= 8192
            or any(ord(character) <= 32 or ord(character) >= 127 for character in value)
            or re.search(r"%(?![A-Fa-f0-9]{2})", value)):
        _fail("reviewed_url_invalid")
    try:
        parsed = urlsplit(value)
        if (parsed.scheme != "https" or parsed.hostname not in hosts or parsed.username is not None
                or parsed.password is not None or "#" in value or parsed.port not in {None, 443}
                or parsed.netloc not in {parsed.hostname, parsed.hostname + ":443"}
                or not parsed.path.startswith("/") or parsed.path.startswith("//")
                or len(parsed.path) > 4096 or len(parsed.query) > 4096):
            _fail("reviewed_url_outside_fixed_https_route_policy")
        decoded = unquote(value, errors="strict")
        if any(ord(character) < 32 or ord(character) == 127 for character in decoded) or "\\" in decoded:
            _fail("reviewed_url_contains_controls_or_backslash")
        if any(segment in {".", ".."} for segment in unquote(parsed.path, errors="strict").split("/")):
            _fail("reviewed_url_contains_relative_segments")
        query = parse_qsl(parsed.query, keep_blank_values=True, strict_parsing=True,
                          encoding="utf-8", errors="strict", max_num_fields=64)
        for key, _value in query:
            normalized = re.sub(r"[^a-z0-9]", "", key.casefold())
            if not key or "%" in key or normalized in _SECRET_KEYS:
                _fail("reviewed_url_contains_credential_query_key")
    except ReviewedPublicHttpError:
        raise
    except (ValueError, UnicodeError):
        raise ReviewedPublicHttpError("reviewed_url_invalid") from None
    return value


@dataclass(frozen=True, slots=True)
class ReviewedPublicHttpConfig:
    """Operator-reviewed routes; request callers select only their installed route identifier."""

    allowed_hosts: tuple[str, ...]
    reviewed_routes: Mapping[str, str]
    timeout_ms: int = 30_000
    max_response_bytes: int = 1_000_000
    max_header_bytes: int = 65_536
    max_framing_bytes: int = 65_536
    accepted_content_types: tuple[str, ...] = ("application/json",)
    _encoded: str = field(init=False, repr=False, compare=False)

    def __post_init__(self) -> None:
        hosts = self.allowed_hosts
        if (not isinstance(hosts, (tuple, list)) or not 1 <= len(hosts) <= 32
                or any(not isinstance(host, str) or len(host) > 253 or not _HOST.fullmatch(host) for host in hosts)
                or len(set(hosts)) != len(hosts)):
            _fail("allowed_hosts_must_be_bounded_exact_dns_names")
        for host in hosts:
            try:
                ipaddress.ip_address(host)
            except ValueError:
                continue
            _fail("allowed_hosts_must_be_dns_names")
        hosts = tuple(sorted(hosts))
        if (not isinstance(self.reviewed_routes, Mapping) or not 1 <= len(self.reviewed_routes) <= 128
                or any(not isinstance(key, str) or not _ROUTE_ID.fullmatch(key) for key in self.reviewed_routes)):
            _fail("reviewed_routes_must_be_bounded_installed_identifiers")
        routes = {key: _url(value, hosts) for key, value in self.reviewed_routes.items()}
        types = self.accepted_content_types
        if (not isinstance(types, (tuple, list)) or not 1 <= len(types) <= 8
                or any(not isinstance(media, str) or len(media) > 128 or not _MEDIA.fullmatch(media) for media in types)
                or len(set(types)) != len(types)):
            _fail("accepted_content_types_must_be_exact_json_media_types")
        _integer(self.timeout_ms, 100, 30_000, "timeout_ms_outside_100_to_30000")
        _integer(self.max_response_bytes, 1, MAX_REVIEWED_PUBLIC_HTTP_RESPONSE_BYTES, "response_byte_bound_invalid")
        _integer(self.max_header_bytes, 128, MAX_REVIEWED_PUBLIC_HTTP_HEADER_BYTES, "header_byte_bound_invalid")
        _integer(self.max_framing_bytes, 16, MAX_REVIEWED_PUBLIC_HTTP_FRAMING_BYTES, "framing_byte_bound_invalid")
        object.__setattr__(self, "allowed_hosts", hosts)
        object.__setattr__(self, "reviewed_routes", MappingProxyType(routes))
        object.__setattr__(self, "accepted_content_types", tuple(sorted(types)))
        metadata = {"schema": REVIEWED_PUBLIC_HTTP_CONFIG_SCHEMA, "allowed_hosts": list(hosts),
            "reviewed_routes": routes, "timeout_ms": self.timeout_ms, "max_response_bytes": self.max_response_bytes,
            "max_header_bytes": self.max_header_bytes, "max_framing_bytes": self.max_framing_bytes,
            "accepted_content_types": list(self.accepted_content_types),
            "transport_policy_id": REVIEWED_PUBLIC_HTTP_POLICY_ID,
            "transport_policy_version": REVIEWED_PUBLIC_HTTP_POLICY_VERSION,
            "transport_policy_digest": REVIEWED_PUBLIC_HTTP_POLICY_DIGEST}
        encoded = _canonical(metadata)
        if len(encoded.encode("utf-8")) > 262_144:
            _fail("reviewed_configuration_byte_bound")
        object.__setattr__(self, "_encoded", encoded)

    @property
    def config_digest(self) -> str:
        return hashlib.sha256(self._encoded.encode("utf-8")).hexdigest()

    @property
    def transport_policy_digest(self) -> str:
        return REVIEWED_PUBLIC_HTTP_POLICY_DIGEST

    def to_metadata(self) -> dict[str, Any]:
        return json.loads(self._encoded) | {"config_digest": self.config_digest}


@dataclass(frozen=True, slots=True)
class ReviewedPublicHttpResponse:
    """Successful bounded UTF-8 bytes, with metadata separated from the source body."""

    status: int
    body: bytes = field(repr=False)
    request_url: str
    response_bytes: int
    body_sha256: str
    transport_policy_digest: str
    config_digest: str
    content_type: str

    def __post_init__(self) -> None:
        if (type(self.status) is not int or self.status != 200 or type(self.body) is not bytes
                or type(self.response_bytes) is not int or self.response_bytes != len(self.body)
                or self.response_bytes > MAX_REVIEWED_PUBLIC_HTTP_RESPONSE_BYTES
                or self.body_sha256 != hashlib.sha256(self.body).hexdigest()
                or not isinstance(self.request_url, str) or len(self.request_url) > 8192
                or not isinstance(self.content_type, str) or not _MEDIA.fullmatch(self.content_type)
                or any(not isinstance(value, str) or not _DIGEST.fullmatch(value)
                       for value in (self.transport_policy_digest, self.config_digest))):
            _fail("successful_response_contract_invalid")
        try:
            self.body.decode("utf-8", "strict")
        except UnicodeError:
            raise ReviewedPublicHttpError("response_is_not_utf8") from None

    def to_metadata(self) -> dict[str, Any]:
        return {"status": self.status, "request_url": self.request_url, "response_bytes": self.response_bytes,
                "body_sha256": self.body_sha256, "transport_policy_digest": self.transport_policy_digest,
                "config_digest": self.config_digest, "content_type": self.content_type,
                "charset": "utf-8", "automatic_retry": False, "live_verification": "unassessed"}


def _remaining(deadline: float) -> float:
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        _fail("total_deadline_exceeded")
    return remaining


def _resolve_public_address(host: str, deadline: float) -> tuple[Any, ...]:
    slots = _DNS_SLOTS
    if not slots.acquire(timeout=_remaining(deadline)):
        _fail("dns_resolver_capacity_or_deadline")
    result: queue.Queue[Any] = queue.Queue(maxsize=1)
    def resolve() -> None:
        try:
            try:
                addresses = socket.getaddrinfo(host, 443, type=socket.SOCK_STREAM)
            except Exception:
                addresses = None
            result.put_nowait(addresses)
        finally:
            slots.release()
    try:
        _remaining(deadline)
        threading.Thread(target=resolve, name="aurora-reviewed-public-dns", daemon=True).start()
    except Exception:
        slots.release()
        raise ReviewedPublicHttpError("dns_resolver_unavailable") from None
    try:
        addresses = result.get(timeout=_remaining(deadline))
    except queue.Empty:
        raise ReviewedPublicHttpError("total_deadline_exceeded") from None
    _remaining(deadline)
    if not isinstance(addresses, list) or not 1 <= len(addresses) <= 64:
        _fail("dns_resolution_invalid")
    try:
        if any(address[0] not in {socket.AF_INET, socket.AF_INET6} or address[1] != socket.SOCK_STREAM
               or address[4][1] != 443 or not ipaddress.ip_address(address[4][0]).is_global
               or ipaddress.ip_address(address[4][0]).is_multicast for address in addresses):
            _fail("dns_address_is_not_public")
    except ReviewedPublicHttpError:
        raise
    except (TypeError, ValueError, IndexError):
        raise ReviewedPublicHttpError("dns_address_shape_invalid") from None
    return addresses[0]


class _DeadlineReader:
    def __init__(self, connection: Any, deadline: float, config: ReviewedPublicHttpConfig) -> None:
        self.connection, self.deadline, self.config = connection, deadline, config
        self.buffer = bytearray()
        self.closed = False
        self.reading_headers = True
        self.header_bytes = self.framing_bytes = self.wire_bytes = 0

    def _receive(self) -> bool:
        self.connection.settimeout(_remaining(self.deadline))
        block = self.connection.recv(4096)
        _remaining(self.deadline)
        self.wire_bytes += len(block)
        if self.wire_bytes > self.config.max_response_bytes + self.config.max_header_bytes + self.config.max_framing_bytes:
            _fail("response_wire_bound")
        self.buffer.extend(block)
        return bool(block)

    def readline(self, limit: int = -1) -> bytes:
        _remaining(self.deadline)
        limit = min(limit if limit >= 0 else 65_537, 65_537)
        while True:
            newline = self.buffer.find(b"\n", 0, limit)
            if newline >= 0 or len(self.buffer) >= limit:
                end = newline + 1 if newline >= 0 else limit
                value = bytes(self.buffer[:end]); del self.buffer[:end]
                break
            if not self._receive():
                value = bytes(self.buffer); self.buffer.clear()
                break
        if self.reading_headers:
            self.header_bytes += len(value)
            if self.header_bytes > self.config.max_header_bytes:
                _fail("response_header_bound")
            if value and (not value.endswith(b"\r\n") or value[:1] in (b" ", b"\t")):
                _fail("response_header_framing_invalid")
        else:
            self.account_framing(len(value))
        return value

    def account_framing(self, size: int) -> None:
        self.framing_bytes += size
        if self.framing_bytes > self.config.max_framing_bytes:
            _fail("response_framing_bound")

    def read(self, size: int = -1) -> bytes:
        _remaining(self.deadline)
        if type(size) is not int or not 0 <= size <= self.config.max_response_bytes + 1:
            _fail("transport_read_bound")
        while len(self.buffer) < size and self._receive():
            pass
        value = bytes(self.buffer[:size]); del self.buffer[:size]
        return value

    def close(self) -> None:
        self.closed = True

    def flush(self) -> None:
        return None


class _ResponseSocket:
    def __init__(self, connection: Any, deadline: float, config: ReviewedPublicHttpConfig) -> None:
        self.reader = _DeadlineReader(connection, deadline, config)

    def makefile(self, *_args: Any, **_kwargs: Any) -> _DeadlineReader:
        return self.reader


class _BoundedHttpResponse(http.client.HTTPResponse):
    def _read_status(self) -> tuple[str, int, str]:
        line = self.fp.readline(65_537)
        matched = re.fullmatch(rb"(HTTP/1\.[01]) ([1-5][0-9]{2})(?: ([\t\x20-\x7e\x80-\xff]*))?\r\n", line)
        if matched is None:
            _fail("response_status_line_invalid")
        return matched[1].decode("ascii"), int(matched[2]), (matched[3] or b"").decode("iso-8859-1")

    def _read_next_chunk_size(self) -> int:
        line = self.fp.readline(65_537)
        if not re.fullmatch(rb"[0-9A-Fa-f]{1,8}\r\n", line):
            _fail("response_chunk_size_framing_invalid")
        size = int(line[:-2], 16)
        if size > self.fp.config.max_response_bytes:
            _fail("response_chunk_bound")
        return size

    def _get_chunk_left(self) -> int | None:
        chunk_left = self.chunk_left
        if not chunk_left:
            if chunk_left is not None:
                self.fp.account_framing(2)
                if self._safe_read(2) != b"\r\n":
                    _fail("response_chunk_delimiter_invalid")
            chunk_left = self._read_next_chunk_size()
            if chunk_left == 0:
                if self.fp.readline(65_537) != b"\r\n":
                    _fail("response_trailers_refused")
                self._close_conn(); chunk_left = None
            self.chunk_left = chunk_left
        return chunk_left


def _retry_after(value: str | None) -> float | None:
    if value is None or not re.fullmatch(r"[0-9]{1,5}(?:\.[0-9]{1,6})?", value):
        return None
    number = float(value)
    return number if 0 <= number <= 86_400 else None


def _headers(response: _BoundedHttpResponse) -> dict[str, str]:
    if response.headers.defects:
        _fail("response_headers_malformed")
    result = {}
    for name, value in response.getheaders():
        normalized = name.casefold()
        if (normalized in result or not re.fullmatch(r"[!#$%&'*+.^_`|~0-9A-Za-z-]+", name)
                or any(ord(character) < 32 and character != "\t" or ord(character) == 127 for character in value)):
            _fail("response_headers_duplicate_or_malformed")
        result[normalized] = value.strip()
    return result


def _content_type(value: str | None, accepted: tuple[str, ...]) -> str:
    if value is None:
        _fail("response_content_type_missing")
    pieces = value.split(";")
    media = pieces[0].strip().casefold()
    if media not in accepted or len(pieces) > 2:
        _fail("response_content_type_unsupported")
    if len(pieces) == 2:
        if not re.fullmatch(r'\s*charset\s*=\s*(?:utf-?8|"utf-?8")\s*', pieces[1], re.IGNORECASE):
            _fail("response_charset_unsupported")
    return media


@dataclass(frozen=True, slots=True)
class ReviewedPublicHttpClient:
    """No URL, payload, header, credentials, proxy, redirect or retry selector exists at dispatch."""

    config: ReviewedPublicHttpConfig

    def __post_init__(self) -> None:
        if type(self.config) is not ReviewedPublicHttpConfig:
            _fail("client_requires_reviewed_config")

    @property
    def transport_policy_digest(self) -> str:
        return REVIEWED_PUBLIC_HTTP_POLICY_DIGEST

    def get(self, route_id: str, *, approve_source_dispatch: bool = False) -> ReviewedPublicHttpResponse:
        if approve_source_dispatch is not True:
            _fail("explicit_source_dispatch_approval_required")
        if not isinstance(route_id, str) or not _ROUTE_ID.fullmatch(route_id) or route_id not in self.config.reviewed_routes:
            _fail("route_is_not_installed")
        url = self.config.reviewed_routes[route_id]
        parsed = urlsplit(url)
        deadline = time.monotonic() + self.config.timeout_ms / 1000
        raw = tls = response = None
        completed = False
        try:
            address = _resolve_public_address(parsed.hostname, deadline)
            raw = socket.socket(address[0], address[1], address[2])
            raw.settimeout(_remaining(deadline)); raw.connect(address[4])
            context = ssl.create_default_context()
            raw.settimeout(_remaining(deadline))
            tls = context.wrap_socket(raw, server_hostname=parsed.hostname)
            target = parsed.path + (("?" + parsed.query) if "?" in url else "")
            request = (f"GET {target} HTTP/1.1\r\nHost: {parsed.netloc}\r\n"
                f"Accept: {', '.join(self.config.accepted_content_types)}\r\nAccept-Encoding: identity\r\n"
                "User-Agent: aurora-reviewed-public-http/1\r\nConnection: close\r\n\r\n").encode("ascii")
            sent = 0
            while sent < len(request):
                tls.settimeout(_remaining(deadline))
                count = tls.send(memoryview(request)[sent:])
                _remaining(deadline)
                if type(count) is not int or not 1 <= count <= len(request) - sent:
                    _fail("request_send_failed")
                sent += count
            response_socket = _ResponseSocket(tls, deadline, self.config)
            response = _BoundedHttpResponse(response_socket)
            response.begin(); response_socket.reader.reading_headers = False
            headers = _headers(response)
            if "content-length" in headers and "transfer-encoding" in headers:
                _fail("response_framing_ambiguous")
            if "transfer-encoding" in headers:
                if headers["transfer-encoding"].casefold() != "chunked" or response.version != 11:
                    _fail("response_transfer_encoding_unsupported")
                response.chunked, response.chunk_left, response.length = True, None, None
            if "location" in headers or "trailer" in headers:
                _fail("response_redirect_or_trailer_refused")
            if headers.get("content-encoding", "identity").casefold() != "identity":
                _fail("response_content_encoding_unsupported")
            length = headers.get("content-length")
            if length is not None and (not re.fullmatch(r"[0-9]{1,10}", length)
                                       or int(length) > self.config.max_response_bytes):
                _fail("response_declared_byte_bound")
            if response.status != 200:
                code = "source_rate_limited" if response.status == 429 else "redirect_refused" if 300 <= response.status < 400 else "source_http_failure"
                raise ReviewedPublicHttpError(code, status_code=response.status,
                    retry_after_seconds=_retry_after(headers.get("retry-after")) if response.status == 429 else None)
            media = _content_type(headers.get("content-type"), self.config.accepted_content_types)
            body = response.read(self.config.max_response_bytes + 1)
            _remaining(deadline)
            if len(body) > self.config.max_response_bytes:
                _fail("response_byte_bound")
            if length is not None and len(body) != int(length):
                _fail("response_declared_length_mismatch")
            result = ReviewedPublicHttpResponse(200, bytes(body), url, len(body), hashlib.sha256(body).hexdigest(),
                REVIEWED_PUBLIC_HTTP_POLICY_DIGEST, self.config.config_digest, media)
            _remaining(deadline)
            completed = True
            return result
        except ReviewedPublicHttpError:
            raise
        except (OSError, http.client.HTTPException, ValueError, TypeError):
            raise ReviewedPublicHttpError("transport_failure") from None
        finally:
            close_failed = False
            for resource in (response, tls if tls is not None else raw):
                if resource is not None:
                    try:
                        resource.close()
                    except Exception:
                        close_failed = True
            # A close failure cannot become an apparently successful response or expose foreign
            # exception text. An already-failed request retains its original sanitized failure.
            if close_failed and completed:
                raise ReviewedPublicHttpError("transport_close_failure") from None


__all__ = ["REVIEWED_PUBLIC_HTTP_POLICY_ID", "REVIEWED_PUBLIC_HTTP_POLICY_VERSION",
    "REVIEWED_PUBLIC_HTTP_POLICY_DIGEST", "REVIEWED_PUBLIC_HTTP_CONFIG_SCHEMA",
    "ReviewedPublicHttpConfig", "ReviewedPublicHttpClient", "ReviewedPublicHttpResponse", "ReviewedPublicHttpError"]
