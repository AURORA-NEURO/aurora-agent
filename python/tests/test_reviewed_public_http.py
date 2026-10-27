"""Real HTTP framing over fixture TLS bytes; these are not live service observations."""
from contextlib import contextmanager
from dataclasses import FrozenInstanceError
import hashlib
import json
import socket
import threading
import time
import traceback
from unittest.mock import patch

import pytest

from prism_sdk import reviewed_public_http as public


HOST = "research.example.org"
URL = "https://research.example.org/annotations/TP53?format=json"
ADDRESS = (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("93.184.216.34", 443))


def config(**changes):
    return public.ReviewedPublicHttpConfig(**({"allowed_hosts": (HOST,), "reviewed_routes": {"tp53": URL}} | changes))


class Clock:
    def __init__(self): self.now = 0.0
    def monotonic(self): return self.now


class Socket:
    def __init__(self, fragments=(), *, clock=None, send_limit=4096, send_delay=0):
        self.fragments = list(fragments)
        self.clock, self.send_limit, self.send_delay = clock, send_limit, send_delay
        self.closed, self.sent, self.timeouts, self.recv_calls = False, bytearray(), [], 0

    def settimeout(self, value): self.timeouts.append(value)
    def connect(self, address): self.address = address
    def close(self): self.closed = True

    def _wait(self, delay):
        if self.clock is not None:
            timeout = self.timeouts[-1]
            self.clock.now += min(delay, timeout)
            if delay >= timeout: raise TimeoutError("fixture-only source detail")

    def send(self, value):
        self._wait(self.send_delay)
        count = min(len(value), self.send_limit)
        self.sent.extend(value[:count])
        return count

    def recv(self, maximum):
        self.recv_calls += 1
        if not self.fragments: return b""
        item = self.fragments.pop(0)
        value, delay = item if isinstance(item, tuple) else (item, 0)
        self._wait(delay)
        if len(value) > maximum:
            self.fragments.insert(0, value[maximum:])
        return value[:maximum]


def wire(body=b'{"gene":"TP53"}', headers=b"", *, status=200, version=b"HTTP/1.1", content_type=b"application/json"):
    return version + b" " + str(status).encode() + b" Fixture\r\nContent-Type: " + content_type + b"\r\n" + headers + b"\r\n" + body


@contextmanager
def transport(fragments, *, configuration=None, clock=None, send_limit=4096, send_delay=0, answers=None):
    raw = Socket()
    secured = Socket(fragments, clock=clock, send_limit=send_limit, send_delay=send_delay)
    client = public.ReviewedPublicHttpClient(config() if configuration is None else configuration)
    with patch.object(public.socket, "getaddrinfo", return_value=[ADDRESS] if answers is None else answers) as dns, \
         patch.object(public.socket, "socket", return_value=raw) as create_socket, \
         patch.object(public.ssl, "create_default_context") as context:
        context.return_value.wrap_socket.return_value = secured
        if clock is None:
            yield client, raw, secured, dns, context, create_socket
        else:
            with patch.object(public.time, "monotonic", side_effect=clock.monotonic):
                yield client, raw, secured, dns, context, create_socket


def get(payload, *, configuration=None):
    with transport([payload], configuration=configuration) as (client, raw, secured, *_rest):
        try:
            return client.get("tp53", approve_source_dispatch=True)
        finally:
            assert secured.closed


