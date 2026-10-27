"""Exact application-installed anonymous GraphQL POST bodies over reviewed HTTPS.

Repository-defined extension; no available blueprint module specifies this contract.
This is not a GraphQL importer or semantic validator. The installing source reader owns
review of its exact query. Dispatch accepts an installed identifier, never caller-selected
query text, variables, endpoint, headers or credentials. Existing GET contracts stay intact.
"""
from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field
import hashlib
import http.client
import json
import re
from types import MappingProxyType
from typing import Any
from urllib.parse import urlsplit

from . import reviewed_public_http as wire

REVIEWED_PUBLIC_GRAPHQL_POLICY_ID = "aurora.reviewed-public-fixed-graphql-post"
REVIEWED_PUBLIC_GRAPHQL_POLICY_VERSION = "1"
REVIEWED_PUBLIC_GRAPHQL_CONFIG_SCHEMA = "aurora-reviewed-public-graphql-config/0.1"
MAX_REVIEWED_GRAPHQL_REQUEST_BYTES = 65_536
_POLICY = {
    "policy_id": REVIEWED_PUBLIC_GRAPHQL_POLICY_ID, "policy_version": REVIEWED_PUBLIC_GRAPHQL_POLICY_VERSION,
    "method": "POST", "route_authority": "application_installed_exact_https_443_urls",
    "body_authority": "application_installed_exact_utf8_json_bytes",
    "graphql_semantics": "installing_reader_review;not_a_general_graphql_validator",
    "headers": "fixed_json_content_type_exact_length_accept_user_agent_identity_encoding_no_credentials",
    "wire_policy_digest": wire.REVIEWED_PUBLIC_HTTP_POLICY_DIGEST,
    "redirects": "refused", "automatic_retry": False, "success_status": 200,
}
REVIEWED_PUBLIC_GRAPHQL_POLICY_DIGEST = wire._digest(_POLICY)
ReviewedPublicGraphqlError = wire.ReviewedPublicHttpError
ReviewedPublicGraphqlResponse = wire.ReviewedPublicHttpResponse


def _body(value: Any) -> bytes:
    if type(value) is not bytes or not 1 <= len(value) <= MAX_REVIEWED_GRAPHQL_REQUEST_BYTES:
        wire._fail("installed_graphql_body_bound")
    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, item in items:
            if key in result:
                wire._fail("installed_graphql_duplicate_field")
            result[key] = item
        return result
    try:
        document = json.loads(value.decode("utf-8", "strict"), object_pairs_hook=pairs,
                              parse_constant=lambda _: wire._fail("installed_graphql_nonfinite"))
    except (ValueError, UnicodeError, RecursionError):
        wire._fail("installed_graphql_body_invalid")
    if (type(document) is not dict or set(document) != {"query", "operationName", "variables"}
            or type(document["variables"]) is not dict or document["variables"]
            or type(document["operationName"]) is not str
            or re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]{0,127}", document["operationName"]) is None
            or type(document["query"]) is not str or "\x00" in document["query"]
            or not document["query"].startswith("query " + document["operationName"] + " {")):
        wire._fail("installed_graphql_envelope_invalid")
    # This limited envelope check does not establish read-only semantics. The source adapter
    # binds the complete reviewed body, so changing it never inherits that adapter's contract.
    return value


