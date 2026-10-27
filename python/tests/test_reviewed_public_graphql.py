"""POST wire fixtures exercise the actual shared parser; none is a live provider observation."""
from contextlib import contextmanager
from dataclasses import FrozenInstanceError
import hashlib
import json
from unittest.mock import patch

import pytest

from prism_sdk import reviewed_public_http as wire
from prism_sdk import reviewed_public_graphql as graphql
from test_reviewed_public_http import ADDRESS, Clock, Socket, wire as response_wire

HOST, URL = "research.example.org", "https://research.example.org/api/graphql"
BODY = b'{"operationName":"ReviewedMetadataV1","query":"query ReviewedMetadataV1 { metadata { id } }","variables":{}}'


def configuration(body=BODY, **changes):
    base = wire.ReviewedPublicHttpConfig(allowed_hosts=(HOST,), reviewed_routes={"metadata": URL}, **changes)
    return graphql.ReviewedPublicGraphqlConfig(base, {"metadata": body})


@contextmanager
def transport(fragments, *, config=None, clock=None, answers=None, send_limit=4096, send_delay=0):
    raw = Socket()
    secured = Socket(fragments, clock=clock, send_limit=send_limit, send_delay=send_delay)
    client = graphql.ReviewedPublicGraphqlClient(configuration() if config is None else config)
    with patch.object(wire.socket, "getaddrinfo", return_value=[ADDRESS] if answers is None else answers) as dns, \
         patch.object(wire.socket, "socket", return_value=raw) as create_socket, \
         patch.object(wire.ssl, "create_default_context") as context:
        context.return_value.wrap_socket.return_value = secured
        if clock is None:
            yield client, raw, secured, dns, context, create_socket
        else:
            with patch.object(wire.time, "monotonic", side_effect=clock.monotonic):
                yield client, raw, secured, dns, context, create_socket


@pytest.mark.parametrize("framing", ["length", "chunked", "close"])
def test_installed_post_sends_exact_body_and_length_without_credentials_or_payload_selector(framing):
    body = '{"data":{"metadata":{"id":"α"}}}'.encode()
    if framing == "length":
        payload = response_wire(body, b"Content-Length: " + str(len(body)).encode() + b"\r\n")
    elif framing == "chunked":
        payload = response_wire(format(len(body), "x").encode() + b"\r\n" + body + b"\r\n0\r\n\r\n",
                                b"Transfer-Encoding: chunked\r\n")
    else:
        payload = response_wire(body)
    with patch.dict("os.environ", {"HTTPS_PROXY": "http://127.0.0.1:1", "CIVIC_API_KEY": "never-inherited"}), \
         transport([payload], send_limit=7) as (client, raw, secured, dns, context, _socket):
        result = client.post("metadata", approve_source_dispatch=True)
    expected = (f"POST /api/graphql HTTP/1.1\r\nHost: {HOST}\r\nAccept: application/json\r\n"
        f"Accept-Encoding: identity\r\nContent-Type: application/json\r\nContent-Length: {len(BODY)}\r\n"
        f"User-Agent: aurora-reviewed-public-graphql/{graphql.REVIEWED_PUBLIC_GRAPHQL_POLICY_VERSION}\r\n"
        "Connection: close\r\n\r\n").encode() + BODY
    assert bytes(secured.sent) == expected
    assert b"Authorization" not in expected and b"never-inherited" not in expected
    assert result.body == body and result.body_sha256 == hashlib.sha256(body).hexdigest()
    assert result.transport_policy_digest == graphql.REVIEWED_PUBLIC_GRAPHQL_POLICY_DIGEST
    assert result.config_digest == client.config.config_digest
    assert raw.address == ADDRESS[4] and secured.closed
    dns.assert_called_once_with(HOST, 443, type=wire.socket.SOCK_STREAM)
    context.return_value.wrap_socket.assert_called_once_with(raw, server_hostname=HOST)
    assert "body" not in result.to_metadata()


def test_get_policy_and_post_policy_are_separate_and_request_bytes_are_digest_bound():
    config = configuration()
    before = config.to_metadata()
    assert before["method"] == "POST"
    assert before["transport_policy_digest"] != wire.REVIEWED_PUBLIC_HTTP_POLICY_DIGEST
    assert before["config_digest"] != config.http_config.config_digest
    changed = configuration(BODY.replace(b"metadata {", b"other {"))
    assert changed.config_digest != config.config_digest
    assert configuration(timeout_ms=100).config_digest != config.config_digest
    metadata = config.to_metadata()
    metadata["reviewed_bodies"]["metadata"]["body_utf8"] = "changed"
    assert config.to_metadata() == before
    with pytest.raises(TypeError):
        config.reviewed_bodies["metadata"] = b"changed"
    with pytest.raises(FrozenInstanceError):
        config.http_config = config.http_config


@pytest.mark.parametrize("approval", [False, 1, "true", None])
def test_post_literal_approval_is_required_before_dns_or_socket(approval):
    with transport([]) as (client, _raw, secured, dns, _context, create_socket):
        with pytest.raises(graphql.ReviewedPublicGraphqlError, match="approval"):
            client.post("metadata", approve_source_dispatch=approval)
        dns.assert_not_called(); create_socket.assert_not_called()
        assert not secured.sent


