"""Bounded HTTPS for importer-selected GET routes and application-owned headers.

Repository-defined extension; no detailed source blueprint id is available. This uses the
reviewed public transport's strict HTTP parser without changing that credentialless client's
policy. It does not authenticate accounts, retry, follow redirects, persist secrets, or parse
source JSON. Private network access remains a separate explicit application policy.
"""
from __future__ import annotations

from dataclasses import dataclass
import http.client
import ipaddress
import math
import queue
import re
import socket
import ssl
import threading
import time
from typing import Any, Callable, Mapping
from urllib.parse import urlsplit

from .authoring import content_digest
from .reviewed_public_http import (
    REVIEWED_PUBLIC_HTTP_POLICY_DIGEST, ReviewedPublicHttpError, _BoundedHttpResponse, _ResponseSocket, _content_type,
    _headers, _remaining, _retry_after,
)


OPENAPI_HTTPS_POLICY_VERSION = "1"
_POLICY = {"id": "aurora.reviewed-openapi-https", "version": OPENAPI_HTTPS_POLICY_VERSION,
    "method": "GET", "destination": "importer_selected_fixed_server_and_declared_arguments",
    "dns": "bounded_4_jobs_all_answers_checked_pinned_address_verified_tls_hostname",
    "deadline": "absolute_dns_connect_tls_auth_resolution_send_receive_and_framing",
    "auth": "application_owned_headers_only_after_verified_tls_no_proxy_credentials",
    "framing": "shared_reviewed_public_http_v2_strict_crlf_unique_headers_length_chunked_or_eof",
    "shared_parser_policy_digest": REVIEWED_PUBLIC_HTTP_POLICY_DIGEST,
    "max_header_bytes": 65536, "max_framing_bytes": 65536, "max_request_bytes": 65536,
    "encoding": "identity_strict_utf8_json", "redirects": False, "automatic_retry": False}
OPENAPI_HTTPS_POLICY_DIGEST = content_digest(_POLICY)
_DNS_SLOTS = threading.BoundedSemaphore(4)
_RESERVED_HEADERS = frozenset({"host", "connection", "content-length", "transfer-encoding",
    "accept-encoding", "proxy-authorization", "proxy-connection", "trailer", "te", "upgrade"})


@dataclass(frozen=True, slots=True)
class _Limits:
    max_response_bytes: int
    max_header_bytes: int = 65536
    max_framing_bytes: int = 65536


def _fail(code: str) -> None:
    raise ReviewedPublicHttpError(code)


def _address(host: str, port: int, private: bool, deadline: float) -> tuple[Any, ...]:
    # A timed-out platform resolver may remain blocked. Its slot is released only when it
    # really exits; repeated caller timeouts therefore cannot accumulate unlimited threads.
    slots = _DNS_SLOTS
    if not slots.acquire(timeout=_remaining(deadline)):
        _fail("dns_resolver_capacity_or_deadline")
    results: queue.Queue[Any] = queue.Queue(maxsize=1)
    def resolve() -> None:
        try:
            try:
                value = socket.getaddrinfo(host, port, type=socket.SOCK_STREAM)
            except Exception:
                value = None
            results.put_nowait(value)
        finally:
            slots.release()
    try:
        _remaining(deadline)
        threading.Thread(target=resolve, name="aurora-openapi-dns", daemon=True).start()
    except Exception:
        slots.release()
        raise ReviewedPublicHttpError("dns_resolver_unavailable") from None
    try:
        addresses = results.get(timeout=_remaining(deadline))
    except queue.Empty:
        raise ReviewedPublicHttpError("total_deadline_exceeded") from None
    _remaining(deadline)
    if not isinstance(addresses, list) or not 1 <= len(addresses) <= 64:
        _fail("dns_resolution_invalid")
    try:
        for item in addresses:
            address = ipaddress.ip_address(item[4][0])
            if (item[0] not in {socket.AF_INET, socket.AF_INET6} or item[1] != socket.SOCK_STREAM
                    or item[4][1] != port or address.is_multicast or address.is_unspecified
                    or not private and not address.is_global):
                _fail("dns_address_is_not_public_or_allowed")
    except ReviewedPublicHttpError:
        raise
    except (TypeError, ValueError, IndexError):
        raise ReviewedPublicHttpError("dns_address_shape_invalid") from None
    return addresses[0]


def _checked_headers(value: Any, *, allow_accept: bool) -> dict[str, str]:
    if not isinstance(value, Mapping) or len(value) > 32:
        _fail("application_headers_invalid")
    checked: dict[str, str] = {}
    seen: set[str] = set()
    for name, text in value.items():
        if (type(name) is not str or not re.fullmatch(r"[A-Za-z0-9-]{1,128}", name)
                or name.lower() in seen or name.lower() in _RESERVED_HEADERS
                or name.lower() == "accept" and (not allow_accept or text != "application/json")
                or type(text) is not str or not 1 <= len(text) <= 8192
                or any(ord(ch) < 32 or ord(ch) > 255 or ord(ch) == 127 for ch in text)):
            _fail("application_headers_invalid")
        checked[name] = text
        seen.add(name.lower())
    if sum(len(name) + len(text) + 4 for name, text in checked.items()) > 60000:
        _fail("application_headers_byte_bound")
    return checked


