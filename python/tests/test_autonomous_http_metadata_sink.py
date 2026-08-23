from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AUTONOMOUS_HTTP_METADATA_SINK_REQUEST_SCHEMA,
    AutonomousHttpConnectorPolicy,
    AutonomousHttpMetadataEventSink,
)
from prism_sdk.errors import ArgumentError, TransportError


class _Response:
    def __init__(self, payload: bytes = b"", *, status: int = 204) -> None:
        self.status = status
        self.headers = {"Content-Type": "application/json"}
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


def _policy() -> AutonomousHttpConnectorPolicy:
    return AutonomousHttpConnectorPolicy(
        allowed_hosts=("collector.test",),
        allowed_methods=("POST",),
    )


def _event(domain: str, sequence: int = 1) -> dict[str, object]:
    return {
        "schema": "bioprism-typescript-autonomous-run-trace-event/0.1",
        "run_id": f"python-export-{domain}",
        "sequence": sequence,
        "domains": [domain],
        "phase": "completed",
        "status": "completed",
        "event_digest": "a" * 64,
        "retention": "metadata_only_no_prompts_responses_or_tool_payloads",
        "secret_material": "never_returned",
    }


def test_metadata_sink_exports_every_domain_with_transient_headers() -> None:
    requests: list[tuple[str, str, dict[str, object], str | None]] = []

    def opener(request, _timeout):
        body = json.loads(request.data.decode("utf-8"))
        requests.append((request.get_method(), request.full_url, body, request.headers.get("Authorization")))
        return _Response()

    sink = AutonomousHttpMetadataEventSink(
        "https://collector.test/v1/metadata",
        source_id="python-runtime-test",
        policy=_policy(),
        header_resolver=lambda _manifest, _request: {"Authorization": "transient-test-credential"},
        opener=opener,
    )

    result = sink.emit_batch([_event(domain, index + 1) for index, domain in enumerate(AUTONOMOUS_DOMAINS)])

    assert result["requested"] == len(AUTONOMOUS_DOMAINS)
    assert result["exported"] == len(AUTONOMOUS_DOMAINS)
    assert result["already_exported"] == 0
    assert result["failed"] == 0
    assert len(result["batch_digest"]) == 64
    assert len(requests) == len(AUTONOMOUS_DOMAINS)
    assert all(method == "POST" and url.endswith("/v1/metadata") for method, url, _body, _auth in requests)
    assert all(auth == "transient-test-credential" for _method, _url, _body, auth in requests)
    assert all(body["schema"] == AUTONOMOUS_HTTP_METADATA_SINK_REQUEST_SCHEMA for _method, _url, body, _auth in requests)
    assert all(body["event_digest"] == body["idempotency_key"] for _method, _url, body, _auth in requests)
    assert all("transient-test-credential" not in json.dumps(body) for _method, _url, body, _auth in requests)
    assert sink.describe()["endpoint_host"] == "collector.test"


def test_metadata_sink_retries_transient_failures_and_deduplicates_409() -> None:
    statuses = iter((503, 204))
    delays: list[float] = []
    retrying = AutonomousHttpMetadataEventSink(
        "https://collector.test/v1/metadata",
        policy=_policy(),
        opener=lambda _request, _timeout: _Response(status=next(statuses)),
        retry_delay_seconds=0.5,
        sleep=delays.append,
    )

    exported = retrying.emit(_event("coding"))
    assert exported.status == "exported"
    assert exported.attempts == 2
    assert delays == [0.5]

    duplicate = AutonomousHttpMetadataEventSink(
        "https://collector.test/v1/metadata",
        policy=_policy(),
        opener=lambda _request, _timeout: _Response(status=409),
    )
    receipt = duplicate.emit(_event("browser"))
    assert receipt.status == "already_exported"
    assert receipt.status_code == 409
    assert receipt.failure_class == "already_exists"
    assert not receipt.retryable


def test_metadata_sink_preserves_refusal_and_transport_failure_classes() -> None:
    refused = AutonomousHttpMetadataEventSink(
        "https://collector.test/v1/metadata",
        policy=_policy(),
        opener=lambda _request, _timeout: _Response(status=401),
    )
    refusal = refused.emit(_event("data"))
    assert refusal.status == "refused"
    assert refusal.failure_class == "auth_refused"
    assert not refusal.retryable
    with pytest.raises(TransportError):
        refused.as_sink()(_event("data", 2))

    calls = 0

    def unavailable(_request, _timeout):
        nonlocal calls
        calls += 1
        raise OSError("collector unavailable")

    failed = AutonomousHttpMetadataEventSink(
        "https://collector.test/v1/metadata",
        policy=_policy(),
        opener=unavailable,
        max_attempts=2,
        retry_delay_seconds=0,
        sleep=lambda _delay: None,
    ).emit(_event("science"))
    assert failed.status == "failed"
    assert failed.failure_class == "transport_error"
    assert failed.attempts == 2
    assert calls == 2


def test_metadata_sink_rejects_secret_fields_schemas_batches_and_unsafe_policy() -> None:
    sink = AutonomousHttpMetadataEventSink(
        "https://collector.test/v1/metadata",
        policy=_policy(),
        opener=lambda _request, _timeout: _Response(),
    )
    with pytest.raises(ArgumentError):
        sink.emit({**_event("operations"), "prompt": "never export"})
    with pytest.raises(ArgumentError):
        sink.emit({**_event("enterprise"), "content": "raw provider output"})
    with pytest.raises(ArgumentError):
        sink.emit({**_event("evaluation"), "value": "secret-shaped"})
    with pytest.raises(ArgumentError):
        sink.emit({"schema": "unknown/0.1", "status": "completed"})
    with pytest.raises(ArgumentError):
        sink.emit({**_event("evaluation"), "metadata": "x" * 25_000})
    with pytest.raises(ArgumentError):
        sink.emit_batch([])
    with pytest.raises(ArgumentError):
        sink.emit_batch([_event("coding")] * 257)
    with pytest.raises(ArgumentError):
        AutonomousHttpMetadataEventSink(
            "https://collector.test/v1/metadata",
            policy=AutonomousHttpConnectorPolicy(allowed_hosts=("collector.test",), allowed_methods=("GET",)),
            opener=lambda _request, _timeout: _Response(),
        )


def test_metadata_sink_callback_exports_portfolio_trace_schema() -> None:
    bodies: list[dict[str, object]] = []
    sink = AutonomousHttpMetadataEventSink(
        "https://collector.test/v1/metadata",
        policy=_policy(),
        opener=lambda request, _timeout: (bodies.append(json.loads(request.data.decode("utf-8"))) or _Response()),
    )
    sink.as_sink()({
        **_event("cross_domain"),
        "schema": "bioprism-typescript-autonomous-workflow-portfolio-execution-trace-event/0.1",
    })
    assert bodies[0]["event"]["schema"].endswith("portfolio-execution-trace-event/0.1")
    assert bodies[0]["retention"] == "metadata_only_event_identity_and_delivery_status"
