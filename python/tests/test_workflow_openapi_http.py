"""Importer-owned HTTPS framing and credential admission over fixture TLS byte streams."""
from contextlib import contextmanager
import socket
import threading
import time
import traceback
from unittest.mock import patch

import pytest

from prism_sdk import workflow_openapi_http as http
from prism_sdk.reviewed_public_http import ReviewedPublicHttpError
from prism_sdk.workflow_openapi import OpenApiTransportError, _http_get


ADDRESS = (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("93.184.216.34", 443))
URL = "https://api.example.org/v1/records/item?limit=3"


class Socket:
    def __init__(self, fragments=(), *, order=None, clock=None, delay=0, send_limit=4096):
        self.fragments = list(fragments)
        self.order, self.clock, self.delay, self.send_limit = order, clock, delay, send_limit
        self.closed, self.sent, self.timeouts = False, bytearray(), []
    def settimeout(self, value): self.timeouts.append(value)
    def connect(self, address):
        self.address = address
        if self.order is not None: self.order.append("connect")
    def close(self): self.closed = True
    def _wait(self):
        if self.clock is not None:
            self.clock[0] += self.delay
    def send(self, value):
        self._wait()
        if self.order is not None: self.order.append("send")
        count = min(len(value), self.send_limit)
        self.sent.extend(value[:count])
        return count
    def recv(self, maximum):
        self._wait()
        if not self.fragments: return b""
        value = self.fragments.pop(0)
        if len(value) > maximum: self.fragments.insert(0, value[maximum:])
        return value[:maximum]


def wire(body=b'{"records":[],"count":0}', headers=b"", status=200):
    return b"HTTP/1.1 " + str(status).encode() + b" Fixture\r\nContent-Type: application/json\r\n" + headers + b"\r\n" + body


@contextmanager
def peer(payload, *, addresses=None, clock=None, delay=0, order=None):
    raw = Socket(order=order)
    tls = Socket([payload], order=order, clock=clock, delay=delay, send_limit=11)
    with patch.object(http.socket, "getaddrinfo", return_value=[ADDRESS] if addresses is None else addresses) as dns, \
         patch.object(http.socket, "socket", return_value=raw) as create, \
         patch.object(http.ssl, "create_default_context") as context:
        def wrap(*args, **kwargs):
            if order is not None: order.append("tls")
            return tls
        context.return_value.wrap_socket.side_effect = wrap
        if clock is None:
            yield raw, tls, dns, create, context
        else:
            with patch.object(http.time, "monotonic", side_effect=lambda: clock[0]):
                yield raw, tls, dns, create, context


def get(**options):
    return _http_get(URL, **({"headers": {"Accept": "application/json"}, "timeout_seconds": 1.0,
        "max_response_bytes": 1024, "allow_private_hosts": False} | options))


@pytest.mark.parametrize("framing", ["length", "chunked", "eof"])
def test_real_parser_delivers_exact_utf8_body_pinned_tls_without_proxy_or_retries(framing):
    body = '{"records":["α"],"count":1}'.encode()
    if framing == "length": payload = wire(body, b"Content-Length: " + str(len(body)).encode() + b"\r\n")
    elif framing == "chunked": payload = wire(format(len(body), "x").encode() + b"\r\n" + body + b"\r\n0\r\n\r\n", b"Transfer-Encoding: chunked\r\n")
    else: payload = wire(body)
    with patch.dict("os.environ", {"HTTPS_PROXY": "http://127.0.0.1:1"}), peer(payload) as (raw, tls, dns, create, context):
        result = get()
    assert result.body == body and result.final_url == URL
    dns.assert_called_once_with("api.example.org", 443, type=socket.SOCK_STREAM)
    create.assert_called_once_with(*ADDRESS[:3])
    assert raw.address == ADDRESS[4] and tls.closed
    context.return_value.wrap_socket.assert_called_once_with(raw, server_hostname="api.example.org")
    assert bytes(tls.sent) == (b"GET /v1/records/item?limit=3 HTTP/1.1\r\nHost: api.example.org\r\n"
        b"Accept-Encoding: identity\r\nConnection: close\r\nAccept: application/json\r\n\r\n")


def test_credential_resolution_occurs_after_tls_and_secret_never_enters_response_or_error_metadata():
    order = []
    secret = "fixture-private-do-not-record"
    def resolve():
        order.append("resolve")
        return {"Authorization": "Bearer " + secret}
    with peer(wire(), order=order) as (_raw, tls, *_):
        result = get(resolve_auth_headers=resolve)
    assert order[:3] == ["connect", "tls", "resolve"] and order[3] == "send"
    assert secret.encode() in tls.sent
    assert secret not in repr(result)


@pytest.mark.parametrize("phase", ["dns", "connect", "tls"])
def test_failed_destination_or_tls_admission_never_resolves_credentials_or_sends(phase):
    private = (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("127.0.0.1", 443))
    with peer(wire(), addresses=[private] if phase == "dns" else None) as (raw, tls, _dns, _create, context):
        if phase == "connect": raw.connect = lambda *_: (_ for _ in ()).throw(OSError("fixture secret"))
        if phase == "tls": context.return_value.wrap_socket.side_effect = OSError("fixture secret")
        with patch("builtins.print") as output, patch.object(http, "_checked_headers", wraps=http._checked_headers):
            resolved = []
            with pytest.raises(OpenApiTransportError) as error:
                get(resolve_auth_headers=lambda: resolved.append(True) or {"Authorization": "secret"})
        assert not resolved and not tls.sent
        assert "fixture secret" not in "".join(traceback.format_exception(error.value))
        output.assert_not_called()