def bounded_openapi_get(url: str, *, headers: Mapping[str, str], timeout_seconds: float,
        max_response_bytes: int, allow_private_hosts: bool,
        resolve_auth_headers: Callable[[], Mapping[str, str]] | None = None
        ) -> tuple[int, dict[str, str], bytes]:
    """Internal importer transport; callers supply no runtime-selected method or body.

    Foreign credential callbacks are synchronous trusted application code: their elapsed time
    is checked before any send, but this cannot forcibly interrupt a blocked Python callback.
    """
    if (type(timeout_seconds) not in {int, float} or not math.isfinite(timeout_seconds)
            or not .1 <= timeout_seconds <= 120 or type(max_response_bytes) is not int
            or not 1 <= max_response_bytes <= 4_000_000 or type(allow_private_hosts) is not bool):
        _fail("transport_configuration_invalid")
    try:
        if (type(url) is not str or not 1 <= len(url) <= 8192
                or any(ord(ch) <= 32 or ord(ch) >= 127 for ch in url) or "\\" in url):
            _fail("transport_route_invalid")
        parts = urlsplit(url)
        port = parts.port or 443
        if (parts.scheme != "https" or not parts.hostname or parts.username is not None
                or parts.password is not None or parts.fragment or "#" in url
                or port != 443 and not allow_private_hosts):
            _fail("transport_route_invalid")
    except (TypeError, ValueError):
        raise ReviewedPublicHttpError("transport_route_invalid") from None
    fixed_headers = _checked_headers(headers, allow_accept=True)
    deadline = time.monotonic() + timeout_seconds
    raw = tls = response = None
    completed = False
    try:
        address = _address(parts.hostname, port, allow_private_hosts, deadline)
        raw = socket.socket(address[0], address[1], address[2])
        raw.settimeout(_remaining(deadline)); raw.connect(address[4])
        context = ssl.create_default_context()
        raw.settimeout(_remaining(deadline))
        tls = context.wrap_socket(raw, server_hostname=parts.hostname)
        _remaining(deadline)
        if resolve_auth_headers is not None:
            try:
                auth_headers = _checked_headers(resolve_auth_headers(), allow_accept=False)
            except Exception:
                raise ReviewedPublicHttpError("credential_admission_failed") from None
            if {key.casefold() for key in auth_headers} & {key.casefold() for key in fixed_headers}:
                _fail("application_headers_conflict")
            fixed_headers.update(auth_headers)
        _remaining(deadline)
        target = parts.path or "/"
        target += ("?" + parts.query) if "?" in url else ""
        prefix = (f"GET {target} HTTP/1.1\r\nHost: {parts.netloc}\r\n"
            f"Accept-Encoding: identity\r\nConnection: close\r\n")
        request = (prefix + "".join(f"{name}: {text}\r\n" for name, text in fixed_headers.items())
            + "\r\n").encode("latin-1", "strict")
        if len(request) > 65536:
            _fail("request_byte_bound")
        sent = 0
        while sent < len(request):
            tls.settimeout(_remaining(deadline))
            count = tls.send(memoryview(request)[sent:])
            _remaining(deadline)
            if type(count) is not int or not 1 <= count <= len(request) - sent:
                _fail("request_send_failed")
            sent += count
        response_socket = _ResponseSocket(tls, deadline, _Limits(max_response_bytes))
        response = _BoundedHttpResponse(response_socket)
        response.begin(); response_socket.reader.reading_headers = False
        received = _headers(response)
        if "content-length" in received and "transfer-encoding" in received:
            _fail("response_framing_ambiguous")
        if "transfer-encoding" in received:
            if received["transfer-encoding"].casefold() != "chunked" or response.version != 11:
                _fail("response_transfer_encoding_unsupported")
            response.chunked, response.chunk_left, response.length = True, None, None
        if "location" in received or "trailer" in received:
            _fail("response_redirect_or_trailer_refused")
        if received.get("content-encoding", "identity").casefold() != "identity":
            _fail("response_content_encoding_unsupported")
        length = received.get("content-length")
        if length is not None and (not re.fullmatch(r"[0-9]{1,10}", length)
                or int(length) > max_response_bytes):
            _fail("response_declared_byte_bound")
        if not 200 <= response.status <= 299:
            code = "source_rate_limited" if response.status == 429 else "redirect_refused" if 300 <= response.status < 400 else "source_http_failure"
            raise ReviewedPublicHttpError(code, status_code=response.status,
                retry_after_seconds=_retry_after(received.get("retry-after")) if response.status == 429 else None)
        _content_type(received.get("content-type"), ("application/json",))
        body = response.read(max_response_bytes + 1)
        _remaining(deadline)
        if len(body) > max_response_bytes:
            _fail("response_byte_bound")
        if length is not None and len(body) != int(length):
            _fail("response_declared_length_mismatch")
        body.decode("utf-8", "strict")
        completed = True
        return response.status, received, bytes(body)
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
        if completed and close_failed:
            raise ReviewedPublicHttpError("transport_close_failure") from None


__all__ = ["OPENAPI_HTTPS_POLICY_DIGEST", "OPENAPI_HTTPS_POLICY_VERSION"]