@pytest.mark.parametrize("route", ["unknown", URL, "../metadata", "", 1])
def test_post_call_cannot_select_an_uninstalled_route(route):
    with transport([]) as (client, _raw, _secured, dns, _context, create_socket):
        with pytest.raises(graphql.ReviewedPublicGraphqlError, match="not_installed"):
            client.post(route, approve_source_dispatch=True)
        dns.assert_not_called(); create_socket.assert_not_called()


@pytest.mark.parametrize("body", [
    b"", b"x" * 65537, b"\xff", b"{}", b'{"query":null}', b'{"query":1,"query":2}',
    b'{"operationName":"X","query":"mutation X { x }","variables":{}}',
    b'{"operationName":"X","query":"query Y { x }","variables":{}}',
    b'{"operationName":"X","query":"query X { x }","variables":{"cursor":"next"}}',
    b'{"operationName":"X","query":"query X { x }","variables":{},"authorization":"x"}',
], ids=lambda value: "body-" + str(len(value)) + "-" + hashlib.sha256(value).hexdigest()[:8])
def test_noninstalled_envelope_forms_refuse_before_client_or_network(body):
    with pytest.raises(graphql.ReviewedPublicGraphqlError):
        configuration(body)


@pytest.mark.parametrize("field,value", [
    ("reviewed_bodies", {"metadata": BODY.replace(b"metadata {", b"other {")}),
    ("reviewed_bodies", {}), ("_encoded", "{}"), ("http_config", None),
])
def test_reflection_changed_post_configuration_refuses_before_dns(field, value):
    config = configuration()
    with transport([], config=config) as (client, _raw, _secured, dns, _context, create_socket):
        object.__setattr__(config, field, value)
        with pytest.raises(graphql.ReviewedPublicGraphqlError, match="configuration_changed"):
            client.post("metadata", approve_source_dispatch=True)
        dns.assert_not_called(); create_socket.assert_not_called()


@pytest.mark.parametrize("payload", [
    response_wire(headers=b"Content-Length: 1\r\nContent-Length: 1\r\n"),
    response_wire(headers=b"Content-Length: 10\r\nTransfer-Encoding: chunked\r\n"),
    response_wire(headers=b"Location: https://other.example.org/\r\n"),
    response_wire(headers=b"Content-Encoding: gzip\r\n"),
    response_wire(headers=b"Content-Length: 9999999\r\n"),
    response_wire(body=b"{}", headers=b"Content-Length: 10\r\n"),
    response_wire(content_type=b"text/plain"),
    response_wire(body=b"\xff"),
    response_wire(body=b"1\r\nx\r\n0\r\nTrailer: refused\r\n\r\n", headers=b"Transfer-Encoding: chunked\r\n"),
])
def test_post_reuses_bounded_strict_response_framing_without_a_second_request(payload):
    with transport([payload]) as (client, _raw, secured, dns, _context, create_socket):
        with pytest.raises(graphql.ReviewedPublicGraphqlError):
            client.post("metadata", approve_source_dispatch=True)
        assert secured.closed and dns.call_count == create_socket.call_count == 1
        assert bytes(secured.sent).count(b"POST /api/graphql") == 1


@pytest.mark.parametrize("status", [301, 302, 307, 401, 403, 429, 500])
def test_post_http_failures_retain_status_without_retry_or_redirect(status):
    with transport([response_wire(status=status, headers=b"Retry-After: 2\r\n")]) as (client, _raw, secured, dns, _context, _socket):
        with pytest.raises(graphql.ReviewedPublicGraphqlError) as failure:
            client.post("metadata", approve_source_dispatch=True)
        assert failure.value.status_code == status and failure.value.automatic_retry is False
        assert failure.value.retry_after_seconds == (2.0 if status == 429 else None)
        assert dns.call_count == 1 and secured.closed


def test_post_send_and_receive_share_one_absolute_deadline():
    clock = Clock()
    with transport([(response_wire(), .08)], config=configuration(timeout_ms=100),
                   clock=clock, send_limit=10000, send_delay=.04) as (client, _raw, secured, dns, _context, _socket):
        with pytest.raises(graphql.ReviewedPublicGraphqlError):
            client.post("metadata", approve_source_dispatch=True)
        assert clock.now <= .101 and dns.call_count == 1 and secured.closed


def test_post_refuses_any_nonpublic_dns_answer_without_connecting():
    private = (wire.socket.AF_INET, wire.socket.SOCK_STREAM, 6, "", ("127.0.0.1", 443))
    with transport([], answers=[ADDRESS, private]) as (client, _raw, _secured, _dns, _context, create_socket):
        with pytest.raises(graphql.ReviewedPublicGraphqlError, match="not_public"):
            client.post("metadata", approve_source_dispatch=True)
        create_socket.assert_not_called()


def test_real_redirect_header_retains_observed_status_without_following_location():
    with transport([response_wire(status=307, headers=b"Location: https://other.example.org/\r\n")]) as (client, _raw, secured, dns, _context, _socket):
        with pytest.raises(graphql.ReviewedPublicGraphqlError) as failure:
            client.post("metadata", approve_source_dispatch=True)
        assert failure.value.status_code == 307 and failure.value.automatic_retry is False
        assert dns.call_count == 1 and bytes(secured.sent).count(b"POST /api/graphql") == 1

