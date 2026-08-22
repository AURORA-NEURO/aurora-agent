from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousHttpConnectorPolicy,
    AutonomousHttpConnectorRequest,
    DomainEvidenceProviderConnectorManifest,
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
