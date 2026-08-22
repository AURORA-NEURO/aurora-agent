"""Caller-owned connector registration and evidence dispatch for the autonomous brain.

The Rust gateway already defines provider-connector manifests and handoff evidence.  This module
supplies the missing application runtime around those contracts: a connector registry, exact
domain/capability routing, approval admission, and a transient execution value paired with a
metadata-only receipt.  The executor is always supplied by the embedding application, so this
layer never discovers a provider, accepts a raw key, or performs network I/O by itself.

An application may close over a short-lived ``CredentialHandle``/session in its executor.  The
runtime receives only the typed manifest and transient request, and the journal-compatible receipt
contains digests and identities—not the request, response, headers, or credential material.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
import os
from pathlib import Path
import threading
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .domain_evidence_source import (
    DomainEvidenceSourceExecutionRequest,
    DomainEvidenceSourcePlanRequest,
)
from .domain_evidence_provider_handoff import DomainEvidenceProviderConnectorManifest
from .domain_tools import (
    AUTONOMOUS_DOMAIN_NAMES,
    _identifier,
    _json_safe,
    _reject_secret_fields,
    _sequence,
)
from .errors import ArgumentError
from .http_client import ApiClient


AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA = "bioprism-python-autonomous-connector-registry/0.1"
AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA = "bioprism-python-autonomous-connector-dispatch/0.1"
AUTONOMOUS_CONNECTOR_RECEIPT_SCHEMA = "bioprism-python-autonomous-connector-receipt/0.1"
AUTONOMOUS_CONNECTOR_SELECTION_PLAN_SCHEMA = "bioprism-python-autonomous-connector-selection-plan/0.1"
AUTONOMOUS_CONNECTOR_SELECTION_ROW_SCHEMA = "bioprism-python-autonomous-connector-selection-row/0.1"
AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_SCHEMA = "bioprism-python-autonomous-connector-receipt-journal/0.1"
AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA = "bioprism-python-autonomous-connector-receipt-entry/0.1"
AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES = ("observed", "partial", "refused", "error", "unknown")
AUTONOMOUS_CONNECTOR_SELECTION_STRATEGIES = ("lexicographic_connector_id", "weighted_evidence")
MAX_AUTONOMOUS_CONNECTORS = 256
MAX_AUTONOMOUS_CONNECTOR_DOMAINS = len(AUTONOMOUS_DOMAIN_NAMES)
MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES = 2_000_000
MAX_AUTONOMOUS_CONNECTOR_RESULT_BYTES = 2_000_000
MAX_AUTONOMOUS_CONNECTOR_PARENT_DIGESTS = 128
MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_ENTRIES = 100_000
MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_BYTES = 50_000_000
MAX_AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_BYTES = 24_000
MAX_AUTONOMOUS_CONNECTOR_SELECTION_SIGNAL_BYTES = 64_000


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(
        character not in "0123456789abcdef" for character in value
    ):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _capability_identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > 256:
        raise ArgumentError(f"{name} must be a bounded capability identifier")
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+-" for character in value):
        raise ArgumentError(f"{name} must be a bounded capability identifier")
    return value


def _manifest_digest(manifest: DomainEvidenceProviderConnectorManifest) -> str:
    return content_digest(manifest.to_dict())


def _identifier_sequence(name: str, value: Any, *, maximum: int, allow_empty: bool = True) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be a sequence")
    if len(value) > maximum or (not allow_empty and not value):
        raise ArgumentError(f"{name} exceeds its bound or is empty")
    result: list[str] = []
    seen: set[str] = set()
    for item in value:
        normalized = _identifier(f"{name} entry", item)
        if normalized in seen:
            raise ArgumentError(f"{name} contains a duplicate entry: {normalized}")
        seen.add(normalized)
        result.append(normalized)
    return tuple(result)


def _bounded_selection_float(name: str, value: Any, *, minimum: float, maximum: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    normalized = float(value)
    if not minimum <= normalized <= maximum:
        raise ArgumentError(f"{name} must be between {minimum} and {maximum}")
    return normalized


def _selection_signal_descriptor(connector_id: str, value: Mapping[str, Any] | None) -> dict[str, Any]:
    connector_id = _identifier("autonomous connector selection signal connector_id", connector_id)
    if value is None:
        raw: Mapping[str, Any] = {}
    elif isinstance(value, Mapping):
        safe = _json_safe(
            "autonomous connector selection signal",
            dict(value),
            maximum=MAX_AUTONOMOUS_CONNECTOR_SELECTION_SIGNAL_BYTES,
        )
        _reject_secret_fields(safe)
        raw = safe
    else:
        raise ArgumentError("autonomous connector selection signal must be an object")
    allowed = {
        "eligible",
        "health",
        "success_rate",
        "evaluator_reward",
        "latency_ms",
        "cost_per_million_tokens",
    }
    if set(raw).difference(allowed):
        raise ArgumentError("autonomous connector selection signal contains unsupported fields")
    eligible = raw.get("eligible", True)
    if not isinstance(eligible, bool):
        raise ArgumentError("autonomous connector selection signal eligible must be a boolean")
    health = _bounded_selection_float(
        "autonomous connector selection signal health",
        raw.get("health", 0.5),
        minimum=0.0,
        maximum=1.0,
    )
    success_rate = _bounded_selection_float(
        "autonomous connector selection signal success_rate",
        raw.get("success_rate", health),
        minimum=0.0,
        maximum=1.0,
    )
    evaluator_reward = _bounded_selection_float(
        "autonomous connector selection signal evaluator_reward",
        raw.get("evaluator_reward", 0.0),
        minimum=-1.0,
        maximum=1.0,
    )
    latency = raw.get("latency_ms")
    if latency is not None:
        latency = _bounded_selection_float(
            "autonomous connector selection signal latency_ms",
            latency,
            minimum=0.0,
            maximum=86_400_000.0,
        )
    cost = raw.get("cost_per_million_tokens")
    if cost is not None:
        cost = _bounded_selection_float(
            "autonomous connector selection signal cost_per_million_tokens",
            cost,
            minimum=0.0,
            maximum=1_000_000.0,
        )
    latency_score = 0.5 if latency is None else 1.0 / (1.0 + latency / 1_000.0)
    cost_score = 0.5 if cost is None else 1.0 / (1.0 + cost / 100.0)
    score = (
        0.35 * health
        + 0.25 * success_rate
        + 0.25 * ((evaluator_reward + 1.0) / 2.0)
        + 0.10 * latency_score
        + 0.05 * cost_score
    )
    return {
        "connector_id": connector_id,
        "eligible": eligible,
        "health": health,
        "success_rate": success_rate,
        "evaluator_reward": evaluator_reward,
        "latency_ms": latency,
        "cost_per_million_tokens": cost,
        "score": score,
    }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorRegistration:
    """Redacted registration metadata plus a caller-owned transient executor."""

    manifest: DomainEvidenceProviderConnectorManifest
    executor: Callable[[DomainEvidenceProviderConnectorManifest, Mapping[str, Any]], Any]
    approval_required: bool = True

    def __post_init__(self) -> None:
        if not isinstance(self.manifest, DomainEvidenceProviderConnectorManifest):
            raise ArgumentError("autonomous connector registration requires a typed manifest")
        if not callable(self.executor):
            raise ArgumentError("autonomous connector registration executor must be callable")
        if not isinstance(self.approval_required, bool):
            raise ArgumentError("autonomous connector approval_required must be a boolean")
        domains = tuple(self.manifest.domains)
        if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in domains):
            raise ArgumentError("autonomous connector manifest contains an unsupported domain")

    @property
    def connector_id(self) -> str:
        return self.manifest.connector_id

    @property
    def manifest_digest(self) -> str:
        return _manifest_digest(self.manifest)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA,
            "manifest": self.manifest.to_dict(),
            "manifest_digest": self.manifest_digest,
            "approval_required": self.approval_required,
            "execution": "caller_owned_executor;metadata_only_registration",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorSelectionRow:
    """One deterministic, review-only connector choice for one autonomous domain."""

    domain: str
    status: str
    connector_id: str | None
    manifest_digest: str | None
    candidate_ids: tuple[str, ...]
    candidate_manifest_digests: tuple[str, ...]
    reason: str
    candidate_scores: tuple[float, ...] = ()
    candidate_eligible: tuple[bool, ...] = ()

    def __post_init__(self) -> None:
        _identifier("autonomous connector selection row domain", self.domain)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("autonomous connector selection row domain is unsupported")
        if self.status not in {"selected", "missing"}:
            raise ArgumentError("autonomous connector selection row status is invalid")
        if self.connector_id is not None:
            _identifier("autonomous connector selection row connector_id", self.connector_id)
        if self.manifest_digest is not None:
            _digest("autonomous connector selection row manifest_digest", self.manifest_digest)
        candidate_ids = _identifier_sequence(
            "autonomous connector selection row candidate_ids",
            self.candidate_ids,
            maximum=MAX_AUTONOMOUS_CONNECTORS,
        )
        candidate_digests = tuple(
            _digest("autonomous connector selection row candidate manifest digest", digest)
            for digest in self.candidate_manifest_digests
        )
        if len(candidate_ids) != len(candidate_digests):
            raise ArgumentError("autonomous connector selection row candidates and digests must align")
        if self.candidate_scores:
            candidate_scores = tuple(
                _bounded_selection_float(
                    "autonomous connector selection row candidate score",
                    score,
                    minimum=0.0,
                    maximum=1.0,
                )
                for score in self.candidate_scores
            )
            if len(candidate_scores) != len(candidate_ids):
                raise ArgumentError("autonomous connector selection row scores must align with candidates")
        else:
            candidate_scores = tuple(0.0 for _ in candidate_ids)
        if self.candidate_eligible:
            if any(not isinstance(eligible, bool) for eligible in self.candidate_eligible):
                raise ArgumentError("autonomous connector selection row eligibility must be boolean")
            candidate_eligible = tuple(self.candidate_eligible)
            if len(candidate_eligible) != len(candidate_ids):
                raise ArgumentError("autonomous connector selection row eligibility must align with candidates")
        else:
            candidate_eligible = tuple(True for _ in candidate_ids)
        if self.status == "selected":
            if self.connector_id is None or self.manifest_digest is None:
                raise ArgumentError("selected connector row requires connector and manifest identities")
            if self.connector_id not in candidate_ids:
                raise ArgumentError("selected connector row must select a candidate")
            selected_index = candidate_ids.index(self.connector_id)
            if not candidate_eligible[selected_index]:
                raise ArgumentError("selected connector row cannot select an ineligible candidate")
            if candidate_digests[selected_index] != self.manifest_digest:
                raise ArgumentError("selected connector row manifest digest does not match its candidate")
        elif self.connector_id is not None or self.manifest_digest is not None:
            raise ArgumentError("missing connector row cannot select a connector")
        _identifier("autonomous connector selection row reason", self.reason)
        object.__setattr__(self, "candidate_ids", candidate_ids)
        object.__setattr__(self, "candidate_manifest_digests", candidate_digests)
        object.__setattr__(self, "candidate_scores", candidate_scores)
        object.__setattr__(self, "candidate_eligible", candidate_eligible)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_SELECTION_ROW_SCHEMA,
            "domain": self.domain,
            "status": self.status,
            "connector_id": self.connector_id,
            "manifest_digest": self.manifest_digest,
            "candidate_ids": list(self.candidate_ids),
            "candidate_manifest_digests": list(self.candidate_manifest_digests),
            "candidate_scores": list(self.candidate_scores),
            "candidate_eligible": list(self.candidate_eligible),
            "reason": self.reason,
            "retention": "metadata_only_manifest_catalogue",
            "secret_material": "never_returned",
        }


def _selection_row_from_mapping(value: Mapping[str, Any]) -> AutonomousConnectorSelectionRow:
    expected = {
        "schema",
        "domain",
        "status",
        "connector_id",
        "manifest_digest",
        "candidate_ids",
        "candidate_manifest_digests",
        "candidate_scores",
        "candidate_eligible",
        "reason",
        "retention",
        "secret_material",
    }
    if not isinstance(value, Mapping) or set(value) != expected:
        raise ArgumentError("autonomous connector selection row is malformed")
    if value.get("schema") != AUTONOMOUS_CONNECTOR_SELECTION_ROW_SCHEMA:
        raise ArgumentError("autonomous connector selection row schema is invalid")
    if value.get("retention") != "metadata_only_manifest_catalogue" or value.get("secret_material") != "never_returned":
        raise ArgumentError("autonomous connector selection row retention is invalid")
    raw_digests = value.get("candidate_manifest_digests")
    if not isinstance(raw_digests, Sequence) or isinstance(raw_digests, (str, bytes)):
        raise ArgumentError("autonomous connector selection row candidate digests are invalid")
    raw_scores = value.get("candidate_scores")
    raw_eligible = value.get("candidate_eligible")
    if not isinstance(raw_scores, Sequence) or isinstance(raw_scores, (str, bytes)):
        raise ArgumentError("autonomous connector selection row candidate scores are invalid")
    if not isinstance(raw_eligible, Sequence) or isinstance(raw_eligible, (str, bytes)):
        raise ArgumentError("autonomous connector selection row candidate eligibility is invalid")
    return AutonomousConnectorSelectionRow(
        domain=value.get("domain"),
        status=value.get("status"),
        connector_id=value.get("connector_id"),
        manifest_digest=value.get("manifest_digest"),
        candidate_ids=value.get("candidate_ids"),
        candidate_manifest_digests=tuple(raw_digests),
        reason=value.get("reason"),
        candidate_scores=tuple(raw_scores),
        candidate_eligible=tuple(raw_eligible),
    )


@dataclass(frozen=True, slots=True)
class AutonomousConnectorSelectionPlan:
    """Digest-bound, deterministic connector selection that never dispatches by itself."""

    domains: tuple[str, ...]
    capability: str | None
    registry_digest: str
    rows: tuple[AutonomousConnectorSelectionRow, ...]
    strategy: str = "lexicographic_connector_id"
    signal_digest: str | None = None

    def __post_init__(self) -> None:
        domains = _sequence(
            "autonomous connector selection plan domains",
            self.domains,
            maximum=MAX_AUTONOMOUS_CONNECTOR_DOMAINS,
        )
        if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in domains):
            raise ArgumentError("autonomous connector selection plan contains an unsupported domain")
        if self.capability is not None:
            _capability_identifier("autonomous connector selection plan capability", self.capability)
        _digest("autonomous connector selection plan registry_digest", self.registry_digest)
        if self.strategy not in AUTONOMOUS_CONNECTOR_SELECTION_STRATEGIES:
            raise ArgumentError("autonomous connector selection plan strategy is invalid")
        _digest("autonomous connector selection plan signal_digest", self.signal_digest, allow_none=True)
        if not isinstance(self.rows, Sequence) or isinstance(self.rows, (str, bytes)) or len(self.rows) != len(domains):
            raise ArgumentError("autonomous connector selection plan rows must align with domains")
        if any(not isinstance(row, AutonomousConnectorSelectionRow) for row in self.rows):
            raise ArgumentError("autonomous connector selection plan rows must be typed")
        if tuple(row.domain for row in self.rows) != domains:
            raise ArgumentError("autonomous connector selection plan row domains are out of order")
        object.__setattr__(self, "domains", domains)
        object.__setattr__(self, "rows", tuple(self.rows))

    @property
    def complete(self) -> bool:
        return all(row.status == "selected" for row in self.rows)

    @property
    def plan_digest(self) -> str:
        return content_digest(self._payload())

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_SELECTION_PLAN_SCHEMA,
            "domains": list(self.domains),
            "capability": self.capability,
            "registry_digest": self.registry_digest,
            "rows": [row.to_dict() for row in self.rows],
            "strategy": self.strategy,
            "signal_digest": self.signal_digest,
        }

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "complete": self.complete,
            "plan_digest": self.plan_digest,
            "execution": "planning_only;review_required_before_dispatch",
            "retention": "metadata_only_manifest_catalogue",
            "secret_material": "never_returned",
        }

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousConnectorSelectionPlan":
        expected = {
            "schema",
            "domains",
            "capability",
            "registry_digest",
            "rows",
            "strategy",
            "signal_digest",
            "complete",
            "plan_digest",
            "execution",
            "retention",
            "secret_material",
        }
        if not isinstance(value, Mapping) or set(value) != expected:
            raise ArgumentError("autonomous connector selection plan is malformed")
        if value.get("schema") != AUTONOMOUS_CONNECTOR_SELECTION_PLAN_SCHEMA:
            raise ArgumentError("autonomous connector selection plan schema is invalid")
        if value.get("execution") != "planning_only;review_required_before_dispatch":
            raise ArgumentError("autonomous connector selection plan execution posture is invalid")
        if value.get("retention") != "metadata_only_manifest_catalogue" or value.get("secret_material") != "never_returned":
            raise ArgumentError("autonomous connector selection plan retention is invalid")
        raw_rows = value.get("rows")
        if not isinstance(raw_rows, Sequence) or isinstance(raw_rows, (str, bytes)):
            raise ArgumentError("autonomous connector selection plan rows are invalid")
        plan = cls(
            domains=value.get("domains"),
            capability=value.get("capability"),
            registry_digest=value.get("registry_digest"),
            rows=tuple(_selection_row_from_mapping(row) for row in raw_rows),
            strategy=value.get("strategy"),
            signal_digest=value.get("signal_digest"),
        )
        if value.get("complete") is not plan.complete:
            raise ArgumentError("autonomous connector selection plan completeness is invalid")
        if _digest("autonomous connector selection plan plan_digest", value.get("plan_digest")) != plan.plan_digest:
            raise ArgumentError("autonomous connector selection plan digest is invalid")
        return plan

    def verify(self, registry: "AutonomousConnectorRegistry") -> "AutonomousConnectorSelectionPlan":
        if not isinstance(registry, AutonomousConnectorRegistry):
            raise ArgumentError("autonomous connector selection plan verification requires a registry")
        if registry.digest != self.registry_digest:
            raise ArgumentError("autonomous connector selection plan is stale or tampered")
        for row in self.rows:
            candidates = tuple(
                registration
                for registration in registry.registrations()
                if row.domain in registration.manifest.domains
                and (self.capability is None or self.capability in registration.manifest.capabilities)
            )
            if tuple(item.connector_id for item in candidates) != row.candidate_ids:
                raise ArgumentError("autonomous connector selection plan candidate set changed")
            if tuple(item.manifest_digest for item in candidates) != row.candidate_manifest_digests:
                raise ArgumentError("autonomous connector selection plan manifest set changed")
        return self


class AutonomousConnectorRegistry:
    """Exact connector catalogue; registration never authorizes or dispatches a connector."""

    def __init__(self, registrations: Sequence[AutonomousConnectorRegistration] = ()) -> None:
        if not isinstance(registrations, Sequence) or isinstance(registrations, (str, bytes)):
            raise ArgumentError("autonomous connector registrations must be a sequence")
        self._connectors: dict[str, AutonomousConnectorRegistration] = {}
        for registration in registrations:
            self.register(registration, replace=False)

    def register(
        self,
        registration: AutonomousConnectorRegistration,
        *,
        replace: bool = False,
    ) -> AutonomousConnectorRegistration:
        if not isinstance(registration, AutonomousConnectorRegistration):
            raise ArgumentError("autonomous connector registration is invalid")
        if not isinstance(replace, bool):
            raise ArgumentError("autonomous connector replace must be a boolean")
        connector_id = _identifier("autonomous connector id", registration.connector_id)
        if connector_id in self._connectors and not replace:
            raise ArgumentError("autonomous connector is already registered")
        if connector_id not in self._connectors and len(self._connectors) >= MAX_AUTONOMOUS_CONNECTORS:
            raise ArgumentError("autonomous connector registry capacity is exhausted")
        self._connectors[connector_id] = registration
        return registration

    def resolve(self, connector_id: str) -> AutonomousConnectorRegistration:
        connector_id = _identifier("autonomous connector id", connector_id)
        registration = self._connectors.get(connector_id)
        if registration is None:
            raise ArgumentError("autonomous connector is not registered")
        return registration

    def registrations(self) -> tuple[AutonomousConnectorRegistration, ...]:
        return tuple(self._connectors[name] for name in sorted(self._connectors))

    def plan_for_domains(
        self,
        domains: Sequence[str],
        *,
        capability: str | None = None,
    ) -> dict[str, Any]:
        requested = _sequence("autonomous connector plan domains", domains, maximum=MAX_AUTONOMOUS_CONNECTOR_DOMAINS)
        if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in requested):
            raise ArgumentError("autonomous connector plan contains an unsupported domain")
        if capability is not None:
            capability = _capability_identifier("autonomous connector plan capability", capability)
        coverage: dict[str, dict[str, Any]] = {}
        for domain in requested:
            candidates = [
                registration
                for registration in self.registrations()
                if domain in registration.manifest.domains
                and (capability is None or capability in registration.manifest.capabilities)
            ]
            coverage[domain] = {
                "status": "selected" if candidates else "missing",
                "connector_ids": [item.connector_id for item in candidates],
                "manifest_digests": [item.manifest_digest for item in candidates],
                "capability": capability,
            }
        payload = {
            "schema": AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA,
            "domains": list(requested),
            "capability": capability,
            "coverage": coverage,
            "registry_digest": self.digest,
            "selection_plan_digest": self.select_for_domains(requested, capability=capability).plan_digest,
            "execution": "planning_only;no_dispatch;no_authorization",
            "secret_material": "never_returned",
        }
        payload["plan_digest"] = content_digest(payload)
        return payload

    def select_for_domains(
        self,
        domains: Sequence[str],
        *,
        capability: str | None = None,
        strategy: str = "lexicographic_connector_id",
        selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
    ) -> AutonomousConnectorSelectionPlan:
        requested = _sequence(
            "autonomous connector selection domains",
            domains,
            maximum=MAX_AUTONOMOUS_CONNECTOR_DOMAINS,
        )
        if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in requested):
            raise ArgumentError("autonomous connector selection contains an unsupported domain")
        if capability is not None:
            capability = _capability_identifier("autonomous connector selection capability", capability)
        if strategy not in AUTONOMOUS_CONNECTOR_SELECTION_STRATEGIES:
            raise ArgumentError("autonomous connector selection strategy is invalid")
        if strategy == "lexicographic_connector_id" and selection_signals is not None:
            raise ArgumentError("lexicographic connector selection cannot consume selection signals")
        normalized_signals: dict[str, dict[str, Any]] = {}
        if selection_signals is not None:
            if not isinstance(selection_signals, Mapping):
                raise ArgumentError("autonomous connector selection signals must be an object")
            for connector_id, raw_signal in selection_signals.items():
                normalized_id = _identifier("autonomous connector selection signal connector_id", connector_id)
                if normalized_id not in self._connectors:
                    raise ArgumentError("autonomous connector selection signal names an unknown connector")
                normalized_signals[normalized_id] = _selection_signal_descriptor(normalized_id, raw_signal)
        if strategy == "weighted_evidence" and selection_signals is None:
            raise ArgumentError("weighted connector selection requires selection_signals")
        signal_digest = (
            content_digest([normalized_signals[key] for key in sorted(normalized_signals)])
            if strategy == "weighted_evidence"
            else None
        )
        rows: list[AutonomousConnectorSelectionRow] = []
        for domain in requested:
            candidates = [
                registration
                for registration in self.registrations()
                if domain in registration.manifest.domains
                and (capability is None or capability in registration.manifest.capabilities)
            ]
            candidate_ids = tuple(item.connector_id for item in candidates)
            candidate_digests = tuple(item.manifest_digest for item in candidates)
            descriptors = tuple(
                normalized_signals.get(connector_id)
                or _selection_signal_descriptor(connector_id, None)
                for connector_id in candidate_ids
            )
            candidate_scores = tuple(
                descriptor["score"] if strategy == "weighted_evidence" else 0.0
                for descriptor in descriptors
            )
            candidate_eligible = tuple(
                descriptor["eligible"] if strategy == "weighted_evidence" else True
                for descriptor in descriptors
            )
            eligible_indexes = [index for index, eligible in enumerate(candidate_eligible) if eligible]
            if strategy == "weighted_evidence":
                selected_index = (
                    sorted(eligible_indexes, key=lambda index: (-candidate_scores[index], candidate_ids[index]))[0]
                    if eligible_indexes
                    else None
                )
            else:
                selected_index = eligible_indexes[0] if eligible_indexes else None
            selected = None if selected_index is None else candidates[selected_index]
            rows.append(
                AutonomousConnectorSelectionRow(
                    domain=domain,
                    status="selected" if selected is not None else "missing",
                    connector_id=None if selected is None else selected.connector_id,
                    manifest_digest=None if selected is None else selected.manifest_digest,
                    candidate_ids=candidate_ids,
                    candidate_manifest_digests=candidate_digests,
                    reason=(
                        strategy
                        if selected is not None
                        else ("no_matching_connector" if not candidates else "no_eligible_connector")
                    ),
                    candidate_scores=candidate_scores,
                    candidate_eligible=candidate_eligible,
                )
            )
        return AutonomousConnectorSelectionPlan(
            domains=requested,
            capability=capability,
            registry_digest=self.digest,
            rows=tuple(rows),
            strategy=strategy,
            signal_digest=signal_digest,
        )

    def select_adaptive_for_domains(
        self,
        domains: Sequence[str],
        *,
        capability: str,
        selection_signals: Mapping[str, Mapping[str, Any]],
    ) -> AutonomousConnectorSelectionPlan:
        """Select connectors using explicit caller/evaluator evidence with deterministic ties."""

        return self.select_for_domains(
            domains,
            capability=capability,
            strategy="weighted_evidence",
            selection_signals=selection_signals,
        )

    @property
    def digest(self) -> str:
        return content_digest([registration.to_dict() for registration in self.registrations()])

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA,
            "digest": self.digest,
            "connectors": [registration.to_dict() for registration in self.registrations()],
            "connector_count": len(self._connectors),
            "execution": "metadata_only;registration_is_not_authorization",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorDispatchRequest:
    """Transient connector input with a digest-only public projection."""

    dispatch_id: str
    execution_id: str
    call_id: str
    connector_id: str
    domains: tuple[str, ...]
    capability: str
    request: Mapping[str, Any]
    parent_digests: tuple[str, ...] = ()
    attempt_id: str | None = None
    selection_plan_digest: str | None = None
    approved: bool = False

    def __post_init__(self) -> None:
        for name, value in (
            ("dispatch_id", self.dispatch_id),
            ("execution_id", self.execution_id),
            ("call_id", self.call_id),
            ("connector_id", self.connector_id),
        ):
            _identifier(f"autonomous connector dispatch {name}", value)
        _capability_identifier("autonomous connector dispatch capability", self.capability)
        domains = _sequence(
            "autonomous connector dispatch domains",
            self.domains,
            maximum=MAX_AUTONOMOUS_CONNECTOR_DOMAINS,
        )
        if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in domains):
            raise ArgumentError("autonomous connector dispatch contains an unsupported domain")
        if not isinstance(self.request, Mapping):
            raise ArgumentError("autonomous connector dispatch request must be an object")
        safe_request = _json_safe(
            "autonomous connector dispatch request",
            dict(self.request),
            maximum=MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES,
        )
        _reject_secret_fields(safe_request)
        object.__setattr__(self, "domains", domains)
        object.__setattr__(self, "request", safe_request)
        if len(self.parent_digests) > MAX_AUTONOMOUS_CONNECTOR_PARENT_DIGESTS:
            raise ArgumentError("autonomous connector dispatch parent_digests exceeds its bound")
        for digest in self.parent_digests:
            _digest("autonomous connector dispatch parent digest", digest)
        if self.attempt_id is not None:
            _identifier("autonomous connector dispatch attempt_id", self.attempt_id)
        if self.selection_plan_digest is not None:
            _digest("autonomous connector dispatch selection_plan_digest", self.selection_plan_digest)
        if not isinstance(self.approved, bool):
            raise ArgumentError("autonomous connector dispatch approved must be a boolean")

    @property
    def request_digest(self) -> str:
        return content_digest(
            {
                "schema": AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA,
                "dispatch_id": self.dispatch_id,
                "execution_id": self.execution_id,
                "call_id": self.call_id,
                "connector_id": self.connector_id,
                "domains": list(self.domains),
                "capability": self.capability,
                "request": dict(self.request),
                "parent_digests": list(self.parent_digests),
                "attempt_id": self.attempt_id,
                "selection_plan_digest": self.selection_plan_digest,
            }
        )

    def to_metadata(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA,
            "dispatch_id": self.dispatch_id,
            "execution_id": self.execution_id,
            "call_id": self.call_id,
            "connector_id": self.connector_id,
            "domains": list(self.domains),
            "capability": self.capability,
            "request_digest": self.request_digest,
            "parent_digests": list(self.parent_digests),
            "attempt_id": self.attempt_id,
            "selection_plan_digest": self.selection_plan_digest,
            "approved": self.approved,
            "retention": "metadata_only_request_not_returned",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorObservation:
    """Caller-owned transient result classification; ``value`` is never retained in receipts."""

    value: Any = None
    status: str = "observed"
    failure_class: str | None = None

    def __post_init__(self) -> None:
        if self.status not in AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES:
            raise ArgumentError("autonomous connector observation status is invalid")
        if self.failure_class is not None:
            _identifier("autonomous connector observation failure_class", self.failure_class)
        safe_value = _json_safe(
            "autonomous connector observation value",
            self.value,
            maximum=MAX_AUTONOMOUS_CONNECTOR_RESULT_BYTES,
        )
        _reject_secret_fields(safe_value)
        object.__setattr__(self, "value", safe_value)


@dataclass(frozen=True, slots=True)
class AutonomousConnectorDispatchReceipt:
    """Metadata-only outcome for one connector attempt."""

    dispatch_id: str
    execution_id: str
    call_id: str
    connector_id: str
    connector_version: str
    provider: str
    connector_kind: str
    manifest_digest: str
    domains: tuple[str, ...]
    capability: str
    status: str
    request_digest: str
    payload_digest: str | None = None
    parent_digests: tuple[str, ...] = ()
    attempt_id: str | None = None
    failure_class: str | None = None

    def __post_init__(self) -> None:
        for name, value in (
            ("dispatch_id", self.dispatch_id),
            ("execution_id", self.execution_id),
            ("call_id", self.call_id),
            ("connector_id", self.connector_id),
            ("connector_version", self.connector_version),
            ("provider", self.provider),
            ("connector_kind", self.connector_kind),
        ):
            _identifier(f"autonomous connector receipt {name}", value)
        _capability_identifier("autonomous connector receipt capability", self.capability)
        _digest("autonomous connector receipt manifest_digest", self.manifest_digest)
        _digest("autonomous connector receipt request_digest", self.request_digest)
        _digest("autonomous connector receipt payload_digest", self.payload_digest, allow_none=True)
        if self.status not in AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES:
            raise ArgumentError("autonomous connector receipt status is invalid")
        domains = _sequence("autonomous connector receipt domains", self.domains, maximum=MAX_AUTONOMOUS_CONNECTOR_DOMAINS)
        if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in domains):
            raise ArgumentError("autonomous connector receipt contains an unsupported domain")
        object.__setattr__(self, "domains", domains)
        for digest in self.parent_digests:
            _digest("autonomous connector receipt parent digest", digest)
        if len(self.parent_digests) > MAX_AUTONOMOUS_CONNECTOR_PARENT_DIGESTS:
            raise ArgumentError("autonomous connector receipt parent_digests exceeds its bound")
        if self.attempt_id is not None:
            _identifier("autonomous connector receipt attempt_id", self.attempt_id)
        if self.failure_class is not None:
            _identifier("autonomous connector receipt failure_class", self.failure_class)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_RECEIPT_SCHEMA,
            "dispatch_id": self.dispatch_id,
            "execution_id": self.execution_id,
            "call_id": self.call_id,
            "connector_id": self.connector_id,
            "connector_version": self.connector_version,
            "provider": self.provider,
            "connector_kind": self.connector_kind,
            "manifest_digest": self.manifest_digest,
            "domains": list(self.domains),
            "capability": self.capability,
            "status": self.status,
            "request_digest": self.request_digest,
            "payload_digest": self.payload_digest,
            "parent_digests": list(self.parent_digests),
            "attempt_id": self.attempt_id,
            "failure_class": self.failure_class,
            "retention": "metadata_only_no_request_or_payload",
            "secret_material": "never_returned",
        }


def _connector_receipt_from_mapping(
    value: AutonomousConnectorDispatchReceipt | Mapping[str, Any],
) -> AutonomousConnectorDispatchReceipt:
    if isinstance(value, AutonomousConnectorDispatchReceipt):
        return AutonomousConnectorDispatchReceipt(
            dispatch_id=value.dispatch_id,
            execution_id=value.execution_id,
            call_id=value.call_id,
            connector_id=value.connector_id,
            connector_version=value.connector_version,
            provider=value.provider,
            connector_kind=value.connector_kind,
            manifest_digest=value.manifest_digest,
            domains=value.domains,
            capability=value.capability,
            status=value.status,
            request_digest=value.request_digest,
            payload_digest=value.payload_digest,
            parent_digests=value.parent_digests,
            attempt_id=value.attempt_id,
            failure_class=value.failure_class,
        )
    if not isinstance(value, Mapping):
        raise ArgumentError("autonomous connector receipt must be a mapping or typed receipt")
    expected = {
        "schema",
        "dispatch_id",
        "execution_id",
        "call_id",
        "connector_id",
        "connector_version",
        "provider",
        "connector_kind",
        "manifest_digest",
        "domains",
        "capability",
        "status",
        "request_digest",
        "payload_digest",
        "parent_digests",
        "attempt_id",
        "failure_class",
        "retention",
        "secret_material",
    }
    if set(value) != expected:
        raise ArgumentError("autonomous connector receipt contains unsupported or missing fields")
    if value.get("schema") != AUTONOMOUS_CONNECTOR_RECEIPT_SCHEMA:
        raise ArgumentError("autonomous connector receipt schema is invalid")
    if value.get("retention") != "metadata_only_no_request_or_payload" or value.get("secret_material") != "never_returned":
        raise ArgumentError("autonomous connector receipt retention is invalid")
    domains = value.get("domains")
    parents = value.get("parent_digests")
    if not isinstance(domains, Sequence) or isinstance(domains, (str, bytes)):
        raise ArgumentError("autonomous connector receipt domains are invalid")
    if not isinstance(parents, Sequence) or isinstance(parents, (str, bytes)):
        raise ArgumentError("autonomous connector receipt parent_digests are invalid")
    return AutonomousConnectorDispatchReceipt(
        dispatch_id=value.get("dispatch_id"),
        execution_id=value.get("execution_id"),
        call_id=value.get("call_id"),
        connector_id=value.get("connector_id"),
        connector_version=value.get("connector_version"),
        provider=value.get("provider"),
        connector_kind=value.get("connector_kind"),
        manifest_digest=value.get("manifest_digest"),
        domains=tuple(domains),
        capability=value.get("capability"),
        status=value.get("status"),
        request_digest=value.get("request_digest"),
        payload_digest=value.get("payload_digest"),
        parent_digests=tuple(parents),
        attempt_id=value.get("attempt_id"),
        failure_class=value.get("failure_class"),
    )


def _connector_receipt_identity_digest(receipt: AutonomousConnectorDispatchReceipt) -> str:
    return content_digest(
        {
            "schema": AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA,
            "execution_id": receipt.execution_id,
            "dispatch_id": receipt.dispatch_id,
            "call_id": receipt.call_id,
            "connector_id": receipt.connector_id,
            "attempt_id": receipt.attempt_id,
        }
    )


def _connector_dispatch_identity_digest(request: AutonomousConnectorDispatchRequest) -> str:
    return content_digest(
        {
            "schema": AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA,
            "execution_id": request.execution_id,
            "dispatch_id": request.dispatch_id,
            "call_id": request.call_id,
            "connector_id": request.connector_id,
            "attempt_id": request.attempt_id,
        }
    )


@dataclass(frozen=True, slots=True)
class AutonomousConnectorReceiptJournalEntry:
    """One validated hash-chain row for a connector dispatch receipt."""

    sequence: int
    previous_entry_digest: str | None
    receipt: AutonomousConnectorDispatchReceipt
    receipt_identity_digest: str
    entry_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA,
            "sequence": self.sequence,
            "previous_entry_digest": self.previous_entry_digest,
            "receipt": self.receipt.to_dict(),
            "receipt_identity_digest": self.receipt_identity_digest,
            "entry_digest": self.entry_digest,
            "retention": "metadata_only_hash_chained_no_request_or_payload",
            "secret_material": "never_returned",
        }


def _connector_entry_from_mapping(value: Mapping[str, Any]) -> AutonomousConnectorReceiptJournalEntry:
    expected = {
        "schema",
        "sequence",
        "previous_entry_digest",
        "receipt",
        "receipt_identity_digest",
        "entry_digest",
        "retention",
        "secret_material",
    }
    if not isinstance(value, Mapping) or set(value) != expected:
        raise ArgumentError("autonomous connector receipt journal entry is malformed")
    if value.get("schema") != AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA:
        raise ArgumentError("autonomous connector receipt journal entry schema is invalid")
    if value.get("retention") != "metadata_only_hash_chained_no_request_or_payload" or value.get("secret_material") != "never_returned":
        raise ArgumentError("autonomous connector receipt journal entry retention is invalid")
    sequence = value.get("sequence")
    if not isinstance(sequence, int) or isinstance(sequence, bool) or not 1 <= sequence <= MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_ENTRIES:
        raise ArgumentError("autonomous connector receipt journal sequence is outside its bound")
    previous = _digest(
        "autonomous connector receipt journal previous_entry_digest",
        value.get("previous_entry_digest"),
        allow_none=True,
    )
    raw_receipt = value.get("receipt")
    if not isinstance(raw_receipt, Mapping):
        raise ArgumentError("autonomous connector receipt journal receipt is invalid")
    receipt = _connector_receipt_from_mapping(raw_receipt)
    identity = _digest(
        "autonomous connector receipt journal receipt_identity_digest",
        value.get("receipt_identity_digest"),
    )
    if identity != _connector_receipt_identity_digest(receipt):
        raise ArgumentError("autonomous connector receipt journal identity digest is invalid")
    entry_digest = _digest("autonomous connector receipt journal entry_digest", value.get("entry_digest"))
    descriptor = {
        "schema": AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA,
        "sequence": sequence,
        "previous_entry_digest": previous,
        "receipt": receipt.to_dict(),
        "receipt_identity_digest": identity,
        "retention": "metadata_only_hash_chained_no_request_or_payload",
        "secret_material": "never_returned",
    }
    if entry_digest != content_digest(descriptor):
        raise ArgumentError("autonomous connector receipt journal entry digest is invalid")
    return AutonomousConnectorReceiptJournalEntry(sequence, previous, receipt, identity, entry_digest)


class AutonomousConnectorReceiptJournal:
    """Bounded JSONL store that can be passed as a connector runtime receipt store."""

    def __init__(
        self,
        path: str | os.PathLike[str],
        *,
        max_entries: int = MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_ENTRIES,
        max_bytes: int = MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_BYTES,
    ) -> None:
        if not isinstance(path, (str, os.PathLike)) or not str(path):
            raise ArgumentError("autonomous connector receipt journal path must be non-empty")
        if isinstance(max_entries, bool) or not isinstance(max_entries, int) or not 1 <= max_entries <= MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_ENTRIES:
            raise ArgumentError("autonomous connector receipt journal max_entries is outside its bound")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_BYTES:
            raise ArgumentError("autonomous connector receipt journal max_bytes is outside its bound")
        self.path = Path(path)
        self.max_entries = max_entries
        self.max_bytes = max_bytes
        self._lock = threading.RLock()
        with self._lock:
            self._read_rows_locked()

    def append(
        self,
        receipt: AutonomousConnectorDispatchReceipt | Mapping[str, Any],
    ) -> AutonomousConnectorReceiptJournalEntry:
        normalized = _connector_receipt_from_mapping(receipt)
        identity = _connector_receipt_identity_digest(normalized)
        with self._lock:
            rows = self._read_rows_locked()
            existing = next((row for row in rows if row.receipt_identity_digest == identity), None)
            if existing is not None:
                if content_digest(existing.receipt.to_dict()) == content_digest(normalized.to_dict()):
                    return existing
                raise ArgumentError("autonomous connector receipt journal identity conflict")
            if len(rows) >= self.max_entries:
                raise ArgumentError("autonomous connector receipt journal entry capacity is exhausted")
            descriptor = {
                "schema": AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA,
                "sequence": len(rows) + 1,
                "previous_entry_digest": rows[-1].entry_digest if rows else None,
                "receipt": normalized.to_dict(),
                "receipt_identity_digest": identity,
                "retention": "metadata_only_hash_chained_no_request_or_payload",
                "secret_material": "never_returned",
            }
            entry = AutonomousConnectorReceiptJournalEntry(
                descriptor["sequence"],
                descriptor["previous_entry_digest"],
                normalized,
                identity,
                content_digest(descriptor),
            )
            line = json.dumps(entry.to_dict(), ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8") + b"\n"
            if len(line) > MAX_AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_BYTES:
                raise ArgumentError("autonomous connector receipt journal entry exceeds its byte bound")
            current_size = self.path.stat().st_size if self.path.exists() else 0
            if current_size + len(line) > self.max_bytes:
                raise ArgumentError("autonomous connector receipt journal byte capacity is exhausted")
            self.path.parent.mkdir(parents=True, exist_ok=True)
            with self.path.open("ab") as handle:
                handle.write(line)
                handle.flush()
                os.fsync(handle.fileno())
            return entry

    def find(
        self,
        *,
        execution_id: str,
        dispatch_id: str,
        call_id: str,
        connector_id: str,
        attempt_id: str | None = None,
    ) -> AutonomousConnectorDispatchReceipt | None:
        identity = content_digest(
            {
                "schema": AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA,
                "execution_id": _identifier("autonomous connector journal execution_id", execution_id),
                "dispatch_id": _identifier("autonomous connector journal dispatch_id", dispatch_id),
                "call_id": _identifier("autonomous connector journal call_id", call_id),
                "connector_id": _identifier("autonomous connector journal connector_id", connector_id),
                "attempt_id": None if attempt_id is None else _identifier("autonomous connector journal attempt_id", attempt_id),
            }
        )
        with self._lock:
            for row in reversed(self._read_rows_locked()):
                if row.receipt_identity_digest == identity:
                    return row.receipt
        return None

    def receipts(
        self,
        *,
        execution_id: str | None = None,
        connector_id: str | None = None,
        after_sequence: int = 0,
        limit: int = 256,
    ) -> tuple[AutonomousConnectorReceiptJournalEntry, ...]:
        if execution_id is not None:
            execution_id = _identifier("autonomous connector journal execution_id", execution_id)
        if connector_id is not None:
            connector_id = _identifier("autonomous connector journal connector_id", connector_id)
        if not isinstance(after_sequence, int) or isinstance(after_sequence, bool) or after_sequence < 0:
            raise ArgumentError("autonomous connector journal after_sequence must be non-negative")
        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= self.max_entries:
            raise ArgumentError("autonomous connector journal limit is outside its bound")
        with self._lock:
            rows = self._read_rows_locked()
        return tuple(
            row
            for row in rows
            if row.sequence > after_sequence
            and (execution_id is None or row.receipt.execution_id == execution_id)
            and (connector_id is None or row.receipt.connector_id == connector_id)
        )[:limit]

    def verify_integrity(self) -> dict[str, Any]:
        with self._lock:
            rows = self._read_rows_locked()
        return {
            "schema": AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_SCHEMA,
            "verified": True,
            "entries": len(rows),
            "head_digest": rows[-1].entry_digest if rows else None,
            "retention": "metadata_only_hash_chained_no_request_or_payload",
            "secret_material": "never_returned",
        }

    def _read_rows_locked(self) -> list[AutonomousConnectorReceiptJournalEntry]:
        if not self.path.exists():
            return []
        if self.path.stat().st_size > self.max_bytes:
            raise ArgumentError("autonomous connector receipt journal exceeds max_bytes")
        rows: list[AutonomousConnectorReceiptJournalEntry] = []
        identities: set[str] = set()
        with self.path.open("rb") as handle:
            for raw_line in handle:
                if len(rows) >= self.max_entries:
                    raise ArgumentError("autonomous connector receipt journal exceeds max_entries")
                try:
                    value = json.loads(raw_line.decode("utf-8"))
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    raise ArgumentError("autonomous connector receipt journal contains invalid JSON") from error
                if not isinstance(value, Mapping):
                    raise ArgumentError("autonomous connector receipt journal line must be an object")
                entry = _connector_entry_from_mapping(value)
                expected_previous = rows[-1].entry_digest if rows else None
                if entry.sequence != len(rows) + 1 or entry.previous_entry_digest != expected_previous:
                    raise ArgumentError("autonomous connector receipt journal hash chain is invalid")
                if entry.receipt_identity_digest in identities:
                    raise ArgumentError("autonomous connector receipt journal contains duplicate identities")
                identities.add(entry.receipt_identity_digest)
                if len(json.dumps(entry.to_dict(), ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")) > MAX_AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_BYTES:
                    raise ArgumentError("autonomous connector receipt journal entry exceeds its byte bound")
                rows.append(entry)
        return rows


@dataclass(frozen=True, slots=True)
class AutonomousConnectorDispatchResult:
    """Transient caller value paired with a durable-safe receipt."""

    receipt: AutonomousConnectorDispatchReceipt
    value: Any = None
    replay: str = "fresh"

    def __post_init__(self) -> None:
        if not isinstance(self.receipt, AutonomousConnectorDispatchReceipt):
            raise ArgumentError("autonomous connector dispatch result receipt is invalid")
        if self.replay not in {"fresh", "replayed"}:
            raise ArgumentError("autonomous connector dispatch result replay is invalid")
        safe_value = _json_safe(
            "autonomous connector dispatch result value",
            self.value,
            maximum=MAX_AUTONOMOUS_CONNECTOR_RESULT_BYTES,
        )
        _reject_secret_fields(safe_value)
        object.__setattr__(self, "value", safe_value)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA,
            "receipt": self.receipt.to_dict(),
            "value_present": self.value is not None,
            "replay": self.replay,
            "retention": "receipt_metadata_only;value_transient",
            "secret_material": "never_returned",
        }


@dataclass(slots=True)
class _AutonomousConnectorInFlight:
    request_digest: str
    event: threading.Event
    outcome: AutonomousConnectorDispatchResult | BaseException | None = None


class AutonomousConnectorRuntime:
    """Approval-aware dispatcher for caller-owned external evidence connectors."""

    def __init__(
        self,
        registry: AutonomousConnectorRegistry,
        *,
        receipt_sink: Callable[[AutonomousConnectorDispatchReceipt], Any] | None = None,
        receipt_store: Any | None = None,
    ) -> None:
        if not isinstance(registry, AutonomousConnectorRegistry):
            raise ArgumentError("autonomous connector runtime requires an AutonomousConnectorRegistry")
        if receipt_sink is not None and not callable(receipt_sink):
            raise ArgumentError("autonomous connector runtime receipt sink must be callable")
        if receipt_store is not None and not all(
            callable(getattr(receipt_store, name, None)) for name in ("append", "find")
        ):
            raise ArgumentError("autonomous connector runtime receipt store is malformed")
        self.registry = registry
        self.receipt_sink = receipt_sink
        self.receipt_store = receipt_store
        self._lock = threading.RLock()
        self._inflight: dict[str, _AutonomousConnectorInFlight] = {}

    def dispatch(self, request: AutonomousConnectorDispatchRequest) -> AutonomousConnectorDispatchResult:
        if not isinstance(request, AutonomousConnectorDispatchRequest):
            raise ArgumentError("autonomous connector dispatch requires a typed request")
        registration = self.registry.resolve(request.connector_id)
        request_digest = request.request_digest
        replay = self._find_replay(request, registration, request_digest)
        if replay is not None:
            return replay
        identity = _connector_dispatch_identity_digest(request)
        with self._lock:
            replay = self._find_replay(request, registration, request_digest)
            if replay is not None:
                return replay
            pending = self._inflight.get(identity)
            if pending is None:
                pending = _AutonomousConnectorInFlight(request_digest, threading.Event())
                self._inflight[identity] = pending
                owner = True
            else:
                if pending.request_digest != request_digest:
                    raise ArgumentError("autonomous connector dispatch identity conflicts with request metadata")
                owner = False
        if not owner:
            pending.event.wait()
            outcome = pending.outcome
            if isinstance(outcome, BaseException):
                raise outcome
            if outcome is None:
                raise ArgumentError("autonomous connector in-flight execution completed without an outcome")
            return AutonomousConnectorDispatchResult(outcome.receipt, outcome.value, replay="replayed")
        try:
            outcome = self._dispatch_fresh(request, registration, request_digest)
            with self._lock:
                pending.outcome = outcome
            return outcome
        except BaseException as error:
            with self._lock:
                pending.outcome = error
            raise
        finally:
            with self._lock:
                if self._inflight.get(identity) is pending:
                    self._inflight.pop(identity, None)
                pending.event.set()

    def dispatch_from_plan(
        self,
        plan: AutonomousConnectorSelectionPlan | Mapping[str, Any],
        request: AutonomousConnectorDispatchRequest,
    ) -> AutonomousConnectorDispatchResult:
        """Dispatch only when a reviewed selection plan still matches the live registry."""

        if isinstance(plan, Mapping):
            plan = AutonomousConnectorSelectionPlan.from_mapping(plan)
        if not isinstance(plan, AutonomousConnectorSelectionPlan):
            raise ArgumentError("autonomous connector planned dispatch requires a typed selection plan")
        if not isinstance(request, AutonomousConnectorDispatchRequest):
            raise ArgumentError("autonomous connector planned dispatch requires a typed request")
        plan.verify(self.registry)
        if plan.capability != request.capability:
            raise ArgumentError("autonomous connector planned dispatch capability does not match the plan")
        if request.selection_plan_digest != plan.plan_digest:
            raise ArgumentError("autonomous connector planned dispatch is not bound to the selection plan")
        rows = {row.domain: row for row in plan.rows}
        for domain in request.domains:
            row = rows.get(domain)
            if row is None or row.status != "selected" or row.connector_id != request.connector_id:
                raise ArgumentError("autonomous connector planned dispatch does not select the requested connector")
        return self.dispatch(request)

    def _dispatch_fresh(
        self,
        request: AutonomousConnectorDispatchRequest,
        registration: AutonomousConnectorRegistration,
        request_digest: str,
    ) -> AutonomousConnectorDispatchResult:
        manifest = registration.manifest
        missing_domains = sorted(set(request.domains) - set(manifest.domains))
        if missing_domains:
            return self._finish(
                request,
                registration,
                status="refused",
                failure_class="domain_scope",
                request_digest=request_digest,
            )
        if request.capability not in manifest.capabilities:
            return self._finish(
                request,
                registration,
                status="refused",
                failure_class="capability_scope",
                request_digest=request_digest,
            )
        if registration.approval_required and not request.approved:
            return self._finish(
                request,
                registration,
                status="refused",
                failure_class="approval_required",
                request_digest=request_digest,
            )
        try:
            raw = registration.executor(manifest, request.request)
            observation = raw if isinstance(raw, AutonomousConnectorObservation) else AutonomousConnectorObservation(raw)
        except Exception:
            return self._finish(
                request,
                registration,
                status="error",
                failure_class="executor_error",
                request_digest=request_digest,
            )
        payload_digest = None if observation.value is None else content_digest(observation.value)
        return self._finish(
            request,
            registration,
            status=observation.status,
            failure_class=observation.failure_class,
            request_digest=request_digest,
            payload_digest=payload_digest,
            value=observation.value,
        )

    def _find_replay(
        self,
        request: AutonomousConnectorDispatchRequest,
        registration: AutonomousConnectorRegistration,
        request_digest: str,
    ) -> AutonomousConnectorDispatchResult | None:
        if self.receipt_store is None:
            return None
        stored = self.receipt_store.find(
            execution_id=request.execution_id,
            dispatch_id=request.dispatch_id,
            call_id=request.call_id,
            connector_id=request.connector_id,
            attempt_id=request.attempt_id,
        )
        if stored is None:
            return None
        receipt = _connector_receipt_from_mapping(stored)
        if receipt.request_digest != request_digest:
            raise ArgumentError("autonomous connector replay identity conflicts with request metadata")
        if receipt.manifest_digest != registration.manifest_digest:
            raise ArgumentError("autonomous connector replay manifest digest changed")
        return AutonomousConnectorDispatchResult(receipt, None, replay="replayed")

    def _finish(
        self,
        request: AutonomousConnectorDispatchRequest,
        registration: AutonomousConnectorRegistration,
        *,
        status: str,
        failure_class: str | None,
        request_digest: str,
        payload_digest: str | None = None,
        value: Any = None,
    ) -> AutonomousConnectorDispatchResult:
        manifest = registration.manifest
        receipt = AutonomousConnectorDispatchReceipt(
            dispatch_id=request.dispatch_id,
            execution_id=request.execution_id,
            call_id=request.call_id,
            connector_id=manifest.connector_id,
            connector_version=manifest.version,
            provider=manifest.provider,
            connector_kind=manifest.connector_kind,
            manifest_digest=registration.manifest_digest,
            domains=request.domains,
            capability=request.capability,
            status=status,
            request_digest=request_digest,
            payload_digest=payload_digest,
            parent_digests=request.parent_digests,
            attempt_id=request.attempt_id,
            failure_class=failure_class,
        )
        persisted_receipt = receipt
        if self.receipt_store is not None:
            try:
                stored = self.receipt_store.append(receipt)
                if isinstance(stored, AutonomousConnectorReceiptJournalEntry):
                    persisted_receipt = stored.receipt
                elif isinstance(stored, Mapping) and stored.get("schema") == AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA:
                    persisted_receipt = _connector_entry_from_mapping(stored).receipt
                elif stored is not None:
                    persisted_receipt = _connector_receipt_from_mapping(stored)
            except Exception as error:
                raise ArgumentError("autonomous connector receipt store failed") from error
        if self.receipt_sink is not None:
            try:
                self.receipt_sink(persisted_receipt)
            except Exception as error:
                raise ArgumentError("autonomous connector receipt sink failed") from error
        return AutonomousConnectorDispatchResult(persisted_receipt, value, replay="fresh")


def create_autonomous_api_source_connector_executor(
    client: ApiClient,
    *,
    use_tool_route: bool = True,
) -> Callable[[DomainEvidenceProviderConnectorManifest, Mapping[str, Any]], Any]:
    """Build a key-agnostic source connector over the typed REST/MCP source routes.

    The caller supplies the already-configured ``ApiClient`` and may close over an opaque
    credential/session in the transport implementation. This helper performs no catalogue or
    credential discovery. It requires a transient request shaped as ``{"plan": {...},
    "execution": {...}}``, creates the typed source plan, then binds execution to the plan digest
    returned by the gateway rather than trusting a caller-supplied digest.
    """

    if not isinstance(client, ApiClient):
        raise ArgumentError("autonomous API source connector requires an ApiClient")
    if not isinstance(use_tool_route, bool):
        raise ArgumentError("autonomous API source connector use_tool_route must be a boolean")

    def execute(
        manifest: DomainEvidenceProviderConnectorManifest,
        request: Mapping[str, Any],
    ) -> AutonomousConnectorObservation:
        if not isinstance(manifest, DomainEvidenceProviderConnectorManifest):
            raise ArgumentError("autonomous API source connector received an invalid manifest")
        if not isinstance(request, Mapping):
            raise ArgumentError("autonomous API source connector request must be an object")
        plan_raw = request.get("plan")
        execution_raw = request.get("execution", {})
        if not isinstance(plan_raw, Mapping) or not isinstance(execution_raw, Mapping):
            raise ArgumentError("autonomous API source connector requires plan and execution objects")
        plan_request = DomainEvidenceSourcePlanRequest(**dict(plan_raw))
        if plan_request.connector_kind != manifest.connector_kind:
            raise ArgumentError("autonomous API source connector kind does not match its manifest")
        if any(domain not in manifest.domains for domain in plan_request.domains):
            raise ArgumentError("autonomous API source connector plan exceeds manifest domain scope")
        plan_report = (
            client.domain_evidence_source_plan_tool(plan_request)
            if use_tool_route
            else client.domain_evidence_source_plan(plan_request)
        )
        plan_digest = getattr(plan_report, "plan_digest", None)
        if not isinstance(plan_digest, str):
            raise ArgumentError("autonomous API source connector plan response omitted its digest")
        parent_digests = execution_raw.get("parent_digests", plan_request.parent_digests)
        if not isinstance(parent_digests, Sequence) or isinstance(parent_digests, (str, bytes)):
            raise ArgumentError("autonomous API source connector parent_digests must be a sequence")
        execution_request = DomainEvidenceSourceExecutionRequest(
            source_plan_digest=plan_digest,
            source_tool=execution_raw.get("source_tool", plan_request.source_tool),
            request=execution_raw.get("request"),
            claim_posture=execution_raw.get("claim_posture"),
            parent_digests=tuple(parent_digests),
        )
        execution_report = (
            client.domain_evidence_source_execute_tool(execution_request)
            if use_tool_route
            else client.domain_evidence_source_execute(execution_request)
        )
        outcome = getattr(execution_report, "outcome", None)
        report_value = execution_report.to_dict() if hasattr(execution_report, "to_dict") else None
        if outcome not in AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES or report_value is None:
            raise ArgumentError("autonomous API source connector execution response is malformed")
        return AutonomousConnectorObservation(value=report_value, status=outcome)

    return execute


__all__ = [
    "AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA",
    "AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA",
    "AUTONOMOUS_CONNECTOR_SELECTION_PLAN_SCHEMA",
    "AUTONOMOUS_CONNECTOR_SELECTION_ROW_SCHEMA",
    "AUTONOMOUS_CONNECTOR_SELECTION_STRATEGIES",
    "AUTONOMOUS_CONNECTOR_RECEIPT_SCHEMA",
    "AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_SCHEMA",
    "AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA",
    "AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES",
    "MAX_AUTONOMOUS_CONNECTORS",
    "MAX_AUTONOMOUS_CONNECTOR_DOMAINS",
    "MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES",
    "MAX_AUTONOMOUS_CONNECTOR_RESULT_BYTES",
    "MAX_AUTONOMOUS_CONNECTOR_PARENT_DIGESTS",
    "MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_ENTRIES",
    "MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_BYTES",
    "MAX_AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_BYTES",
    "MAX_AUTONOMOUS_CONNECTOR_SELECTION_SIGNAL_BYTES",
    "AutonomousConnectorRegistration",
    "AutonomousConnectorRegistry",
    "AutonomousConnectorSelectionRow",
    "AutonomousConnectorSelectionPlan",
    "AutonomousConnectorDispatchRequest",
    "AutonomousConnectorObservation",
    "AutonomousConnectorDispatchReceipt",
    "AutonomousConnectorReceiptJournalEntry",
    "AutonomousConnectorReceiptJournal",
    "AutonomousConnectorDispatchResult",
    "AutonomousConnectorRuntime",
    "create_autonomous_api_source_connector_executor",
]
