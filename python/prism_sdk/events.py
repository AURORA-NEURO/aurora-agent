"""Typed cursor and webhook-outbox projections from the bounded HTTP gateway."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .errors import ArgumentError, ProtocolError
from .capability import (
    DomainWorkflowReconciliationPersistenceStatus,
    DomainWorkflowReconciliationSummaryReport,
)
from .mission import MissionPersistenceStatus


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


def _optional_digest(name: str, value: Any) -> str | None:
    if value is None:
        return None
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ArgumentError(f"{name} must be a 64-character lowercase hexadecimal digest")
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
    delivery_attempts_durable: bool
    delivery_receipt_metadata_durable: bool
    secrets_persisted: bool
    retained_delivery_attempts: int
    dropped_delivery_attempts: int
    next_attempt_id: int
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
        delivery_attempts_durable = raw.get("delivery_attempts_durable", False)
        delivery_receipt_metadata_durable = raw.get("delivery_receipt_metadata_durable", False)
        secrets_persisted = raw.get("secrets_persisted", False)
        if (
            not isinstance(subscriptions_durable, bool)
            or not isinstance(webhook_deliveries_durable, bool)
            or not isinstance(delivery_attempts_durable, bool)
            or not isinstance(delivery_receipt_metadata_durable, bool)
        ):
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
            delivery_attempts_durable,
            delivery_receipt_metadata_durable,
            secrets_persisted,
            _non_negative(
                "event persistence retained_delivery_attempts",
                raw.get("retained_delivery_attempts", 0),
            ),
            _non_negative(
                "event persistence dropped_delivery_attempts",
                raw.get("dropped_delivery_attempts", 0),
            ),
            _non_negative("event persistence next_attempt_id", raw.get("next_attempt_id", 0)),
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


MAX_OPERATIONS_SNAPSHOT_LIMIT = 256
MAX_OPERATIONS_DOMAIN_GROUPS = 64
MAX_OPERATIONS_DOMAIN_TOOLS = 256


@dataclass(frozen=True)
class OperationsDomainGroup:
    """One bounded workspace capability group and exact tool-name coverage."""

    raw: dict[str, Any]
    id: str
    status: str
    domains: tuple[str, ...]
    declared_tool_count: int
    advertised_tool_count: int
    missing_tool_count: int
    missing_tools: tuple[str, ...]
    fully_advertised: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsDomainGroup":
        raw = _mapping("operations domain group", value)
        declared = _non_negative(
            "operations domain group declared_tool_count", raw.get("declared_tool_count")
        )
        advertised = _non_negative(
            "operations domain group advertised_tool_count", raw.get("advertised_tool_count")
        )
        missing = _non_negative(
            "operations domain group missing_tool_count", raw.get("missing_tool_count")
        )
        missing_tools = _texts("operations domain group missing_tools", raw.get("missing_tools"))
        fully_advertised = raw.get("fully_advertised")
        if not isinstance(fully_advertised, bool):
            raise ArgumentError("operations domain group fully_advertised must be a boolean")
        if advertised + missing != declared or missing != len(missing_tools):
            raise ArgumentError("operations domain group tool counts must reconcile")
        if fully_advertised != (missing == 0):
            raise ArgumentError("operations domain group fully_advertised is inconsistent")
        return cls(
            raw=raw,
            id=_text("operations domain group id", raw.get("id")),
            status=_text("operations domain group status", raw.get("status")),
            domains=_texts("operations domain group domains", raw.get("domains")),
            declared_tool_count=declared,
            advertised_tool_count=advertised,
            missing_tool_count=missing,
            missing_tools=missing_tools,
            fully_advertised=fully_advertised,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class OperationsDomainCoverage:
    """Typed aggregate of exact capability-group/tool catalogue coverage."""

    raw: dict[str, Any]
    schema: str
    groups: tuple[OperationsDomainGroup, ...]
    group_count: int
    returned_groups: int
    truncated: bool
    domain_label_count: int
    declared_tool_memberships: int
    unique_declared_tools: int
    advertised_tool_count: int
    fully_advertised_group_count: int
    groups_with_gaps: int
    declared_tools_not_advertised: tuple[str, ...]
    omitted_declared_tools_not_advertised: int
    advertised_tools_without_group: tuple[str, ...]
    omitted_advertised_tools_without_group: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsDomainCoverage":
        raw = _mapping("operations domain coverage", value)
        if raw.get("schema") != "bioprism-domain-coverage/0.1":
            raise ArgumentError("operations domain coverage schema is invalid")
        values = raw.get("groups")
        if not isinstance(values, Sequence) or isinstance(values, (str, bytes)):
            raise ArgumentError("operations domain coverage groups must be an array")
        groups = tuple(OperationsDomainGroup.from_wire(item) for item in values)

        def non_negative(name: str) -> int:
            return _non_negative(f"operations domain coverage {name}", raw.get(name))

        group_count = non_negative("group_count")
        returned_groups = non_negative("returned_groups")
        if group_count < returned_groups or returned_groups != len(groups):
            raise ArgumentError("operations domain coverage group counts must reconcile")
        if returned_groups > MAX_OPERATIONS_DOMAIN_GROUPS:
            raise ArgumentError("operations domain coverage returned too many groups")
        truncated = raw.get("truncated")
        if not isinstance(truncated, bool) or truncated != (group_count > returned_groups):
            raise ArgumentError("operations domain coverage truncation is inconsistent")
        declared_tools_not_advertised = _texts(
            "operations domain coverage declared_tools_not_advertised",
            raw.get("declared_tools_not_advertised"),
        )
        advertised_tools_without_group = _texts(
            "operations domain coverage advertised_tools_without_group",
            raw.get("advertised_tools_without_group"),
        )
        omitted_declared = non_negative("omitted_declared_tools_not_advertised")
        omitted_advertised = non_negative("omitted_advertised_tools_without_group")
        if len(declared_tools_not_advertised) > MAX_OPERATIONS_DOMAIN_TOOLS:
            raise ArgumentError("operations domain coverage declared tool projection is too large")
        if len(advertised_tools_without_group) > MAX_OPERATIONS_DOMAIN_TOOLS:
            raise ArgumentError("operations domain coverage advertised tool projection is too large")
        fully_advertised = non_negative("fully_advertised_group_count")
        groups_with_gaps = non_negative("groups_with_gaps")
        if fully_advertised + groups_with_gaps != group_count:
            raise ArgumentError("operations domain coverage group health counts must reconcile")
        return cls(
            raw=raw,
            schema=_text("operations domain coverage schema", raw.get("schema")),
            groups=groups,
            group_count=group_count,
            returned_groups=returned_groups,
            truncated=truncated,
            domain_label_count=non_negative("domain_label_count"),
            declared_tool_memberships=non_negative("declared_tool_memberships"),
            unique_declared_tools=non_negative("unique_declared_tools"),
            advertised_tool_count=non_negative("advertised_tool_count"),
            fully_advertised_group_count=fully_advertised,
            groups_with_gaps=groups_with_gaps,
            declared_tools_not_advertised=declared_tools_not_advertised,
            omitted_declared_tools_not_advertised=omitted_declared,
            advertised_tools_without_group=advertised_tools_without_group,
            omitted_advertised_tools_without_group=omitted_advertised,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class OperationsHandoffGroup:
    """One selected domain group plus its non-executing routing action."""

    raw: dict[str, Any]
    coverage: OperationsDomainGroup
    route_need_id: str
    next_action: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsHandoffGroup":
        raw = _mapping("operations handoff group", value)
        return cls(
            raw=raw,
            coverage=OperationsDomainGroup.from_wire(raw),
            route_need_id=_text("operations handoff route_need_id", raw.get("route_need_id")),
            next_action=_text("operations handoff next_action", raw.get("next_action")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class OperationsHandoff:
    """Typed, content-addressed, non-executing domain routing handoff."""

    raw: dict[str, Any]
    workflow: str
    schema: str
    handoff_id: str
    domain_coverage_digest: str
    goal: str
    selection: dict[str, Any]
    coverage: dict[str, Any]
    groups: tuple[OperationsHandoffGroup, ...]
    route_request: dict[str, Any]
    execution_prerequisites: dict[str, Any]
    handoff_status: str
    execution: str
    next_steps: tuple[str, ...]
    guarantees: tuple[str, ...]
    non_claims: tuple[str, ...]
    links: dict[str, str]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsHandoff":
        raw = _mapping("operations handoff", value)
        if raw.get("ok") is not True:
            raise ArgumentError("operations handoff must be successful")
        if raw.get("workflow") != "operations_domain_handoff":
            raise ArgumentError("operations handoff workflow is invalid")
        if raw.get("schema") != "bioprism-operations-handoff/0.1":
            raise ArgumentError("operations handoff schema is invalid")
        groups_value = raw.get("groups")
        if not isinstance(groups_value, Sequence) or isinstance(groups_value, (str, bytes)):
            raise ArgumentError("operations handoff groups must be an array")
        groups = tuple(OperationsHandoffGroup.from_wire(item) for item in groups_value)
        selection = _mapping("operations handoff selection", raw.get("selection"))
        coverage = _mapping("operations handoff coverage", raw.get("coverage"))
        route_request = _mapping("operations handoff route_request", raw.get("route_request"))
        execution_prerequisites = _mapping(
            "operations handoff execution_prerequisites", raw.get("execution_prerequisites")
        )
        links_raw = _mapping("operations handoff links", raw.get("links"))
        links = {
            _text("operations handoff link name", name): _text(
                f"operations handoff link {name}", target
            )
            for name, target in links_raw.items()
        }
        handoff_status = _text("operations handoff status", raw.get("handoff_status"))
        if handoff_status not in {
            "unresolved_domain",
            "no_actionable_gaps",
            "requires_catalogue_review",
            "ready_for_capability_route",
        }:
            raise ArgumentError("operations handoff status is invalid")
        execution = _text("operations handoff execution", raw.get("execution"))
        if execution != "not_started":
            raise ArgumentError("operations handoff execution must remain not_started")
        return cls(
            raw=raw,
            workflow="operations_domain_handoff",
            schema="bioprism-operations-handoff/0.1",
            handoff_id=_review_id("operations handoff handoff_id", raw.get("handoff_id")),
            domain_coverage_digest=_review_id(
                "operations handoff domain_coverage_digest",
                raw.get("domain_coverage_digest"),
            ),
            goal=_text("operations handoff goal", raw.get("goal")),
            selection=selection,
            coverage=coverage,
            groups=groups,
            route_request=route_request,
            execution_prerequisites=execution_prerequisites,
            handoff_status=handoff_status,
            execution=execution,
            next_steps=_texts("operations handoff next_steps", raw.get("next_steps")),
            guarantees=_texts("operations handoff guarantees", raw.get("guarantees")),
            non_claims=_texts("operations handoff non_claims", raw.get("non_claims")),
            links=links,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class OperationsDomainActivityGroup:
    """One capability group with bounded local event observations."""

    raw: dict[str, Any]
    coverage: OperationsDomainGroup
    observed_event_count: int
    observed_tool_count: int
    observed_tools: tuple[str, ...]
    unobserved_advertised_tool_count: int
    last_event_id: int | None
    activity_state: str
    observation_scope: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsDomainActivityGroup":
        raw = _mapping("operations domain activity group", value)
        observed_tools = _texts(
            "operations domain activity observed_tools", raw.get("observed_tools")
        )
        observed_tool_count = _non_negative(
            "operations domain activity observed_tool_count",
            raw.get("observed_tool_count"),
        )
        if observed_tool_count != len(observed_tools):
            raise ArgumentError("operations domain activity observed tool counts must reconcile")
        last_event_id = raw.get("last_event_id")
        if last_event_id is not None:
            last_event_id = _non_negative(
                "operations domain activity last_event_id", last_event_id
            )
        activity_state = _text(
            "operations domain activity activity_state", raw.get("activity_state")
        )
        if activity_state not in {
            "catalogue_gap",
            "observed_in_page",
            "catalogued_unobserved_in_page",
        }:
            raise ArgumentError("operations domain activity state is invalid")
        return cls(
            raw=raw,
            coverage=OperationsDomainGroup.from_wire(raw),
            observed_event_count=_non_negative(
                "operations domain activity observed_event_count",
                raw.get("observed_event_count"),
            ),
            observed_tool_count=observed_tool_count,
            observed_tools=observed_tools,
            unobserved_advertised_tool_count=_non_negative(
                "operations domain activity unobserved_advertised_tool_count",
                raw.get("unobserved_advertised_tool_count"),
            ),
            last_event_id=last_event_id,
            activity_state=activity_state,
            observation_scope=_text(
                "operations domain activity observation_scope",
                raw.get("observation_scope"),
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class OperationsDomainActivity:
    """Typed per-domain activity projection with explicit cursor scope."""

    raw: dict[str, Any]
    workflow: str
    schema: str
    event_cursor: dict[str, Any]
    groups: tuple[OperationsDomainActivityGroup, ...]
    summary: dict[str, int]
    observation_policy: dict[str, Any]
    guarantees: tuple[str, ...]
    non_claims: tuple[str, ...]
    links: dict[str, str]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsDomainActivity":
        raw = _mapping("operations domain activity", value)
        if raw.get("ok") is not True:
            raise ArgumentError("operations domain activity must be successful")
        if raw.get("workflow") != "operations_domain_activity":
            raise ArgumentError("operations domain activity workflow is invalid")
        if raw.get("schema") != "bioprism-operations-domain-activity/0.1":
            raise ArgumentError("operations domain activity schema is invalid")
        event_cursor = _mapping("operations domain activity event_cursor", raw.get("event_cursor"))
        for name in ("after", "next_after", "returned_events", "dropped_events"):
            _non_negative(f"operations domain activity event_cursor {name}", event_cursor.get(name))
        for name in ("oldest", "newest"):
            if event_cursor.get(name) is not None:
                _non_negative(
                    f"operations domain activity event_cursor {name}", event_cursor.get(name)
                )
        if not isinstance(event_cursor.get("gap"), bool):
            raise ArgumentError("operations domain activity event_cursor gap must be a boolean")
        groups_value = raw.get("groups")
        if not isinstance(groups_value, Sequence) or isinstance(groups_value, (str, bytes)):
            raise ArgumentError("operations domain activity groups must be an array")
        groups = tuple(OperationsDomainActivityGroup.from_wire(item) for item in groups_value)
        summary_raw = _mapping("operations domain activity summary", raw.get("summary"))
        summary = {
            name: _non_negative(
                f"operations domain activity summary {name}", summary_raw.get(name)
            )
            for name in (
                "group_count",
                "returned_groups",
                "tool_events_scanned",
                "attributed_tool_events",
                "unattributed_tool_events",
                "groups_with_catalogue_gaps",
                "groups_with_observed_activity",
                "catalogued_unobserved_tool_count",
            )
        }
        policy = _mapping(
            "operations domain activity observation_policy", raw.get("observation_policy")
        )
        if policy.get("readiness_claimed") is not False:
            raise ArgumentError("operations domain activity must not claim readiness")
        links_raw = _mapping("operations domain activity links", raw.get("links"))
        links = {
            _text("operations domain activity link name", name): _text(
                f"operations domain activity link {name}", target
            )
            for name, target in links_raw.items()
        }
        return cls(
            raw=raw,
            workflow="operations_domain_activity",
            schema="bioprism-operations-domain-activity/0.1",
            event_cursor=event_cursor,
            groups=groups,
            summary=summary,
            observation_policy=policy,
            guarantees=_texts(
                "operations domain activity guarantees", raw.get("guarantees")
            ),
            non_claims=_texts(
                "operations domain activity non_claims", raw.get("non_claims")
            ),
            links=links,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class OperationsDomainGateGroup:
    """One capability group with pooled and domain-bound evidence-channel gate states."""

    raw: dict[str, Any]
    coverage: OperationsDomainGroup
    gate_state: str
    readiness_claimed: bool
    gates: dict[str, dict[str, Any]]
    last_event_id: int | None
    evidence_scope: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsDomainGateGroup":
        raw = _mapping("operations domain gate group", value)
        gate_state = _text("operations domain gate gate_state", raw.get("gate_state"))
        if gate_state not in {
            "catalogue_blocked",
            "insufficient_evidence",
            "review_required",
        }:
            raise ArgumentError("operations domain gate state is invalid")
        if raw.get("readiness_claimed") is not False:
            raise ArgumentError("operations domain gate group must not claim readiness")
        gates_raw = _mapping("operations domain gate gates", raw.get("gates"))
        required = {
            "catalogue",
            "observed_activity",
            "transport_completion",
            "evaluation_evidence",
            "safety_evidence",
            "release_evidence",
        }
        if not required.issubset(gates_raw):
            raise ArgumentError("operations domain gate group is missing a required gate")
        if "domain_evaluator_evidence" in gates_raw:
            required.add("domain_evaluator_evidence")
            if not required.issubset(gates_raw):
                raise ArgumentError("operations domain gate group is missing a required gate")
        gates = {name: _mapping(f"operations domain gate {name}", gates_raw[name]) for name in gates_raw}
        last_event_id = raw.get("last_event_id")
        if last_event_id is not None:
            last_event_id = _non_negative("operations domain gate last_event_id", last_event_id)
        return cls(
            raw=raw,
            coverage=OperationsDomainGroup.from_wire(raw),
            gate_state=gate_state,
            readiness_claimed=False,
            gates=gates,
            last_event_id=last_event_id,
            evidence_scope=_text("operations domain gate evidence_scope", raw.get("evidence_scope")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class OperationsDomainGates:
    """Typed evidence gates that never convert local activity into readiness."""

    raw: dict[str, Any]
    workflow: str
    schema: str
    gate_digest: str
    gate_digest_scope: str
    event_cursor: dict[str, Any]
    groups: tuple[OperationsDomainGateGroup, ...]
    summary: dict[str, Any]
    gate_policy: dict[str, Any]
    guarantees: tuple[str, ...]
    non_claims: tuple[str, ...]
    links: dict[str, str]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsDomainGates":
        raw = _mapping("operations domain gates", value)
        if raw.get("ok") is not True:
            raise ArgumentError("operations domain gates must be successful")
        if raw.get("workflow") != "operations_domain_gates":
            raise ArgumentError("operations domain gates workflow is invalid")
        if raw.get("schema") != "bioprism-operations-domain-gates/0.1":
            raise ArgumentError("operations domain gates schema is invalid")
        gate_digest = _review_id("operations domain gates gate_digest", raw.get("gate_digest"))
        gate_digest_scope = _text(
            "operations domain gates gate_digest_scope", raw.get("gate_digest_scope")
        )
        if gate_digest_scope != "tool_evidence_projection_without_gate_digest":
            raise ArgumentError("operations domain gates digest scope is invalid")
        event_cursor = _mapping("operations domain gates event_cursor", raw.get("event_cursor"))
        for name in ("after", "next_after", "returned_events", "dropped_events"):
            _non_negative(f"operations domain gates event_cursor {name}", event_cursor.get(name))
        for name in ("oldest", "newest"):
            if event_cursor.get(name) is not None:
                _non_negative(f"operations domain gates event_cursor {name}", event_cursor.get(name))
        if not isinstance(event_cursor.get("gap"), bool):
            raise ArgumentError("operations domain gates event_cursor gap must be a boolean")
        groups_value = raw.get("groups")
        if not isinstance(groups_value, Sequence) or isinstance(groups_value, (str, bytes)):
            raise ArgumentError("operations domain gates groups must be an array")
        groups = tuple(OperationsDomainGateGroup.from_wire(item) for item in groups_value)
        summary_raw = _mapping("operations domain gates summary", raw.get("summary"))
        summary: dict[str, Any] = {}
        for name in (
            "group_count",
            "returned_groups",
            "tool_events_scanned",
            "attributed_tool_events",
            "unattributed_tool_events",
            "completed_tool_events",
            "refused_tool_events",
            "evaluation_evidence_events",
            "domain_evaluator_evidence_events",
            "safety_evidence_events",
            "release_evidence_events",
            "groups_blocked_catalogue",
            "groups_insufficient_evidence",
            "groups_review_required",
        ):
            value = summary_raw.get(name, 0)
            summary[name] = _non_negative(f"operations domain gates summary {name}", value)
        if summary_raw.get("readiness_claimed") is not False:
            raise ArgumentError("operations domain gates summary must not claim readiness")
        summary["readiness_claimed"] = False
        gate_policy = _mapping("operations domain gates gate_policy", raw.get("gate_policy"))
        if gate_policy.get("readiness_claimed") is not False:
            raise ArgumentError("operations domain gates policy must not claim readiness")
        links_raw = _mapping("operations domain gates links", raw.get("links"))
        links = {
            _text("operations domain gates link name", name): _text(
                f"operations domain gates link {name}", target
            )
            for name, target in links_raw.items()
        }
        return cls(
            raw=raw,
            workflow="operations_domain_gates",
            schema="bioprism-operations-domain-gates/0.1",
            gate_digest=gate_digest,
            gate_digest_scope=gate_digest_scope,
            event_cursor=event_cursor,
            groups=groups,
            summary=summary,
            gate_policy=gate_policy,
            guarantees=_texts("operations domain gates guarantees", raw.get("guarantees")),
            non_claims=_texts("operations domain gates non_claims", raw.get("non_claims")),
            links=links,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class OperationsGateReview:
    """One durable, content-addressed operator review of bounded gate evidence."""

    raw: dict[str, Any]
    review_id: str
    event_id: int
    request_id: str
    acceptance: dict[str, Any]
    gate_digest: str
    group_ids: tuple[str, ...]
    evidence: tuple[dict[str, Any], ...]
    replay: str
    readiness_claimed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsGateReview":
        raw = _mapping("operations gate review", value)
        review_id = _review_id("operations gate review review_id", raw.get("review_id"))
        acceptance = _mapping("operations gate review acceptance", raw.get("acceptance"))
        if acceptance.get("review_id") != review_id:
            raise ArgumentError("operations gate review acceptance review_id must match")
        evidence_raw = raw.get("evidence")
        if not isinstance(evidence_raw, Sequence) or isinstance(evidence_raw, (str, bytes)):
            raise ArgumentError("operations gate review evidence must be an array")
        if raw.get("readiness_claimed") is not False:
            raise ArgumentError("operations gate review must not claim readiness")
        return cls(
            raw=raw,
            review_id=review_id,
            event_id=_non_negative("operations gate review event_id", raw.get("event_id")),
            request_id=_text("operations gate review request_id", raw.get("request_id")),
            acceptance=acceptance,
            gate_digest=_review_id("operations gate review gate_digest", raw.get("gate_digest")),
            group_ids=_texts("operations gate review group_ids", raw.get("group_ids")),
            evidence=tuple(_mapping("operations gate review evidence row", item) for item in evidence_raw),
            replay=_text("operations gate review replay", raw.get("replay")),
            readiness_claimed=False,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class OperationsGateReviews:
    """Cursor page of durable operations gate-review records with retention evidence."""

    raw: dict[str, Any]
    workflow: str
    schema: str
    review_id: str | None
    found: bool
    page: EventPage
    reviews: tuple[OperationsGateReview, ...]
    review_count: int
    readiness_claimed: bool
    guarantees: tuple[str, ...]
    non_claims: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsGateReviews":
        raw = _mapping("operations gate reviews", value)
        if raw.get("ok") is not True:
            raise ArgumentError("operations gate reviews must be successful")
        if raw.get("workflow") != "operations_gate_reviews":
            raise ArgumentError("operations gate reviews workflow is invalid")
        if raw.get("schema") != "bioprism-operations-gate-reviews/0.1":
            raise ArgumentError("operations gate reviews schema is invalid")
        review_id = raw.get("review_id")
        if review_id is not None:
            review_id = _review_id("operations gate reviews review_id", review_id)
        reviews_value = raw.get("reviews")
        if not isinstance(reviews_value, Sequence) or isinstance(reviews_value, (str, bytes)):
            raise ArgumentError("operations gate reviews reviews must be an array")
        reviews = tuple(OperationsGateReview.from_wire(item) for item in reviews_value)
        review_count = _non_negative("operations gate reviews review_count", raw.get("review_count"))
        if review_count != len(reviews) or raw.get("found") is not (len(reviews) > 0):
            raise ArgumentError("operations gate reviews count and found flag must reconcile")
        if raw.get("readiness_claimed") is not False:
            raise ArgumentError("operations gate reviews must not claim readiness")
        return cls(
            raw=raw,
            workflow="operations_gate_reviews",
            schema="bioprism-operations-gate-reviews/0.1",
            review_id=review_id,
            found=len(reviews) > 0,
            page=EventPage.from_wire(raw.get("page")),
            reviews=reviews,
            review_count=review_count,
            readiness_claimed=False,
            guarantees=_texts("operations gate reviews guarantees", raw.get("guarantees")),
            non_claims=_texts("operations gate reviews non_claims", raw.get("non_claims")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class OperationsSnapshot:
    """Typed bounded control-plane evidence assembled by the HTTP gateway."""

    raw: dict[str, Any]
    schema: str
    service: str
    api_version: str
    protocol_version: str
    after: int
    limit: int
    recent_events: EventPage
    event_metrics: dict[str, int]
    mission_summary: dict[str, Any]
    mission_persistence: MissionPersistenceStatus
    event_persistence: EventPersistenceStatus
    reconciliation_persistence: DomainWorkflowReconciliationPersistenceStatus
    reconciliation_summary: DomainWorkflowReconciliationSummaryReport
    recovery: RecoveryMatrix
    domain_coverage: OperationsDomainCoverage
    consistency: dict[str, Any]
    capabilities: dict[str, Any]
    operator_actions: tuple[str, ...]
    guarantees: tuple[str, ...]
    non_claims: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsSnapshot":
        raw = _mapping("operations snapshot", value)
        if raw.get("ok") is not True:
            raise ArgumentError("operations snapshot must be successful")
        schema = _text("operations snapshot schema", raw.get("schema"))
        if schema != "bioprism-operations-snapshot/0.1":
            raise ArgumentError("operations snapshot schema is invalid")
        after = _non_negative("operations snapshot after", raw.get("after"))
        limit = raw.get("limit")
        if (
            not isinstance(limit, int)
            or isinstance(limit, bool)
            or not 1 <= limit <= MAX_OPERATIONS_SNAPSHOT_LIMIT
        ):
            raise ArgumentError(
                f"operations snapshot limit must be between 1 and {MAX_OPERATIONS_SNAPSHOT_LIMIT}"
            )
        recent_events = EventPage.from_wire(raw.get("recent_events"))
        if recent_events.after != after:
            raise ArgumentError("operations snapshot after must match the event page")

        metrics_raw = _mapping("operations snapshot event_metrics", raw.get("event_metrics"))
        metric_names = (
            "retained_events",
            "dropped_events",
            "subscriptions",
            "active_subscriptions",
            "pending_deliveries",
            "dropped_deliveries",
            "next_event_id",
            "next_delivery_id",
            "retained_delivery_attempts",
            "dropped_delivery_attempts",
            "next_attempt_id",
        )
        event_metrics = {
            name: _non_negative(f"operations snapshot event_metrics {name}", metrics_raw.get(name))
            for name in metric_names
        }

        summary_raw = _mapping("operations snapshot mission_summary", raw.get("mission_summary"))
        total = _non_negative("operations snapshot mission total", summary_raw.get("total"))
        counts_raw = _mapping(
            "operations snapshot mission status_counts", summary_raw.get("status_counts")
        )
        status_counts = {
            _text("operations snapshot mission status", name): _non_negative(
                f"operations snapshot mission status_counts {name}", count
            )
            for name, count in counts_raw.items()
        }
        if sum(status_counts.values()) != total:
            raise ArgumentError("operations snapshot mission status counts must reconcile")
        mission_summary = dict(summary_raw)
        mission_summary["total"] = total
        mission_summary["status_counts"] = status_counts
        for name in ("recovered_after_restart", "cancel_requested", "registry_capacity"):
            mission_summary[name] = _non_negative(
                f"operations snapshot mission {name}", summary_raw.get(name)
            )
        if mission_summary["recovered_after_restart"] > total:
            raise ArgumentError("operations snapshot recovered mission count exceeds total")
        if mission_summary["cancel_requested"] > total:
            raise ArgumentError("operations snapshot cancellation count exceeds total")

        persistence = _mapping("operations snapshot persistence", raw.get("persistence"))
        mission_persistence = MissionPersistenceStatus.from_wire(persistence.get("missions"))
        event_persistence = EventPersistenceStatus.from_wire(persistence.get("events"))
        reconciliation_persistence = DomainWorkflowReconciliationPersistenceStatus.from_wire(
            persistence.get("workflow_reconciliations")
        )
        reconciliation_summary = DomainWorkflowReconciliationSummaryReport.from_wire(
            raw.get("reconciliation_summary")
        )
        recovery = RecoveryMatrix.from_wire(raw.get("recovery"))
        domain_coverage = OperationsDomainCoverage.from_wire(raw.get("domain_coverage"))
        consistency_raw = _mapping("operations snapshot consistency", raw.get("consistency"))
        consistency = dict(consistency_raw)
        if not isinstance(consistency.get("read_model"), str):
            raise ArgumentError("operations snapshot consistency read_model must be text")
        for name in (
            "cross_store_atomic",
            "event_cursor_authoritative",
            "clock_free",
            "underlying_routes_remain_authoritative",
        ):
            if not isinstance(consistency.get(name), bool):
                raise ArgumentError(f"operations snapshot consistency {name} must be a boolean")

        capabilities_raw = _mapping("operations snapshot capabilities", raw.get("capabilities"))
        capabilities = dict(capabilities_raw)
        for name in ("tool_count", "resource_count"):
            capabilities[name] = _non_negative(
                f"operations snapshot capabilities {name}", capabilities_raw.get(name)
            )
        for name in (
            "rest_tools",
            "json_rpc",
            "event_cursor",
            "async_missions",
            "mission_inventory",
            "operations_snapshot",
            "domain_coverage",
            "delivery_attempt_provenance",
            "external_delivery_worker",
        ):
            candidate = capabilities_raw.get(name)
            if not isinstance(candidate, bool):
                raise ArgumentError(f"operations snapshot capability {name} must be a boolean")

        return cls(
            raw=raw,
            schema=schema,
            service=_text("operations snapshot service", raw.get("service")),
            api_version=_text("operations snapshot api_version", raw.get("api_version")),
            protocol_version=_text(
                "operations snapshot protocol_version", raw.get("protocol_version")
            ),
            after=after,
            limit=limit,
            recent_events=recent_events,
            event_metrics=event_metrics,
            mission_summary=mission_summary,
            mission_persistence=mission_persistence,
            event_persistence=event_persistence,
            reconciliation_persistence=reconciliation_persistence,
            reconciliation_summary=reconciliation_summary,
            recovery=recovery,
            domain_coverage=domain_coverage,
            consistency=consistency,
            capabilities=capabilities,
            operator_actions=_texts(
                "operations snapshot operator_actions", raw.get("operator_actions")
            ),
            guarantees=_texts("operations snapshot guarantees", raw.get("guarantees")),
            non_claims=_texts("operations snapshot non_claims", raw.get("non_claims")),
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
class DeliveryAttempt:
    """One bounded, durable send/retry/replay/acknowledgement provenance row."""

    raw: dict[str, Any]
    attempt_id: int
    delivery_id: int
    subscription_id: str
    event_id: int
    event_type: str
    attempt: int
    action: str
    outcome: str
    receiver_accepted: bool | None
    retryable: bool | None
    error: str | None
    signature: str
    receipt_id: str | None
    receipt_digest: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryAttempt":
        raw = _mapping("delivery attempt", value)
        error = raw.get("error")
        if error is not None:
            error = _text("delivery attempt error", error)
        receipt_id = (
            validate_receipt_id(raw["receipt_id"])
            if raw.get("receipt_id") is not None
            else None
        )
        receipt_digest = _optional_digest(
            "delivery attempt receipt_digest", raw.get("receipt_digest")
        )
        if receipt_digest is not None and receipt_id is None:
            raise ArgumentError("delivery attempt receipt_digest requires receipt_id")
        return cls(
            raw=raw,
            attempt_id=_non_negative("delivery attempt attempt_id", raw.get("attempt_id")),
            delivery_id=_non_negative("delivery attempt delivery_id", raw.get("delivery_id")),
            subscription_id=_text(
                "delivery attempt subscription_id", raw.get("subscription_id")
            ),
            event_id=_non_negative("delivery attempt event_id", raw.get("event_id")),
            event_type=_text("delivery attempt event_type", raw.get("event_type")),
            attempt=_non_negative("delivery attempt attempt", raw.get("attempt")),
            action=_text("delivery attempt action", raw.get("action")),
            outcome=_text("delivery attempt outcome", raw.get("outcome")),
            receiver_accepted=_optional_bool(
                "delivery attempt receiver_accepted", raw.get("receiver_accepted")
            ),
            retryable=_optional_bool("delivery attempt retryable", raw.get("retryable")),
            error=error,
            signature=_text("delivery attempt signature", raw.get("signature")),
            receipt_id=receipt_id,
            receipt_digest=receipt_digest,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DeliveryAttemptPage:
    """Cursor page over durable delivery-attempt provenance."""

    raw: dict[str, Any]
    attempts: tuple[DeliveryAttempt, ...]
    after: int
    next_after: int
    oldest: int | None
    newest: int | None
    gap: bool
    dropped_attempts: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryAttemptPage":
        envelope = _mapping("delivery attempts response", value)
        page_value = envelope.get("page", envelope)
        raw = _mapping("delivery attempt page", page_value)
        values = raw.get("attempts")
        if not isinstance(values, Sequence) or isinstance(values, (str, bytes)):
            raise ArgumentError("delivery attempt page attempts must be an array")
        attempts = tuple(DeliveryAttempt.from_wire(item) for item in values)
        ids = tuple(attempt.attempt_id for attempt in attempts)
        if ids != tuple(sorted(set(ids))):
            raise ArgumentError("delivery attempt page ids must be sorted and unique")
        gap = raw.get("gap")
        if not isinstance(gap, bool):
            raise ArgumentError("delivery attempt page gap must be a boolean")
        oldest = raw.get("oldest")
        newest = raw.get("newest")
        if oldest is not None:
            oldest = _non_negative("delivery attempt page oldest", oldest)
        if newest is not None:
            newest = _non_negative("delivery attempt page newest", newest)
        return cls(
            raw=raw,
            attempts=attempts,
            after=_non_negative("delivery attempt page after", raw.get("after")),
            next_after=_non_negative(
                "delivery attempt page next_after", raw.get("next_after")
            ),
            oldest=oldest,
            newest=newest,
            gap=gap,
            dropped_attempts=_non_negative(
                "delivery attempt page dropped_attempts", raw.get("dropped_attempts")
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DeliveryReceiptAttempts:
    """Typed attempt provenance correlated to one content-addressed receipt."""

    raw: dict[str, Any]
    receipt_id: str
    found: bool
    page: DeliveryAttemptPage

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryReceiptAttempts":
        raw = _mapping("delivery receipt attempts", value)
        if raw.get("workflow") != "developer_delivery_receipt_attempts":
            raise ArgumentError("delivery receipt attempts workflow is invalid")
        receipt_id = validate_receipt_id(raw.get("receipt_id"))
        found = raw.get("found")
        if not isinstance(found, bool):
            raise ArgumentError("delivery receipt attempts found must be a boolean")
        page = DeliveryAttemptPage.from_wire(raw.get("page"))
        if found != bool(page.attempts):
            raise ArgumentError("delivery receipt attempts found must match page attempts")
        return cls(raw=raw, receipt_id=receipt_id, found=found, page=page)

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
    "OperationsDomainGroup",
    "OperationsDomainCoverage",
    "OperationsHandoffGroup",
    "OperationsHandoff",
    "OperationsDomainActivityGroup",
    "OperationsDomainActivity",
    "OperationsDomainGateGroup",
    "OperationsDomainGates",
    "MAX_OPERATIONS_SNAPSHOT_LIMIT",
    "MAX_OPERATIONS_DOMAIN_GROUPS",
    "MAX_OPERATIONS_DOMAIN_TOOLS",
    "OperationsSnapshot",
    "DeliveryView",
    "DeliveryPage",
    "DeliveryAttempt",
    "DeliveryAttemptPage",
    "DeliveryReceiptAttempts",
    "SseEvent",
    "SseSnapshot",
    "parse_sse",
]