@pytest.mark.parametrize("framing", ["length", "chunked", "close"])
def test_actual_wire_parser_returns_exact_successful_utf8_bytes_and_policy_metadata(framing):
    body = '{"gene":"TP53","annotation":"α🧬"}'.encode()
    if framing == "length":
        payload = wire(body, b"Content-Length: " + str(len(body)).encode() + b"\r\n")
    elif framing == "chunked":
        chunks = [body[:7], body[7:]]
        payload = wire(b"".join(format(len(chunk), "x").encode() + b"\r\n" + chunk + b"\r\n" for chunk in chunks)
                       + b"0\r\n\r\n", b"Transfer-Encoding: chunked\r\n")
    else: payload = wire(body)
    with patch.dict("os.environ", {"HTTPS_PROXY": "http://127.0.0.1:1", "AURORA_API_TOKEN": "never-inherited"}), \
         transport([payload], send_limit=13) as (client, raw, secured, dns, context, _create_socket):
        result = client.get("tp53", approve_source_dispatch=True)
    assert result.body == body and result.response_bytes == len(body)
    assert result.body_sha256 == hashlib.sha256(body).hexdigest()
    assert result.request_url == URL and result.content_type == "application/json"
    assert result.config_digest == client.config.config_digest
    assert result.transport_policy_digest == client.transport_policy_digest == public.REVIEWED_PUBLIC_HTTP_POLICY_DIGEST
    assert "body" not in result.to_metadata() and result.to_metadata()["live_verification"] == "unassessed"
    assert raw.address == ADDRESS[4] and secured.closed
    dns.assert_called_once_with(HOST, 443, type=socket.SOCK_STREAM)
    context.return_value.wrap_socket.assert_called_once_with(raw, server_hostname=HOST)
    request = bytes(secured.sent)
    assert request == (f"GET /annotations/TP53?format=json HTTP/1.1\r\nHost: {HOST}\r\nAccept: application/json\r\n"
        f"Accept-Encoding: identity\r\nUser-Agent: aurora-reviewed-public-http/{public.REVIEWED_PUBLIC_HTTP_POLICY_VERSION}\r\nConnection: close\r\n\r\n").encode()
    assert b"Authorization" not in request and b"never-inherited" not in request


@pytest.mark.parametrize("approval", [False, 1, "true", None])
def test_only_literal_approval_can_resolve_or_send_an_installed_route(approval):
    with transport([wire()]) as (client, _raw, secured, dns, _context, create_socket):
        with pytest.raises(public.ReviewedPublicHttpError, match="approval"):
            client.get("tp53", approve_source_dispatch=approval)
        dns.assert_not_called(); create_socket.assert_not_called()
        assert not secured.sent


@pytest.mark.parametrize("route", ["unknown", URL, "../tp53", "", 1])
def test_dispatch_accepts_only_a_preinstalled_route_identifier(route):
    with transport([wire()]) as (client, _raw, _secured, dns, _context, create_socket):
        with pytest.raises(public.ReviewedPublicHttpError, match="not_installed"):
            client.get(route, approve_source_dispatch=True)
        dns.assert_not_called(); create_socket.assert_not_called()


def test_configuration_snapshots_are_immutable_deterministic_and_bound_to_every_policy_control():
    hosts, routes, media = [HOST], {"tp53": URL}, ["application/json"]
    fixed = config(allowed_hosts=hosts, reviewed_routes=routes, accepted_content_types=media)
    expected = fixed.config_digest
    hosts[0] = "changed.example.org"; routes["tp53"] = URL + "&changed=true"; media[0] = "text/plain"
    assert fixed.allowed_hosts == (HOST,) and fixed.reviewed_routes["tp53"] == URL
    assert fixed.config_digest == config().config_digest == expected
    metadata = fixed.to_metadata(); metadata["reviewed_routes"]["tp53"] = "changed"
    assert fixed.to_metadata()["reviewed_routes"]["tp53"] == URL
    with pytest.raises(TypeError): fixed.reviewed_routes["other"] = URL
    with pytest.raises(FrozenInstanceError): fixed.timeout_ms = 1
    with pytest.raises(FrozenInstanceError): public.ReviewedPublicHttpClient(fixed).config = config()
    for change in ({"timeout_ms": 100}, {"max_response_bytes": 2}, {"max_header_bytes": 128},
                   {"max_framing_bytes": 16}, {"accepted_content_types": ("application/problem+json",)},
                   {"reviewed_routes": {"tp53": URL + "&fields=id"}}):
        assert config(**change).config_digest != expected


@pytest.mark.parametrize("field,value", [
    ("reviewed_routes", {"tp53": "https://unreviewed.example.org/other"}),
    ("allowed_hosts", ("unreviewed.example.org",)), ("timeout_ms", 30001),
    ("max_response_bytes", 4000001), ("max_header_bytes", 127),
    ("max_framing_bytes", 15), ("accepted_content_types", ("text/plain",)),
    ("_encoded", "{}"),
])
@pytest.mark.parametrize("stage", ["before_registration", "after_registration"])
def test_reflection_changed_configuration_refuses_before_dns_or_socket(field, value, stage):
    cfg = config()
    with patch.object(public.socket, "getaddrinfo") as dns, patch.object(public.socket, "socket") as create_socket:
        if stage == "before_registration":
            object.__setattr__(cfg, field, value)
            with pytest.raises(public.ReviewedPublicHttpError, match="configuration_changed"):
                public.ReviewedPublicHttpClient(cfg)
        else:
            client = public.ReviewedPublicHttpClient(cfg)
            object.__setattr__(cfg, field, value)
            with pytest.raises(public.ReviewedPublicHttpError, match="configuration_changed"):
                client.get("tp53", approve_source_dispatch=True)
        dns.assert_not_called(); create_socket.assert_not_called()


