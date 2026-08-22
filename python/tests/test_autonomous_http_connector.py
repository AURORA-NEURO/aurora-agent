from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousHttpConnectorPage,
    AutonomousHttpConnectorPolicy,
    AutonomousHttpConnectorRequest,
    DomainEvidenceProviderConnectorManifest,
    create_autonomous_http_paginated_connector_executor,
    create_autonomous_http_connector_executor,
)
from prism_sdk.errors import ArgumentError


class _Response:
    def __init__(self, payload: bytes, *, status: int = 200, content_type: str = "application/json") -> None:
        self.status = status
        self.headers = {"Content-Type": content_type}
        self._payload = payload
        self._offset = 0

    def __enter__(self) -> "_Response":
        return self

    def __exit__(self, *_args: object) -> None:
        return None

    def getcode(self) -> int:
        return self.status

    def read(self, size: int = -1) -> bytes:
        if size < 0:
            size = len(self._payload) - self._offset
        chunk = self._payload[self._offset : self._offset + size]
        self._offset += len(chunk)
        return chunk


def _manifest() -> DomainEvidenceProviderConnectorManifest:
    return DomainEvidenceProviderConnectorManifest(
        connector_id="http-connector",
        version="v1",
        provider="local-loopback-test",
        connector_kind="provider_api",
        domains=tuple(AUTONOMOUS_DOMAINS),
        capabilities=("evidence_read",),
    )


def _policy(**overrides: object) -> AutonomousHttpConnectorPolicy:
    values = {
        "allowed_hosts": ("example.test",),
        "require_https": False,
        "allow_loopback": False,
    }
    values.update(overrides)
    return AutonomousHttpConnectorPolicy(**values)


def test_http_executor_is_domain_neutral_and_keeps_auth_transient() -> None:
    calls: list[tuple[str, str, str | None]] = []

    def opener(request, _timeout):
        calls.append((request.get_method(), request.full_url, request.headers.get("Authorization")))
        domain = request.full_url.rsplit("/", 1)[-1]
        return _Response(json.dumps({"domain": domain, "records": 1}).encode("utf-8"))

    executor = create_autonomous_http_connector_executor(
        lambda _manifest, request: AutonomousHttpConnectorRequest(
            method="GET",
            url=f"http://example.test/evidence/{request['domain']}",
        ),
        policy=_policy(),
        header_resolver=lambda _manifest, _request: {"Authorization": "Bearer transient-test-only"},
        opener=opener,
    )

    results = [executor(_manifest(), {"domain": domain}) for domain in AUTONOMOUS_DOMAINS]

    assert [result.status for result in results] == ["observed"] * len(AUTONOMOUS_DOMAINS)
    assert [result.value["domain"] for result in results] == list(AUTONOMOUS_DOMAINS)
    assert len(calls) == len(AUTONOMOUS_DOMAINS)
    assert all(call[0] == "GET" and call[2] == "Bearer transient-test-only" for call in calls)
    assert all("transient-test-only" not in repr(result) for result in results)


@pytest.mark.parametrize(
    ("status", "expected_status", "expected_failure"),
    [
        (401, "refused", "auth_refused"),
        (403, "refused", "auth_refused"),
        (404, "refused", "not_found"),
        (429, "error", "rate_limited"),
        (500, "error", "http_5xx"),
    ],
)
def test_http_executor_projects_http_failures_without_response_body(status, expected_status, expected_failure) -> None:
    executor = create_autonomous_http_connector_executor(
        lambda _manifest, _request: AutonomousHttpConnectorRequest(method="GET", url="http://example.test/status"),
        policy=_policy(),
        opener=lambda _request, _timeout: _Response(b"provider-secret-body", status=status),
    )

    result = executor(_manifest(), {"operation": "status"})

    assert result.status == expected_status
    assert result.failure_class == expected_failure
    assert result.value == {"status_code": status}