@dataclass(frozen=True, slots=True)
class ReviewedPublicGraphqlConfig:
    """Trusted application configuration sealed before dispatch; no dynamic query argument."""

    http_config: wire.ReviewedPublicHttpConfig
    reviewed_bodies: Mapping[str, bytes]
    _encoded: str = field(init=False, repr=False, compare=False)

    def __post_init__(self) -> None:
        if type(self.http_config) is not wire.ReviewedPublicHttpConfig:
            wire._fail("graphql_requires_reviewed_http_config")
        base = self.http_config._validated_copy()
        if (not isinstance(self.reviewed_bodies, Mapping)
                or set(self.reviewed_bodies) != set(base.reviewed_routes)):
            wire._fail("graphql_body_routes_must_match")
        bodies = {key: _body(value) for key, value in self.reviewed_bodies.items()}
        metadata = base.to_metadata()
        # The base configuration validates reusable wire limits. Its GET identity must never
        # be presented as the POST policy or as this configuration's digest.
        metadata.pop("config_digest")
        metadata.update({
            "schema": REVIEWED_PUBLIC_GRAPHQL_CONFIG_SCHEMA,
            "transport_policy_id": REVIEWED_PUBLIC_GRAPHQL_POLICY_ID,
            "transport_policy_version": REVIEWED_PUBLIC_GRAPHQL_POLICY_VERSION,
            "transport_policy_digest": REVIEWED_PUBLIC_GRAPHQL_POLICY_DIGEST,
            "method": "POST",
            "reviewed_bodies": {key: {"body_utf8": value.decode("utf-8"),
                "request_bytes": len(value), "body_sha256": hashlib.sha256(value).hexdigest()}
                for key, value in bodies.items()},
        })
        encoded = wire._canonical(metadata)
        if len(encoded.encode("utf-8")) > 262_144:
            wire._fail("graphql_configuration_byte_bound")
        object.__setattr__(self, "http_config", base)
        object.__setattr__(self, "reviewed_bodies", MappingProxyType(bodies))
        object.__setattr__(self, "_encoded", encoded)

    @property
    def config_digest(self) -> str:
        return hashlib.sha256(self._encoded.encode("utf-8")).hexdigest()

    @property
    def transport_policy_digest(self) -> str:
        return REVIEWED_PUBLIC_GRAPHQL_POLICY_DIGEST

    def to_metadata(self) -> dict[str, Any]:
        return json.loads(self._encoded) | {"config_digest": self.config_digest}

    def _validated_copy(self, expected_encoded: str | None = None) -> ReviewedPublicGraphqlConfig:
        try:
            if type(self._encoded) is not str or (expected_encoded is not None and self._encoded != expected_encoded):
                wire._fail("reviewed_configuration_changed")
            current = ReviewedPublicGraphqlConfig(self.http_config, self.reviewed_bodies)
            if current._encoded != self._encoded:
                wire._fail("reviewed_configuration_changed")
            return current
        except Exception:
            raise ReviewedPublicGraphqlError("reviewed_configuration_changed") from None


