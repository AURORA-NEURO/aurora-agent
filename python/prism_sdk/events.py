"""Typed cursor and webhook-outbox projections from the bounded HTTP gateway."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .errors import ArgumentError, ProtocolError


MAX_EVENT_PAGE = 1000


def _mapping(name: str, value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be a mapping")
    return dict(value)


def _text(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    return value


def _non_negative(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _texts(name: str, value: Any) -> tuple[str, ...]:
    if not isinstance(value, list):
        raise ArgumentError(f"{name} must be an array")
    return tuple(_text(f"{name}[{index}]", item) for index, item in enumerate(value))


def _review_id(name: str, value: Any) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ArgumentError(f"{name} must be a 64-character hexadecimal content hash")
    return value


def validate_review_id(value: Any) -> str:
    """Validate a content-addressed route-review identifier for HTTP query/path use."""

    return _review_id("review_id", value)


def validate_receipt_id(value: Any) -> str:
    """Validate a bounded delivery-receipt identifier for HTTP query/path use."""

    if not isinstance(value, str) or not value.strip() or len(value) > 128:
        raise ArgumentError("receipt_id must be a non-empty string of at most 128 characters")
    if any(ord(character) < 0x20 for character in value):
        raise ArgumentError("receipt_id must not contain control characters")
    return value


def _optional_bool(name: str, value: Any) -> bool | None:
    if value is None:
        return None
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean or null")
    return value


@dataclass(frozen=True)
class ApiEvent:
    """One retained, sequence-addressed event emitted by the HTTP boundary."""

    raw: dict[str, Any]
    id: int
    event_type: str
    subject: str
    request_id: str
    payload: Any

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ApiEvent":
        raw = _mapping("API event", value)
        return cls(
            raw=raw,
            id=_non_negative("API event id", raw.get("id")),
            event_type=_text("API event event_type", raw.get("event_type")),
            subject=_text("API event subject", raw.get("subject")),
            request_id=_text("API event request_id", raw.get("request_id")),
            payload=raw.get("payload"),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class EventPage:
    """Typed cursor page with explicit retention-gap evidence."""

    raw: dict[str, Any]
    events: tuple[ApiEvent, ...]
    after: int
    next_after: int
    oldest: int | None
    newest: int | None
    gap: bool
    dropped_events: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EventPage":
        envelope = _mapping("events response", value)
        page_value = envelope.get("page", envelope)
        raw = _mapping("event page", page_value)
        values = raw.get("events")
        if not isinstance(values, Sequence) or isinstance(values, (str, bytes)):
            raise ArgumentError("event page events must be an array")
        events = tuple(ApiEvent.from_wire(item) for item in values)
        ids = tuple(event.id for event in events)
        if ids != tuple(sorted(set(ids))):
            raise ArgumentError("event page ids must be sorted and unique")
        after = _non_negative("event page after", raw.get("after"))
        next_after = _non_negative("event page next_after", raw.get("next_after"))
        oldest = raw.get("oldest")
        if oldest is not None:
            oldest = _non_negative("event page oldest", oldest)
        newest = raw.get("newest")
        if newest is not None:
            newest = _non_negative("event page newest", newest)
        gap = raw.get("gap")
        if not isinstance(gap, bool):
            raise ArgumentError("event page gap must be a boolean")
        return cls(
            raw=raw,
            events=events,
            after=after,
            next_after=next_after,
            oldest=oldest,
            newest=newest,
            gap=gap,
            dropped_events=_non_negative("event page dropped_events", raw.get("dropped_events")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class RouteReviewEvidence:
    """Typed retained event evidence for one content-addressed route review."""

    raw: dict[str, Any]
    review_id: str
    found: bool
    page: EventPage

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RouteReviewEvidence":
        raw = _mapping("route-review evidence", value)
        if raw.get("workflow") != "capability_route_review_evidence":
            raise ArgumentError("route-review evidence workflow is invalid")
        found = raw.get("found")
        if not isinstance(found, bool):
            raise ArgumentError("route-review evidence found must be a boolean")
        page = EventPage.from_wire(raw.get("page"))
        review_id = _review_id("route-review evidence review_id", raw.get("review_id"))
        if found != bool(page.events):
            raise ArgumentError("route-review evidence found must match page events")
        return cls(raw=raw, review_id=review_id, found=found, page=page)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DeliveryReceiptEvents:
    """Typed retained event evidence for one content-addressed delivery receipt."""

    raw: dict[str, Any]
    receipt_id: str
    found: bool
    page: EventPage

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryReceiptEvents":
        raw = _mapping("delivery-receipt events", value)
        if raw.get("workflow") != "developer_delivery_receipt_events":
            raise ArgumentError("delivery-receipt events workflow is invalid")
        found = raw.get("found")
        if not isinstance(found, bool):
            raise ArgumentError("delivery-receipt events found must be a boolean")
        receipt_id = validate_receipt_id(raw.get("receipt_id"))
        page = EventPage.from_wire(raw.get("page"))
        if found != bool(page.events):
            raise ArgumentError("delivery-receipt events found must match page events")
        return cls(raw=raw, receipt_id=receipt_id, found=found, page=page)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class EventPersistenceStatus:
    """Typed operator view over the optional event-cursor checkpoint."""

    raw: dict[str, Any]
    enabled: bool
    file_present: bool
    file_bytes: int | None
    schema_version: int
    state_digest: str | None
    integrity_verified: bool | None
    max_file_bytes: int
    retained_events: int
    next_event_id: int
    dropped_events: int
    subscriptions_durable: bool
    webhook_deliveries_durable: bool
    secrets_persisted: bool
    recovery_policy: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EventPersistenceStatus":
        raw = _mapping("event persistence status", value)

        def non_negative(name: str) -> int:
            return _non_negative(f"event persistence {name}", raw.get(name))

        enabled = raw.get("enabled")
        file_present = raw.get("file_present")
        if not isinstance(enabled, bool) or not isinstance(file_present, bool):
            raise ArgumentError("event persistence enabled and file_present must be booleans")
        file_bytes = raw.get("file_bytes")
        if file_bytes is not None:
            file_bytes = _non_negative("event persistence file_bytes", file_bytes)
        state_digest = raw.get("state_digest")
        if state_digest is not None and (
            not isinstance(state_digest, str)
            or len(state_digest) != 64
            or any(character not in "0123456789abcdef" for character in state_digest)
        ):
            raise ArgumentError(
                "event persistence state_digest must be 64 lowercase hexadecimal characters"
            )
        integrity_verified = raw.get("integrity_verified")
        if integrity_verified is not None and not isinstance(integrity_verified, bool):
            raise ArgumentError("event persistence integrity_verified must be a boolean or null")
        subscriptions_durable = raw.get("subscriptions_durable")
        webhook_deliveries_durable = raw.get("webhook_deliveries_durable")
        secrets_persisted = raw.get("secrets_persisted", False)
        if not isinstance(subscriptions_durable, bool) or not isinstance(webhook_deliveries_durable, bool):
            raise ArgumentError("event persistence durability fields must be booleans")
        if not isinstance(secrets_persisted, bool):
            raise ArgumentError("event persistence secrets_persisted must be a boolean")
        if secrets_persisted:
            raise ArgumentError("event persistence must never persist webhook secrets")
        recovery_policy = _text("event persistence recovery_policy", raw.get("recovery_policy"))
        return cls(
            raw,
            enabled,
            file_present,
            file_bytes,
            non_negative("schema_version"),
            state_digest,
            integrity_verified,
            non_negative("max_file_bytes"),
            non_negative("retained_events"),
            non_negative("next_event_id"),
            non_negative("dropped_events"),
            subscriptions_durable,
            webhook_deliveries_durable,
            secrets_persisted,
            recovery_policy,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class RecoveryBoundary:
    """One explicit restart boundary reported by the gateway recovery matrix."""

    raw: dict[str, Any]
    id: str
    configured: bool
    checkpoint_present: bool
    schema_version: int | None
    state_digest: str | None
    integrity_verified: bool | None
    restores: tuple[str, ...]
    does_not_restore: tuple[str, ...]
    operator_action: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RecoveryBoundary":
        raw = _mapping("recovery boundary", value)
        boundary_id = _text("recovery boundary id", raw.get("id"))
        configured = raw.get("configured")
        checkpoint_present = raw.get("checkpoint_present")
        if not isinstance(configured, bool) or not isinstance(checkpoint_present, bool):
            raise ArgumentError("recovery boundary configured and checkpoint_present must be booleans")
        schema_version = raw.get("schema_version")
        if schema_version is not None:
            schema_version = _non_negative("recovery boundary schema_version", schema_version)
        state_digest = raw.get("state_digest")
        if state_digest is not None and (
            not isinstance(state_digest, str)
            or len(state_digest) != 64
            or any(character not in "0123456789abcdef" for character in state_digest)
        ):
            raise ArgumentError(
                "recovery boundary state_digest must be 64 lowercase hexadecimal characters"
            )
        integrity_verified = raw.get("integrity_verified")
        if integrity_verified is not None and not isinstance(integrity_verified, bool):
            raise ArgumentError("recovery boundary integrity_verified must be a boolean or null")
        return cls(
            raw=raw,
            id=boundary_id,
            configured=configured,
            checkpoint_present=checkpoint_present,
            schema_version=schema_version,
            state_digest=state_digest,
            integrity_verified=integrity_verified,
            restores=_texts("recovery boundary restores", raw.get("restores")),
            does_not_restore=_texts(
                "recovery boundary does_not_restore", raw.get("does_not_restore")
            ),
            operator_action=_text("recovery boundary operator_action", raw.get("operator_action")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class RecoveryMatrix:
    """Typed operator matrix for restart, secret, and external-effect boundaries."""

    raw: dict[str, Any]
    schema: str
    boundaries: tuple[RecoveryBoundary, ...]
    automatic_resume: bool
    automatic_external_delivery: bool
    observed: dict[str, int]
    guarantees: tuple[str, ...]
    non_claims: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RecoveryMatrix":
        raw = _mapping("recovery matrix", value)
        boundaries_value = raw.get("boundaries")
        if not isinstance(boundaries_value, list) or not boundaries_value:
            raise ArgumentError("recovery matrix boundaries must be a non-empty array")
        boundaries = tuple(
            RecoveryBoundary.from_wire(item) for item in boundaries_value
        )
        boundary_ids = [boundary.id for boundary in boundaries]
        if len(set(boundary_ids)) != len(boundary_ids):
            raise ArgumentError("recovery matrix boundary ids must be unique")
        automatic_resume = raw.get("automatic_resume")
        automatic_external_delivery = raw.get("automatic_external_delivery")
        if not isinstance(automatic_resume, bool) or not isinstance(automatic_external_delivery, bool):
            raise ArgumentError(
                "recovery matrix automatic resume and delivery fields must be booleans"
            )
        observed_raw = _mapping("recovery matrix observed", raw.get("observed"))
        observed = {
            name: _non_negative(f"recovery matrix observed {name}", number)
            for name, number in observed_raw.items()
        }
        return cls(
            raw=raw,
            schema=_text("recovery matrix schema", raw.get("schema")),
            boundaries=boundaries,
            automatic_resume=automatic_resume,
            automatic_external_delivery=automatic_external_delivery,
            observed=observed,
            guarantees=_texts("recovery matrix guarantees", raw.get("guarantees")),
            non_claims=_texts("recovery matrix non_claims", raw.get("non_claims")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DeliveryView:
    """One signed webhook delivery plus operator-visible failure and replay state."""

    raw: dict[str, Any]
    delivery_id: int
    subscription_id: str
    attempt: int
    state: str
    last_error: str | None
    last_error_retryable: bool | None
    event_id: int
    event_type: str
    signature: str
    envelope: Any

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryView":
        raw = _mapping("delivery", value)
        return cls(
            raw=raw,
            delivery_id=_non_negative("delivery id", raw.get("delivery_id")),
            subscription_id=_text("delivery subscription_id", raw.get("subscription_id")),
            attempt=_non_negative("delivery attempt", raw.get("attempt")),
            state=_text("delivery state", raw.get("state")),
            last_error=(
                None
                if raw.get("last_error") is None
                else _text("delivery last_error", raw.get("last_error"))
            ),
            last_error_retryable=_optional_bool(
                "delivery last_error_retryable", raw.get("last_error_retryable")
            ),
            event_id=_non_negative("delivery event_id", raw.get("event_id")),
            event_type=_text("delivery event_type", raw.get("event_type")),
            signature=_text("delivery signature", raw.get("signature")),
            envelope=raw.get("envelope"),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DeliveryPage:
    """Typed cursor page over pending signed webhook deliveries."""

    raw: dict[str, Any]
    deliveries: tuple[DeliveryView, ...]
    after: int
    next_after: int
    pending_count: int
    dropped_deliveries: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryPage":
        envelope = _mapping("deliveries response", value)
        page_value = envelope.get("page", envelope)
        raw = _mapping("delivery page", page_value)
        values = raw.get("deliveries")
        if not isinstance(values, Sequence) or isinstance(values, (str, bytes)):
            raise ArgumentError("delivery page deliveries must be an array")
        deliveries = tuple(DeliveryView.from_wire(item) for item in values)
        ids = tuple(delivery.delivery_id for delivery in deliveries)
        if ids != tuple(sorted(set(ids))):
            raise ArgumentError("delivery page ids must be sorted and unique")
        return cls(
            raw=raw,
            deliveries=deliveries,
            after=_non_negative("delivery page after", raw.get("after")),
            next_after=_non_negative("delivery page next_after", raw.get("next_after")),
            pending_count=_non_negative("delivery page pending_count", raw.get("pending_count")),
            dropped_deliveries=_non_negative("delivery page dropped_deliveries", raw.get("dropped_deliveries")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class SseEvent:
    """One parsed Server-Sent Events record from a bounded gateway snapshot."""

    data: str
    id: str | None = None
    event: str | None = None
    retry: int | None = None


@dataclass(frozen=True)
class SseSnapshot:
    """Parsed SSE snapshot plus the cursor header needed for the next request."""

    content_type: str
    next_after: int | None
    events: tuple[SseEvent, ...]
    raw: str


def parse_sse(value: str) -> tuple[SseEvent, ...]:
    """Parse one bounded SSE snapshot using EventSource field and dispatch rules."""

    if not isinstance(value, str):
        raise ProtocolError("SSE response must be text")
    events: list[SseEvent] = []
    data_lines: list[str] = []
    event_id: str | None = None
    event_name: str | None = None
    retry: int | None = None

    def dispatch() -> None:
        nonlocal data_lines, event_id, event_name, retry
        if data_lines:
            events.append(SseEvent("\n".join(data_lines), event_id, event_name, retry))
        data_lines = []
        event_id = None
        event_name = None
        retry = None

    for line in value.replace("\r\n", "\n").replace("\r", "\n").split("\n"):
        if line == "":
            dispatch()
            continue
        if line.startswith(":"):
            continue
        if ":" in line:
            field, field_value = line.split(":", 1)
            if field_value.startswith(" "):
                field_value = field_value[1:]
        else:
            field, field_value = line, ""
        if field == "id":
            if "\x00" in field_value:
                raise ProtocolError("SSE id contains a NUL character")
            event_id = field_value
        elif field == "event":
            event_name = field_value
        elif field == "data":
            data_lines.append(field_value)
        elif field == "retry":
            if not field_value.isdigit():
                raise ProtocolError("SSE retry is not an unsigned integer")
            retry = int(field_value)
            if retry > 9_007_199_254_740_991:
                raise ProtocolError("SSE retry exceeds safe integer range")
    dispatch()
    return tuple(events)


__all__ = [
    "MAX_EVENT_PAGE",
    "ApiEvent",
    "EventPage",
    "EventPersistenceStatus",
    "RecoveryBoundary",
    "RecoveryMatrix",
    "DeliveryView",
    "DeliveryPage",
    "SseEvent",
    "SseSnapshot",
    "parse_sse",
]