def test_http_executor_bounds_payloads_and_returns_digest_only_for_non_json() -> None:
    invalid = create_autonomous_http_connector_executor(
        lambda _manifest, _request: AutonomousHttpConnectorRequest(method="GET", url="http://example.test/plain"),
        policy=_policy(),
        opener=lambda _request, _timeout: _Response(b"not-json", content_type="text/plain"),
    )
    oversized = create_autonomous_http_connector_executor(
        lambda _manifest, _request: AutonomousHttpConnectorRequest(method="GET", url="http://example.test/large"),
        policy=_policy(max_response_bytes=4),
        opener=lambda _request, _timeout: _Response(b"12345"),
    )
    timed_out = create_autonomous_http_connector_executor(
        lambda _manifest, _request: AutonomousHttpConnectorRequest(method="GET", url="http://example.test/timeout"),
        policy=_policy(),
        opener=lambda _request, _timeout: (_ for _ in ()).throw(TimeoutError()),
    )

    invalid_result = invalid(_manifest(), {})
    oversized_result = oversized(_manifest(), {})
    timeout_result = timed_out(_manifest(), {})

    assert invalid_result.status == "partial"
    assert invalid_result.failure_class == "invalid_json"
    assert invalid_result.value["content_type"] == "text/plain"
    assert "body_digest" in invalid_result.value and len(invalid_result.value["body_digest"]) == 64
    assert oversized_result.status == "error"
    assert oversized_result.failure_class == "response_too_large"
    assert oversized_result.value["status_code"] == 200
    assert len(oversized_result.value["body_digest"]) == 64
    assert timeout_result.status == "error"
    assert timeout_result.failure_class == "timeout"


def test_http_connector_rejects_ambiguous_hosts_credentials_and_unsafe_headers() -> None:
    with pytest.raises(ArgumentError, match="credential-shaped"):
        AutonomousHttpConnectorRequest(method="POST", url="http://example.test", body={"api_key": "never"})
    with pytest.raises(ArgumentError, match="query"):
        create_autonomous_http_connector_executor(
            lambda _manifest, _request: AutonomousHttpConnectorRequest(method="GET", url="http://example.test?access_token=never"),
            policy=_policy(),
            opener=lambda _request, _timeout: _Response(b"{}"),
        )(_manifest(), {})
    with pytest.raises(ArgumentError, match="allowlist"):
        create_autonomous_http_connector_executor(
            lambda _manifest, _request: AutonomousHttpConnectorRequest(method="GET", url="http://other.test"),
            policy=_policy(),
            opener=lambda _request, _timeout: _Response(b"{}"),
        )(_manifest(), {})
    with pytest.raises(ArgumentError, match="header value"):
        AutonomousHttpConnectorRequest(method="GET", url="http://example.test", headers={"X-Test": "ok\r\nInjected: yes"})
    with pytest.raises(ArgumentError, match="HTTPS"):
        create_autonomous_http_connector_executor(
            lambda _manifest, _request: AutonomousHttpConnectorRequest(method="GET", url="http://example.test"),
            policy=AutonomousHttpConnectorPolicy(allowed_hosts=("example.test",)),
            opener=lambda _request, _timeout: _Response(b"{}"),
        )(_manifest(), {})


def test_http_connector_allows_loopback_only_when_explicitly_enabled() -> None:
    executor = create_autonomous_http_connector_executor(
        lambda _manifest, _request: AutonomousHttpConnectorRequest(method="GET", url="http://127.0.0.1/status"),
        policy=AutonomousHttpConnectorPolicy(require_https=False, allow_loopback=True),
        opener=lambda _request, _timeout: _Response(b"{\"ok\":true}"),
    )

    result = executor(_manifest(), {})

    assert result.status == "observed"
    assert result.value == {"ok": True}


def test_bounded_pagination_follows_opaque_cursors_for_every_autonomous_domain_without_retaining_them() -> None:
    calls: list[tuple[str, str | None]] = []

    def opener(request, _timeout):
        from urllib.parse import parse_qs, urlsplit

        query = parse_qs(urlsplit(request.full_url).query)
        domain = query["domain"][0]
        cursor = query.get("cursor", [None])[0]
        calls.append((domain, request.headers.get("Authorization")))
        return _Response(
            json.dumps({
                "items": [{"domain": domain, "page": 2 if cursor else 1}],
                "next_cursor": None if cursor else f"opaque-{domain}",
            }).encode("utf-8")
        )

    executor = create_autonomous_http_paginated_connector_executor(
        lambda _manifest, request: AutonomousHttpConnectorRequest(
            method="GET",
            url=f"http://example.test/page?domain={request['domain']}"
            + (f"&cursor={request['__autonomous_http_page_cursor']}" if request.get("__autonomous_http_page_cursor") else ""),
        ),
        policy=_policy(),
        header_resolver=lambda _manifest, _request: {"Authorization": "Bearer transient-test-only"},
        opener=opener,
    )

    results = [executor(_manifest(), {"domain": domain}) for domain in AUTONOMOUS_DOMAINS]

    assert [result.status for result in results] == ["observed"] * len(AUTONOMOUS_DOMAINS)
    assert all(result.value["complete"] is True for result in results)
    assert all(result.value["item_count"] == 2 and result.value["page_count"] == 2 for result in results)
    assert all(result.value["next_cursor_digest"] is None for result in results)
    assert len(calls) == len(AUTONOMOUS_DOMAINS) * 2
    assert all(auth == "Bearer transient-test-only" for _domain, auth in calls)
    assert all("opaque-" not in repr(result) for result in results)


