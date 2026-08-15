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
class DeliveryView:
    """One signed webhook outbox delivery, including its retry attempt."""

    raw: dict[str, Any]
    delivery_id: int
    subscription_id: str
    attempt: int
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
    "DeliveryView",
    "DeliveryPage",
    "SseEvent",
    "SseSnapshot",
    "parse_sse",
]