def test_replacing_the_entire_typed_client_configuration_cannot_change_its_registration():
    with transport([wire()]) as (client, _raw, secured, dns, _context, create_socket):
        object.__setattr__(client, "config", config(reviewed_routes={"tp53": URL + "&changed=true"}))
        with pytest.raises(public.ReviewedPublicHttpError, match="configuration_changed"):
            client.get("tp53", approve_source_dispatch=True)
        dns.assert_not_called(); create_socket.assert_not_called()
        assert not secured.sent


def test_after_admission_foreign_callback_mutation_cannot_change_wire_or_receipt_binding():
    cfg = config()
    expected = cfg.config_digest
    with transport([wire()], configuration=cfg) as (client, _raw, secured, dns, _context, _create_socket):
        def mutate(*_args, **_kwargs):
            object.__setattr__(cfg, "reviewed_routes", {"tp53": "https://unreviewed.example.org/other"})
            object.__setattr__(cfg, "accepted_content_types", ("text/plain",))
            object.__setattr__(cfg, "max_response_bytes", 1)
            return [ADDRESS]
        dns.side_effect = mutate
        result = client.get("tp53", approve_source_dispatch=True)
    assert result.request_url == URL and result.config_digest == expected
    assert bytes(secured.sent).startswith(b"GET /annotations/TP53?format=json HTTP/1.1\r\nHost: research.example.org\r\n")
    assert b"Accept: application/json\r\n" in bytes(secured.sent)
    with patch.object(public.socket, "getaddrinfo") as dns:
        with pytest.raises(public.ReviewedPublicHttpError, match="configuration_changed"):
            client.get("tp53", approve_source_dispatch=True)
        dns.assert_not_called()


def test_policy_digest_explicitly_binds_close_delimited_framing():
    assert "bounded_eof" in public._POLICY["framing"]
    assert public.REVIEWED_PUBLIC_HTTP_POLICY_VERSION == "2"
    assert public.REVIEWED_PUBLIC_HTTP_POLICY_DIGEST == public._digest(public._POLICY)


@pytest.mark.parametrize("payload", [
    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n",
    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 0\r\n",
    b"HTTP/1.1 100 Continue\r\nInterim: value\r\n",
    b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nContent-Type: application/json\r\n",
])
def test_eof_cannot_replace_the_actual_blank_header_terminator(payload):
    with pytest.raises(public.ReviewedPublicHttpError, match="response_header_framing_invalid"):
        get(payload)


def test_a_complete_interim_header_block_followed_by_complete_final_headers_is_valid():
    assert get(b"HTTP/1.1 100 Continue\r\nInterim: value\r\n\r\n" + wire()).body == b'{"gene":"TP53"}'


@pytest.mark.parametrize("url", ["http://research.example.org/data", "https://evil.example.org/data",
    "https://research.example.org:444/data", "https://user:password@research.example.org/data",
    "https://research.example.org/data#part", "https://research.example.org/data#", "https://research.example.org",
    "https://research.example.org//data", "https://research.example.org/../data", "https://research.example.org/%2e%2e/data",
    "https://research.example.org/a\\b", "https://research.example.org/%0d%0a", "https://research.example.org/%ZZ",
    "https://research.example.org/data?api_key=value", "https://research.example.org/data?access%5Ftoken=value",
    "https://research.example.org/data?%2574oken=value", "https://research.example.org/data?format",
    "https://research.example.org/data?" + "x=1&" * 65, "https://research.example.org/" + "x" * 4097])
def test_unreviewable_routes_credentials_relative_paths_and_query_bounds_are_refused_without_dns(url):
    with patch.object(public.socket, "getaddrinfo") as dns, pytest.raises(public.ReviewedPublicHttpError):
        config(reviewed_routes={"tp53": url})
    dns.assert_not_called()


