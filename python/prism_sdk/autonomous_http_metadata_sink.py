"""Bounded HTTP export for already-redacted autonomous operational metadata.

The sink is deliberately narrower than an observability backend.  It accepts only caller-selected
event schemas, rejects secret- or payload-shaped fields recursively, and delegates network policy,
transient headers, timeout handling, and response-body redaction to the shared HTTP connector.
Collector authentication, durable ingestion, tenant isolation, and OTLP translation remain
deployment-owned.  A 409 is an idempotent duplicate, not a failed autonomous run.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
import re
import time
from typing import Any, Callable, Mapping, Sequence
from urllib.parse import urlsplit

from .authoring import canonical_bytes, canonical_json, content_digest
from .autonomous_connectors import AutonomousConnectorObservation
from .autonomous_http_connector import (
    AutonomousHttpConnectorPolicy,
    AutonomousHttpConnectorRequest,
    HeaderResolver,
    OpenRequest,
    create_autonomous_http_connector_executor,
)
from .autonomy import AUTONOMOUS_DOMAINS
from .errors import ArgumentError, TransportError


AUTONOMOUS_HTTP_METADATA_SINK_SCHEMA = "bioprism-python-autonomous-http-metadata-sink/0.1"
AUTONOMOUS_HTTP_METADATA_SINK_REQUEST_SCHEMA = "bioprism-python-autonomous-http-metadata-event/0.1"
AUTONOMOUS_HTTP_METADATA_SINK_RECEIPT_SCHEMA = "bioprism-python-autonomous-http-metadata-receipt/0.1"
MAX_AUTONOMOUS_HTTP_METADATA_EVENT_BYTES = 24_000
MAX_AUTONOMOUS_HTTP_METADATA_BATCH = 256
MAX_AUTONOMOUS_HTTP_METADATA_RETRY_ATTEMPTS = 8
MAX_AUTONOMOUS_HTTP_METADATA_RETRY_DELAY_SECONDS = 30.0

_DEFAULT_RETRY_DELAY_SECONDS = 0.25
_DEFAULT_ACCEPTED_SCHEMAS = (
    "bioprism-typescript-autonomous-run-trace-event/0.1",
    "bioprism-typescript-autonomous-workflow-portfolio-execution-trace-event/0.1",
    "bioprism-python-autonomous-run-trace-event/0.1",
    "bioprism-python-autonomous-workflow-portfolio-execution-trace-event/0.1",
)
_SECRET_FIELD_MARKERS = frozenset(
    {
        "apikey",
        "authorization",
        "bearer",
        "body",
        "content",
        "credential",
        "credentials",
        "headers",
        "messages",
        "password",
        "privatekey",
        "prompt",
        "providerresponse",
        "response",
        "secret",
        "token",
        "toolarguments",
        "tooloutput",
        "value",
    }
)
_RETRYABLE_FAILURES = frozenset({"rate_limited", "timeout", "transport_error", "http_5xx"})


def _normalized_key(value: str) -> str:
    return "".join(character for character in value.lower() if character.isalnum())


def _bounded_text(name: str, value: Any, maximum: int) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value


def _identifier(name: str, value: Any) -> str:
    text = _bounded_text(name, value, 256)
    if not re.fullmatch(r"[A-Za-z0-9_.:+-]+", text):
        raise ArgumentError(f"{name} is not a safe identifier")
    return text


def _secret_free_metadata(name: str, value: Any, maximum: int, depth: int = 0) -> Any:
    if depth > 16:
        raise ArgumentError(f"{name} is too deeply nested")
    if value is None or isinstance(value, (str, bool, int)):
        return value
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ArgumentError(f"{name} contains a non-finite number")
        return value
    if isinstance(value, (list, tuple)):
        if len(value) > MAX_AUTONOMOUS_HTTP_METADATA_BATCH:
            raise ArgumentError(f"{name} array exceeds its bound")
        result = [_secret_free_metadata(name, item, maximum, depth + 1) for item in value]
    elif isinstance(value, Mapping):
        result = {}
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip() or "\x00" in key:
                raise ArgumentError(f"{name} contains an invalid JSON field")
            marker = _normalized_key(key)
            if marker in _SECRET_FIELD_MARKERS or marker.startswith("gsk") or marker.startswith("skproj"):
                raise ArgumentError(f"{name} contains a transient or credential-shaped field")
            result[key] = _secret_free_metadata(f"{name}.{key}", child, maximum, depth + 1)
    else:
        raise ArgumentError(f"{name} must be JSON-safe")
    try:
        encoded = canonical_bytes(result)
    except Exception as error:
        raise ArgumentError(f"{name} is not canonical JSON") from error
    if len(encoded) > maximum:
        raise ArgumentError(f"{name} exceeds its byte bound")
    return result


def _status_code(observation: AutonomousConnectorObservation) -> int | None:
    value = observation.value
    code = value.get("status_code") if isinstance(value, Mapping) else None
    return code if isinstance(code, int) and not isinstance(code, bool) and 100 <= code <= 599 else None


@dataclass(frozen=True, slots=True)
class AutonomousHttpMetadataSinkReceipt:
    event_schema: str
    event_digest: str
    source_id: str
    status: str
    attempts: int
    status_code: int | None
    failure_class: str | None
    retryable: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_HTTP_METADATA_SINK_RECEIPT_SCHEMA,
            "event_schema": self.event_schema,
            "event_digest": self.event_digest,
            "source_id": self.source_id,
            "status": self.status,
            "attempts": self.attempts,
            "status_code": self.status_code,
            "failure_class": self.failure_class,
            "retryable": self.retryable,
            "transport": "bounded_http_connector;idempotency_key_is_event_digest",
            "retention": "metadata_only_event_identity_and_delivery_status",
            "secret_material": "never_returned",
        }


class AutonomousHttpMetadataEventSink:
    """Export pre-redacted event metadata to a caller-owned HTTP collector."""

    def __init__(
        self,
        endpoint: str,
        *,
        accepted_schemas: Sequence[str] | None = None,
        source_id: str = "aurora-autonomous-runtime",
        policy: AutonomousHttpConnectorPolicy | None = None,
        header_resolver: HeaderResolver | None = None,
        opener: OpenRequest | None = None,
        max_attempts: int = 3,
        retry_delay_seconds: float = _DEFAULT_RETRY_DELAY_SECONDS,
        sleep: Callable[[float], None] | None = None,
    ) -> None:
        self.endpoint = _bounded_text("HTTP metadata sink endpoint", endpoint, 8_192)
        if any(character.isspace() for character in self.endpoint):
            raise ArgumentError("HTTP metadata sink endpoint contains whitespace")
        raw_schemas = _DEFAULT_ACCEPTED_SCHEMAS if accepted_schemas is None else accepted_schemas
        if isinstance(raw_schemas, (str, bytes)) or not isinstance(raw_schemas, Sequence) or not 1 <= len(raw_schemas) <= 32:
            raise ArgumentError("HTTP metadata sink accepted_schemas is outside its bound")
        self.accepted_schemas = tuple(_bounded_text("HTTP metadata sink accepted schema", schema, 256) for schema in raw_schemas)
        if len(set(self.accepted_schemas)) != len(self.accepted_schemas):
            raise ArgumentError("HTTP metadata sink accepted_schemas contains duplicates")
        self.source_id = _identifier("HTTP metadata sink source_id", source_id)
        self.policy = policy or AutonomousHttpConnectorPolicy(allowed_methods=("POST",))
        if not isinstance(self.policy, AutonomousHttpConnectorPolicy) or "POST" not in self.policy.allowed_methods:
            raise ArgumentError("HTTP metadata sink policy must allow POST")
        if isinstance(max_attempts, bool) or not isinstance(max_attempts, int) or not 1 <= max_attempts <= MAX_AUTONOMOUS_HTTP_METADATA_RETRY_ATTEMPTS:
            raise ArgumentError("HTTP metadata sink max_attempts is outside its bound")
        if isinstance(retry_delay_seconds, bool) or not isinstance(retry_delay_seconds, (int, float)) or not 0 <= float(retry_delay_seconds) <= MAX_AUTONOMOUS_HTTP_METADATA_RETRY_DELAY_SECONDS:
            raise ArgumentError("HTTP metadata sink retry_delay_seconds is outside its bound")
        self.max_attempts = max_attempts
        self.retry_delay_seconds = float(retry_delay_seconds)
        self.sleep = sleep or time.sleep
        if not callable(self.sleep):
            raise ArgumentError("HTTP metadata sink sleep must be callable")
        self._manifest = {
            "schema": "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
            "connector_id": "autonomous.http.metadata-sink",
            "version": "0.1.0",
            "provider": "caller-http",
            "connector_kind": "provider_api",
            "domains": list(AUTONOMOUS_DOMAINS),
            "capabilities": ["metadata_event_export"],
            "transport": "caller_managed",
            "auth_posture": {
                "status": "delegated",
                "secret_refs": [],
                "does_not_claim": ["collector authorization is valid", "collector storage is durable", "metadata is task truth"],
            },
        }
        self._execute = create_autonomous_http_connector_executor(
            lambda _manifest, request: AutonomousHttpConnectorRequest(method="POST", url=self.endpoint, body=request),
            policy=self.policy,
            header_resolver=header_resolver,
            opener=opener,
        )

    def describe(self) -> dict[str, Any]:
        try:
            host = urlsplit(self.endpoint).hostname or "unknown"
        except ValueError:
            host = "unknown"
        return {
            "schema": AUTONOMOUS_HTTP_METADATA_SINK_SCHEMA,
            "source_id": self.source_id,
            "endpoint_host": host,
            "accepted_schemas": list(self.accepted_schemas),
            "max_attempts": self.max_attempts,
            "retry_delay_seconds": self.retry_delay_seconds,
            "idempotency": "event_digest;collector_409_is_already_exported",
            "transport": "bounded_http_connector;caller_header_resolver",
            "retention": "metadata_only;event_payload_must_be_pre_redacted",
            "secret_material": "never_returned",
        }

    def emit(self, event: Mapping[str, Any]) -> AutonomousHttpMetadataSinkReceipt:
        safe_event = _secret_free_metadata("HTTP metadata sink event", event, MAX_AUTONOMOUS_HTTP_METADATA_EVENT_BYTES)
        if not isinstance(safe_event, dict) or not isinstance(safe_event.get("schema"), str) or safe_event["schema"] not in self.accepted_schemas:
            raise ArgumentError("HTTP metadata sink event schema is not accepted")
        event_schema = safe_event["schema"]
        event_digest = content_digest(safe_event)
        request = {
            "schema": AUTONOMOUS_HTTP_METADATA_SINK_REQUEST_SCHEMA,
            "source_id": self.source_id,
            "event": safe_event,
            "event_digest": event_digest,
            "idempotency_key": event_digest,
            "retention": "metadata_only_event_identity_and_delivery_status",
            "secret_material": "never_returned",
        }
        if len(canonical_json(request).encode("utf-8")) > self.policy.max_request_bytes:
            raise ArgumentError("HTTP metadata sink request exceeds its bound")
        for attempt in range(1, self.max_attempts + 1):
            observation = self._execute(self._manifest, request)
            if not isinstance(observation, AutonomousConnectorObservation):
                raise ArgumentError("HTTP metadata sink transport returned an invalid observation")
            status_code = _status_code(observation)
            failure = observation.failure_class
            retryable = failure in _RETRYABLE_FAILURES
            if observation.status == "observed":
                return AutonomousHttpMetadataSinkReceipt(event_schema, event_digest, self.source_id, "exported", attempt, status_code, None, False)
            if status_code == 409:
                return AutonomousHttpMetadataSinkReceipt(event_schema, event_digest, self.source_id, "already_exported", attempt, status_code, "already_exists", False)
            status = "refused" if observation.status == "refused" else "failed"
            receipt = AutonomousHttpMetadataSinkReceipt(event_schema, event_digest, self.source_id, status, attempt, status_code, failure, retryable)
            if not retryable or attempt >= self.max_attempts:
                return receipt
            self.sleep(self.retry_delay_seconds * (2 ** (attempt - 1)))
        raise TransportError("HTTP metadata sink exhausted its bounded retry attempts")

    def emit_batch(self, events: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
        if isinstance(events, (str, bytes)) or not isinstance(events, Sequence) or not 1 <= len(events) <= MAX_AUTONOMOUS_HTTP_METADATA_BATCH:
            raise ArgumentError("HTTP metadata sink batch is outside its bound")
        receipts = [self.emit(event) for event in events]
        receipt_dicts = [receipt.to_dict() for receipt in receipts]
        result: dict[str, Any] = {
            "schema": AUTONOMOUS_HTTP_METADATA_SINK_SCHEMA,
            "source_id": self.source_id,
            "requested": len(receipts),
            "exported": sum(receipt.status == "exported" for receipt in receipts),
            "already_exported": sum(receipt.status == "already_exported" for receipt in receipts),
            "refused": sum(receipt.status == "refused" for receipt in receipts),
            "failed": sum(receipt.status == "failed" for receipt in receipts),
            "receipts": receipt_dicts,
            "retention": "metadata_only_event_identity_and_delivery_status",
            "secret_material": "never_returned",
        }
        result["batch_digest"] = content_digest(result)
        return result

    def as_sink(self) -> Callable[[Mapping[str, Any]], None]:
        def write(event: Mapping[str, Any]) -> None:
            receipt = self.emit(event)
            if receipt.status not in {"exported", "already_exported"}:
                raise TransportError(f"HTTP metadata sink refused event export: {receipt.failure_class or receipt.status}")

        return write


__all__ = [
    "AUTONOMOUS_HTTP_METADATA_SINK_SCHEMA",
    "AUTONOMOUS_HTTP_METADATA_SINK_REQUEST_SCHEMA",
    "AUTONOMOUS_HTTP_METADATA_SINK_RECEIPT_SCHEMA",
    "MAX_AUTONOMOUS_HTTP_METADATA_EVENT_BYTES",
    "MAX_AUTONOMOUS_HTTP_METADATA_BATCH",
    "MAX_AUTONOMOUS_HTTP_METADATA_RETRY_ATTEMPTS",
    "MAX_AUTONOMOUS_HTTP_METADATA_RETRY_DELAY_SECONDS",
    "AutonomousHttpMetadataSinkReceipt",
    "AutonomousHttpMetadataEventSink",
]