def test_bounded_pagination_detects_shape_cycles_item_caps_and_page_caps() -> None:
    def make_executor(payload, **options):
        return create_autonomous_http_paginated_connector_executor(
            lambda _manifest, _request: AutonomousHttpConnectorRequest(method="GET", url="http://example.test/page"),
            policy=_policy(),
            opener=lambda _request, _timeout: _Response(json.dumps(payload).encode("utf-8")),
            **options,
        )

    shape = make_executor({"values": [1]})(_manifest(), {})
    item_limit = make_executor(
        {"items": [{"id": 1}, {"id": 2}], "next_cursor": "secret-cursor"},
        max_items=1,
    )(_manifest(), {})
    page_limit = make_executor({"items": [{"id": 1}], "next_cursor": "next"}, max_pages=1)(_manifest(), {})
    cycle_calls = 0

    def cycle_opener(_request, _timeout):
        nonlocal cycle_calls
        cycle_calls += 1
        return _Response(json.dumps({"items": [{"cycle_calls": cycle_calls}], "next_cursor": "same"}).encode("utf-8"))

    cycle = create_autonomous_http_paginated_connector_executor(
        lambda _manifest, _request: AutonomousHttpConnectorRequest(method="GET", url="http://example.test/page"),
        policy=_policy(),
        opener=cycle_opener,
    )(_manifest(), {})

    assert shape.status == "partial" and shape.failure_class == "page_shape"
    assert item_limit.failure_class == "item_limit"
    assert item_limit.value["complete"] is False
    assert len(item_limit.value["next_cursor_digest"]) == 64
    assert "secret-cursor" not in repr(item_limit)
    assert page_limit.failure_class == "page_limit"
    assert page_limit.value["page_count"] == 1
    assert cycle.failure_class == "cursor_cycle"
    assert cycle_calls == 2
    assert len(cycle.value["next_cursor_digest"]) == 64


def test_pagination_preserves_useful_items_when_a_later_page_transport_fails() -> None:
    calls = 0

    def opener(_request, _timeout):
        nonlocal calls
        calls += 1
        if calls == 1:
            return _Response(json.dumps({"items": [{"retained": True}], "next_cursor": "next"}).encode("utf-8"))
        raise OSError("offline")

    executor = create_autonomous_http_paginated_connector_executor(
        lambda _manifest, request: AutonomousHttpConnectorRequest(
            method="GET",
            url="http://example.test/page" + ("?cursor=next" if request.get("__autonomous_http_page_cursor") else ""),
        ),
        policy=_policy(),
        opener=opener,
    )

    result = executor(_manifest(), {})

    assert result.status == "partial"
    assert result.failure_class == "transport_error"
    assert result.value["items"] == [{"retained": True}]
    assert result.value["item_count"] == 1
    assert result.value["complete"] is False


def test_http_connector_page_rejects_secret_shaped_items() -> None:
    with pytest.raises(ArgumentError, match="credential-shaped"):
        AutonomousHttpConnectorPage(items=({"access_token": "never"},))


def test_pagination_enforces_aggregate_item_bytes_even_when_item_count_is_small() -> None:
    executor = create_autonomous_http_paginated_connector_executor(
        lambda _manifest, _request: AutonomousHttpConnectorRequest(method="GET", url="http://example.test/page"),
        policy=_policy(),
        opener=lambda _request, _timeout: _Response(
            json.dumps({"items": [{"payload": "x" * 1_500_000}]}).encode("utf-8")
        ),
    )

    result = executor(_manifest(), {})

    assert result.status == "partial"
    assert result.failure_class == "item_bytes_limit"
    assert result.value["items"] == []


def test_provider_specific_pagination_parser_can_normalize_a_nonstandard_envelope() -> None:
    executor = create_autonomous_http_paginated_connector_executor(
        lambda _manifest, _request: AutonomousHttpConnectorRequest(method="GET", url="http://example.test/custom"),
        policy=_policy(),
        page_parser=lambda value, _page_number: AutonomousHttpConnectorPage(
            items=value["records"],
            next_cursor=value["cursor"],
        ),
        opener=lambda _request, _timeout: _Response(
            json.dumps({"records": [{"normalized": True}], "cursor": None}).encode("utf-8")
        ),
    )

    result = executor(_manifest(), {})

    assert result.status == "observed"
    assert result.value["items"] == [{"normalized": True}]
    assert "records" not in repr(result)