@pytest.mark.parametrize("change", [{"allowed_hosts": ()}, {"allowed_hosts": (HOST, HOST)},
    {"allowed_hosts": ("127.0.0.1",)}, {"allowed_hosts": ("Research.example.org",)}, {"allowed_hosts": ("*.example.org",)},
    {"reviewed_routes": {}}, {"reviewed_routes": {"../route": URL}}, {"timeout_ms": True}, {"timeout_ms": 0},
    {"timeout_ms": 30001}, {"max_response_bytes": 0}, {"max_response_bytes": 4000001},
    {"max_header_bytes": 127}, {"max_framing_bytes": 15}, {"accepted_content_types": ("*/*",)},
    {"accepted_content_types": ("text/plain",)}, {"accepted_content_types": ("application/json; charset=utf-8",)},
    {"accepted_content_types": ("application/json", "application/json")}])
def test_configuration_requires_bounded_exact_public_json_contract(change):
    with pytest.raises(public.ReviewedPublicHttpError): config(**change)


@pytest.mark.parametrize("content_type", [b"application/json", b"application/json; charset=UTF8",
    b'application/json; charset="UTF-8"', b"Application/JSON ; charset=utf-8"])
def test_json_media_types_accept_only_the_reviewed_media_and_utf8_charset(content_type):
    assert get(wire(content_type=content_type)).content_type == "application/json"


@pytest.mark.parametrize("content_type", [b"text/html", b"application/problem+json", b"application/json; charset=latin-1",
    b"application/json; charset=utf-8; charset=utf-8", b"application/json; profile=arbitrary", b"application/json;"])
def test_unreviewed_media_parameters_and_charsets_are_refusals(content_type):
    with pytest.raises(public.ReviewedPublicHttpError, match="content_type|charset"):
        get(wire(content_type=content_type))


def test_explicit_json_suffix_media_is_configurable_and_body_tree_validation_is_adapter_owned():
    cfg = config(accepted_content_types=("application/problem+json",))
    nested = b"[" * 1000 + b"0" + b"]" * 1000
    assert get(wire(nested, content_type=b"application/problem+json"), configuration=cfg).body == nested
    assert get(wire(b"not JSON; source parser must refuse it")).body.startswith(b"not JSON")
    with pytest.raises(public.ReviewedPublicHttpError, match="not_utf8"):
        get(wire(b"\xff"))


@pytest.mark.parametrize("headers", [b"Content-Length: 2\r\ncontent-length: 2\r\n",
    b"Transfer-Encoding: chunked\r\nTransfer-Encoding: chunked\r\n",
    b"Content-Length: 2\r\nTransfer-Encoding: chunked\r\n", b"Content-Length: -1\r\n",
    b"Content-Length: 2,2\r\n", b"Content-Length: 1000001\r\n", b"Transfer-Encoding: gzip\r\n",
    b"Transfer-Encoding: gzip, chunked\r\n", b"Content-Encoding: gzip\r\n", b"Content-Encoding:\r\n",
    b"Location: https://unreviewed.example/\r\n", b"Trailer: Digest\r\n", b"X-Header: value\r\n folded\r\n",
    b"Bad Header: value\r\n"])
def test_malformed_ambiguous_or_unreviewed_http_headers_never_produce_a_success(headers):
    with pytest.raises(public.ReviewedPublicHttpError): get(wire(b"{}", headers))


@pytest.mark.parametrize("chunked", [b"2\r\n{}XX0\r\n\r\n", b"2;ext=value\r\n{}\r\n0\r\n\r\n",
    b"2\n{}\r\n0\r\n\r\n", b"0\r\nDigest: value\r\n\r\n", b"0\r\n", b"f4241\r\n", b"3\r\n{}"])
def test_invalid_chunk_sizes_delimiters_extensions_trailers_and_truncation_fail(chunked):
    with pytest.raises(public.ReviewedPublicHttpError): get(wire(chunked, b"Transfer-Encoding: chunked\r\n"))


def test_declared_length_truncation_invalid_header_lines_and_http10_chunking_are_refused():
    for payload in (wire(b"{}", b"Content-Length: 3\r\n"), wire().replace(b"\r\n", b"\n"),
                    wire(b"2\r\n{}\r\n0\r\n\r\n", b"Transfer-Encoding: chunked\r\n", version=b"HTTP/1.0")):
        with pytest.raises(public.ReviewedPublicHttpError): get(payload)


@pytest.mark.parametrize("status_line", [b"HTTP/1.2 200 OK\r\n", b"HTTP/2 200 OK\r\n",
    b"HTTP/1.1 0200 OK\r\n", b"HTTP/1.1 600 Unsupported\r\n", b"HTTP/1.1 200 bad\x00reason\r\n"])