@pytest.mark.parametrize("payload,code", [
    (b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n", "response_header_framing_invalid"),
    (wire(headers=b"Content-Length: 2\r\nContent-Length: 2\r\n"), "response_headers_duplicate_or_malformed"),
    (wire(headers=b"Content-Length: 2\r\nTransfer-Encoding: chunked\r\n"), "response_framing_ambiguous"),
    (wire(headers=b"Content-Encoding: gzip\r\n"), "response_content_encoding_unsupported"),
    (wire(headers=b"Location: https://other.example/\r\n"), "response_redirect_or_trailer_refused"),
    (wire(headers=b"Content-Length: 1025\r\n"), "response_declared_byte_bound"),
    (wire(body=b"{}", headers=b"Content-Length: 3\r\n"), "response_declared_length_mismatch"),
    (wire(body=b"2;secret=one\r\n{}\r\n0\r\n\r\n", headers=b"Transfer-Encoding: chunked\r\n"), "response_chunk_size_framing_invalid"),
    (wire(body=b"2\r\n{}\r\n0\r\nX-Secret: no\r\n\r\n", headers=b"Transfer-Encoding: chunked\r\n"), "response_trailers_refused"),
])
def test_malformed_or_unbounded_response_cannot_become_success(payload, code):
    with peer(payload) as (_raw, tls, *_):
        with pytest.raises(OpenApiTransportError) as error: get()
    assert error.value.failure_class == code and tls.closed


@pytest.mark.parametrize("retry,expected", [(b"0", 0.0), (b"2.5", 2.5), (b"invalid-secret", None), (None, None)])
def test_rate_hold_is_typed_no_automatic_retry_and_unknown_retry_is_not_zero(retry, expected):
    headers = b"" if retry is None else b"Retry-After: " + retry + b"\r\n"
    with peer(wire(b"private-error-body", headers, 429)) as (_raw, tls, dns, *_):
        with pytest.raises(OpenApiTransportError) as error: get()
    assert error.value.to_dict() == {"failure_class": "source_rate_limited", "status_code": 429,
        "retry_after_seconds": expected, "automatic_retry": False}
    assert "private-error-body" not in "".join(traceback.format_exception(error.value))
    assert dns.call_count == 1 and tls.closed
    with pytest.raises(AttributeError): error.value.status_code = 200


@pytest.mark.parametrize("phase", ["auth", "send", "receive"])
def test_one_total_deadline_includes_header_admission_every_send_and_receive(phase):
    clock = [0.0]
    def resolve():
        if phase == "auth": clock[0] += 2
        return {"Authorization": "Bearer fixture"}
    with peer(wire(), clock=clock, delay=.1 if phase == "send" else 0) as (_raw, tls, *_):
        if phase == "receive":
            tls.fragments = [bytes([ch]) for ch in wire()]
            original = tls.recv
            def recv(size):
                clock[0] += .1
                return original(size)
            tls.recv = recv
        with pytest.raises(OpenApiTransportError) as error: get(resolve_auth_headers=resolve)
    assert error.value.failure_class == "total_deadline_exceeded" and tls.closed
    if phase == "auth": assert not tls.sent


@pytest.mark.parametrize("headers", [
    {"Proxy-Authorization": "secret"}, {"Transfer-Encoding": "chunked"}, {"Accept-Encoding": "gzip"},
    {"X-Key": "bad\r\nHost: secret"}, {"X-Key": "🧬"}, {"Host": "other.example"},
    {"Authorization": "one", "authorization": "two"},
])
def test_forbidden_request_headers_fail_before_dns_or_socket(headers):
    with peer(wire()) as (_raw, tls, dns, create, _context):
        with pytest.raises(OpenApiTransportError): get(headers=headers)
    dns.assert_not_called(); create.assert_not_called()
    assert not tls.sent


def test_hung_dns_jobs_are_bounded_and_timeout_cannot_release_a_still_live_resolver_slot():
    release = threading.Event()
    entered = []
    semaphore = threading.BoundedSemaphore(2)
    def resolve(*_args, **_kwargs):
        entered.append(True)
        release.wait(3)
        return [ADDRESS]
    try:
        with patch.object(http, "_DNS_SLOTS", semaphore), patch.object(http.socket, "getaddrinfo", side_effect=resolve), \
             patch.object(http.socket, "socket") as create:
            for _ in range(3):
                with pytest.raises(ReviewedPublicHttpError):
                    http._address("api.example.org", 443, False, time.monotonic() + .05)
            assert len(entered) == 2
            create.assert_not_called()
    finally:
        release.set()
    assert semaphore.acquire(timeout=2) and semaphore.acquire(timeout=2)


def test_private_network_policy_remains_explicit_and_does_not_disable_hostname_verified_tls():
    private = (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("127.0.0.1", 443))
    with peer(wire(), addresses=[private]) as (raw, _tls, _dns, _create, context):
        assert get(allow_private_hosts=True).status == 200
    assert raw.address == private[4]
    context.return_value.wrap_socket.assert_called_once_with(raw, server_hostname="api.example.org")