@dataclass(frozen=True, slots=True)
class ReviewedPublicGraphqlClient:
    config: ReviewedPublicGraphqlConfig
    _encoded_config: str = field(init=False, repr=False, compare=False)

    def __post_init__(self) -> None:
        if type(self.config) is not ReviewedPublicGraphqlConfig:
            wire._fail("client_requires_reviewed_graphql_config")
        object.__setattr__(self, "_encoded_config", self.config._validated_copy()._encoded)

    @property
    def transport_policy_digest(self) -> str:
        return REVIEWED_PUBLIC_GRAPHQL_POLICY_DIGEST

    def post(self, route_id: str, *, approve_source_dispatch: bool = False) -> ReviewedPublicGraphqlResponse:
        if approve_source_dispatch is not True:
            wire._fail("explicit_source_dispatch_approval_required")
        if type(self.config) is not ReviewedPublicGraphqlConfig:
            wire._fail("reviewed_configuration_changed")
        config = self.config._validated_copy(self._encoded_config)
        base = config.http_config
        if type(route_id) is not str or route_id not in base.reviewed_routes:
            wire._fail("route_is_not_installed")
        url, body = base.reviewed_routes[route_id], config.reviewed_bodies[route_id]
        parsed = urlsplit(url)
        deadline = wire.time.monotonic() + base.timeout_ms / 1000
        raw = tls = response = None
        completed = False
        try:
            address = wire._resolve_public_address(parsed.hostname, deadline)
            raw = wire.socket.socket(address[0], address[1], address[2])
            raw.settimeout(wire._remaining(deadline)); raw.connect(address[4])
            context = wire.ssl.create_default_context()
            raw.settimeout(wire._remaining(deadline))
            tls = context.wrap_socket(raw, server_hostname=parsed.hostname)
            target = parsed.path + (("?" + parsed.query) if "?" in url else "")
            request = (f"POST {target} HTTP/1.1\r\nHost: {parsed.netloc}\r\n"
                f"Accept: {', '.join(base.accepted_content_types)}\r\nAccept-Encoding: identity\r\n"
                f"Content-Type: application/json\r\nContent-Length: {len(body)}\r\n"
                f"User-Agent: aurora-reviewed-public-graphql/{REVIEWED_PUBLIC_GRAPHQL_POLICY_VERSION}\r\n"
                "Connection: close\r\n\r\n").encode("ascii") + body
            sent = 0
            while sent < len(request):
                tls.settimeout(wire._remaining(deadline))
                count = tls.send(memoryview(request)[sent:])
                wire._remaining(deadline)
                if type(count) is not int or not 1 <= count <= len(request) - sent:
                    wire._fail("request_send_failed")
                sent += count
            response_socket = wire._ResponseSocket(tls, deadline, base)
            response = wire._BoundedHttpResponse(response_socket)
            response.begin(); response_socket.reader.reading_headers = False
            headers = wire._headers(response)
            if "content-length" in headers and "transfer-encoding" in headers:
                wire._fail("response_framing_ambiguous")
            if "transfer-encoding" in headers:
                if headers["transfer-encoding"].casefold() != "chunked" or response.version != 11:
                    wire._fail("response_transfer_encoding_unsupported")
                response.chunked, response.chunk_left, response.length = True, None, None
            if "location" in headers or "trailer" in headers:
                raise ReviewedPublicGraphqlError("response_redirect_or_trailer_refused", status_code=response.status)
            if headers.get("content-encoding", "identity").casefold() != "identity":
                wire._fail("response_content_encoding_unsupported")
            length = headers.get("content-length")
            if length is not None and (not re.fullmatch(r"[0-9]{1,10}", length)
                    or int(length) > base.max_response_bytes):
                wire._fail("response_declared_byte_bound")
            if response.status != 200:
                code = ("source_rate_limited" if response.status == 429 else
                        "redirect_refused" if 300 <= response.status < 400 else "source_http_failure")
                raise ReviewedPublicGraphqlError(code, status_code=response.status,
                    retry_after_seconds=wire._retry_after(headers.get("retry-after")) if response.status == 429 else None)
            media = wire._content_type(headers.get("content-type"), base.accepted_content_types)
            received = response.read(base.max_response_bytes + 1)
            wire._remaining(deadline)
            if len(received) > base.max_response_bytes:
                wire._fail("response_byte_bound")
            if length is not None and len(received) != int(length):
                wire._fail("response_declared_length_mismatch")
            result = ReviewedPublicGraphqlResponse(200, bytes(received), url, len(received),
                hashlib.sha256(received).hexdigest(), REVIEWED_PUBLIC_GRAPHQL_POLICY_DIGEST,
                config.config_digest, media)
            wire._remaining(deadline)
            completed = True
            return result
        except ReviewedPublicGraphqlError:
            raise
        except (OSError, http.client.HTTPException, ValueError, TypeError):
            raise ReviewedPublicGraphqlError("transport_failure") from None
        finally:
            close_failed = False
            for resource in (response, tls if tls is not None else raw):
                if resource is not None:
                    try:
                        resource.close()
                    except Exception:
                        close_failed = True
            if close_failed and completed:
                raise ReviewedPublicGraphqlError("transport_close_failure") from None


__all__ = ["REVIEWED_PUBLIC_GRAPHQL_POLICY_ID", "REVIEWED_PUBLIC_GRAPHQL_POLICY_VERSION",
    "REVIEWED_PUBLIC_GRAPHQL_POLICY_DIGEST", "REVIEWED_PUBLIC_GRAPHQL_CONFIG_SCHEMA",
    "MAX_REVIEWED_GRAPHQL_REQUEST_BYTES", "ReviewedPublicGraphqlConfig", "ReviewedPublicGraphqlClient",
    "ReviewedPublicGraphqlResponse", "ReviewedPublicGraphqlError"]