def test_status_line_version_code_and_control_grammar_are_strict(status_line):
    with pytest.raises(public.ReviewedPublicHttpError, match="status_line_invalid"):
        get(status_line + b"Content-Type: application/json\r\n\r\n{}")


def test_body_headers_and_chunk_framing_have_independent_bounds():
    with pytest.raises(public.ReviewedPublicHttpError):
        get(wire(b"x" * 1001), configuration=config(max_response_bytes=1000))
    with pytest.raises(public.ReviewedPublicHttpError, match="header_bound"):
        get(wire(headers=b"X-Large: " + b"x" * 500 + b"\r\n"), configuration=config(max_header_bytes=128))
    with pytest.raises(public.ReviewedPublicHttpError, match="framing_bound"):
        get(wire(b"1\r\nx\r\n" * 10 + b"0\r\n\r\n", b"Transfer-Encoding: chunked\r\n"),
            configuration=config(max_framing_bytes=16))


@pytest.mark.parametrize("status", [201, 204, 301, 302, 304, 400, 403, 404, 408, 500, 503])
def test_non200_statuses_are_typed_failures_without_empty_values_or_retry(status):
    with patch.object(public.time, "sleep") as sleep:
        with pytest.raises(public.ReviewedPublicHttpError) as error:
            get(wire(b"never-return-this-payload", status=status))
    assert error.value.status_code == status and error.value.retry_after_seconds is None
    assert "never-return" not in str(error.value) + json.dumps(error.value.to_dict())
    sleep.assert_not_called()


@pytest.mark.parametrize("header,expected", [("0", 0.0), ("0.25", 0.25), ("86400", 86400.0),
    ("86401", None), ("-1", None), ("1e2", None), ("NaN", None), ("Wed, 21 Oct 2026 07:28:00 GMT", None), ("9" * 100, None)])
def test_429_preserves_bounded_numeric_retry_observation_but_unknown_remains_none(header, expected):
    with pytest.raises(public.ReviewedPublicHttpError) as error:
        get(wire(b"untrusted source payload", b"Retry-After: " + header.encode() + b"\r\n", status=429))
    assert error.value.failure_class == "source_rate_limited"
    assert error.value.status_code == 429 and error.value.retry_after_seconds == expected
    assert error.value.automatic_retry is False
    assert "untrusted" not in json.dumps(error.value.to_dict())


@pytest.mark.parametrize("addresses", [[], [ADDRESS, (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("127.0.0.1", 443))],
    [(socket.AF_INET, socket.SOCK_STREAM, 6, "", ("10.0.0.1", 443))],
    [(socket.AF_INET, socket.SOCK_STREAM, 6, "", ("224.0.0.1", 443))],
    [(socket.AF_INET, socket.SOCK_STREAM, 6, "", ("93.184.216.34", 80))], [ADDRESS] * 65])
def test_every_dns_answer_must_be_public_with_a_bounded_shape_before_any_socket(addresses):
    with transport([wire()], answers=addresses) as (client, _raw, _secured, _dns, _context, create_socket):
        with pytest.raises(public.ReviewedPublicHttpError): client.get("tp53", approve_source_dispatch=True)
        create_socket.assert_not_called()


def test_four_timed_out_dns_jobs_remain_bounded_across_distinct_client_configurations():
    slots, release = threading.BoundedSemaphore(4), threading.Event()
    threads, entered = [], []
    thread_type = threading.Thread
    def lookup(*_args, **_kwargs):
        entered.append(True); release.wait(2); return [ADDRESS]
    def worker(*args, **kwargs):
        thread = thread_type(*args, **kwargs); threads.append(thread); return thread
    with patch.object(public, "_DNS_SLOTS", slots), patch.object(public.socket, "getaddrinfo", side_effect=lookup), \
         patch.object(public.threading, "Thread", side_effect=worker), patch.object(public.socket, "socket") as create_socket:
        try:
            for index in range(5):
                client = public.ReviewedPublicHttpClient(config(timeout_ms=100, reviewed_routes={"tp53": URL + "&fixture=" + str(index)}))
                with pytest.raises(public.ReviewedPublicHttpError): client.get("tp53", approve_source_dispatch=True)
            assert len(threads) == len(entered) == 4 and not slots.acquire(blocking=False)
            create_socket.assert_not_called()
        finally:
            release.set()
            for thread in threads: thread.join(timeout=2)
        assert all(not thread.is_alive() for thread in threads)
        for _ in range(4): assert slots.acquire(blocking=False)
        for _ in range(4): slots.release()


def test_dns_worker_start_failure_releases_its_reserved_slot_without_a_request():
    slots = threading.BoundedSemaphore(1)
    with patch.object(public, "_DNS_SLOTS", slots), patch.object(public.threading.Thread, "start", side_effect=RuntimeError("hidden")):
        with pytest.raises(public.ReviewedPublicHttpError, match="unavailable"):
            public._resolve_public_address(HOST, time.monotonic() + 1)
    assert slots.acquire(blocking=False); slots.release()


@pytest.mark.parametrize("stage", ["headers", "body", "framing", "send"])
def test_slow_trickles_cannot_extend_the_absolute_request_deadline(stage):
    clock = Clock()
    if stage == "headers": fragments = [(bytes([byte]), 0.3) for byte in wire()]
    elif stage == "body": fragments = [(wire(b"", b"Content-Length: 10\r\n"), 0)] + [(b"x", 0.3)] * 10
    elif stage == "framing": fragments = [(wire(b"", b"Transfer-Encoding: chunked\r\n"), 0)] + [(b"1\r\nx\r\n", 0.3)] * 10
    else: fragments = [wire()]
    with transport(fragments, configuration=config(timeout_ms=1000), clock=clock,
                   send_limit=1 if stage == "send" else 4096, send_delay=0.3 if stage == "send" else 0) as (client, _raw, secured, *_rest):
        with pytest.raises(public.ReviewedPublicHttpError): client.get("tp53", approve_source_dispatch=True)
    assert clock.now <= 1.0 and secured.closed
    assert secured.timeouts[-1] <= secured.timeouts[0]


def test_tls_failures_are_sanitized_and_close_the_raw_socket_without_a_response():
    with transport([wire()]) as (client, raw, _secured, _dns, context, _create_socket):
        context.return_value.wrap_socket.side_effect = OSError("source-or-key-text-must-not-leak")
        with pytest.raises(public.ReviewedPublicHttpError) as error:
            client.get("tp53", approve_source_dispatch=True)
        assert raw.closed and "must-not-leak" not in "".join(traceback.format_exception(error.value))


def test_close_failure_is_sanitized_and_cannot_turn_into_success_or_mask_a_source_refusal():
    for status in (200, 429):
        with transport([wire(status=status)]) as (client, _raw, secured, *_rest):
            secured.close = lambda: (_ for _ in ()).throw(OSError("never-expose-close-detail"))
            with pytest.raises(public.ReviewedPublicHttpError) as error:
                client.get("tp53", approve_source_dispatch=True)
        assert "never-expose" not in "".join(traceback.format_exception(error.value))
        assert error.value.failure_class == ("transport_close_failure" if status == 200 else "source_rate_limited")


def test_a_fixed_empty_query_delimiter_is_not_normalized_out_of_the_reviewed_request_target():
    cfg = config(reviewed_routes={"tp53": "https://research.example.org/annotations/TP53?"})
    with transport([wire()], configuration=cfg) as (client, _raw, secured, *_rest):
        result = client.get("tp53", approve_source_dispatch=True)
    assert result.request_url.endswith("?")
    assert bytes(secured.sent).startswith(b"GET /annotations/TP53? HTTP/1.1\r\n")


def test_success_and_failure_metadata_are_immutable_while_traceback_handling_remains_valid():
    result = get(wire())
    with pytest.raises(FrozenInstanceError): result.body = b"changed"
    with pytest.raises(public.ReviewedPublicHttpError):
        public.ReviewedPublicHttpResponse(200, bytearray(b"{}"), URL, 2, hashlib.sha256(b"{}").hexdigest(),
            public.REVIEWED_PUBLIC_HTTP_POLICY_DIGEST, config().config_digest, "application/json")
    error = public.ReviewedPublicHttpError("source_rate_limited", status_code=429, retry_after_seconds=0)
    with pytest.raises(AttributeError): error.failure_class = "changed"
    with pytest.raises(AttributeError): error._metadata = ("changed", None, None)
    @contextmanager
    def boundary(): yield
    with pytest.raises(public.ReviewedPublicHttpError) as raised:
        with boundary(): raise error
    assert raised.value is error and error.to_dict()["retry_after_seconds"] == 0
