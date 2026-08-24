"""Bounded multi-source evidence fan-out, normalization, and quorum adjudication.

This module adds the missing multi-provider layer above the evidence runtime. It plans a fixed set
of caller-owned source routes, requires explicit source-dispatch approval, executes the routes with
bounded concurrency, and returns only digest-bound source metadata in the durable result. Acquired
and normalized values are available only in the transient result object so an application can pass
them to its own evaluator or UI without turning consensus into a truth oracle.
"""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
import json
import math
from typing import Any, Callable, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence import AutonomousEvidencePlan, AutonomousEvidenceRequirement
from .autonomous_evidence_retry import classify_autonomous_evidence_acquisition_error
from .autonomous_evidence_runtime import AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_EVIDENCE_RECONCILIATION_PLAN_SCHEMA = "bioprism-python-autonomous-evidence-reconciliation-plan/0.1"
AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_SCHEMA = "bioprism-python-autonomous-evidence-reconciliation-source/0.1"
AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_SCHEMA = "bioprism-python-autonomous-evidence-reconciliation-result/0.1"
MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_ROUTES = 16
MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_CONCURRENCY = 8
MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_METADATA_BYTES = 64_000
MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_VALUE_BYTES = 64_000_000
MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_BYTES = 512_000
MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_PARENT_DIGESTS = 64

AUTONOMOUS_EVIDENCE_RECONCILIATION_STATUSES = frozenset({
    "consensus", "consensus_with_dissent", "disagreement", "insufficient_evidence", "failed",
})
AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_STATUSES = frozenset({"observed", "failed"})
_RETENTION = "metadata_only;source_values_and_normalized_values_caller_owned"
_PLAN_RETENTION = "metadata_only;route_metadata_and_digests_only"
_ROUTE_RETENTION = "metadata_only;request_metadata_caller_owned"
_SECRET_MARKERS = frozenset({
    "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
    "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
})


def _text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value.strip()


def _identifier(name: str, value: Any, maximum: int = 256) -> str:
    result = _text(name, value, maximum)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+- /" for character in result):
        raise ArgumentError(f"{name} contains unsupported identifier characters")
    return result


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum or value > maximum:
        raise ArgumentError(f"{name} is outside its bound")
    return value


def _safe_marker(value: str) -> str:
    return "".join(character for character in value.lower() if character.isalnum())


def _assert_safe_json(value: Any, name: str, depth: int = 0, *, reject_secret_fields: bool = True) -> None:
    if depth > 32:
        raise ArgumentError(f"{name} is too deeply nested")
    if value is None or isinstance(value, (str, bool, int)):
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ArgumentError(f"{name} contains a non-finite number")
        return
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip() or "\x00" in key:
                raise ArgumentError(f"{name} contains an invalid object field")
            marker = _safe_marker(key)
            if reject_secret_fields and (marker in _SECRET_MARKERS or any(part in marker for part in ("token", "secret", "credential", "authorization"))):
                raise ArgumentError(f"{name}.{key} is credential-shaped metadata")
            _assert_safe_json(child, f"{name}.{key}", depth + 1, reject_secret_fields=reject_secret_fields)
        return
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        if len(value) > 16_384:
            raise ArgumentError(f"{name} contains too many entries")
        for index, child in enumerate(value):
            _assert_safe_json(child, f"{name}[{index}]", depth + 1, reject_secret_fields=reject_secret_fields)
        return
    raise ArgumentError(f"{name} is not JSON-safe")


def _json_bytes(value: Any, name: str, maximum: int, *, reject_secret_fields: bool = True) -> int:
    _assert_safe_json(value, name, reject_secret_fields=reject_secret_fields)
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} is not canonical JSON") from error
    if len(encoded) > maximum:
        raise ArgumentError(f"{name} exceeds its bounded byte limit")
    return len(encoded)


def _safe_metadata(value: Any, name: str) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be a mapping")
    _json_bytes(value, name, MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_METADATA_BYTES)
    # Canonical round-tripping makes tuple/list and mapping subclasses deterministic without
    # retaining a caller-owned mutable object in the plan identity.
    try:
        normalized = json.loads(canonical_json(value))
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} is not canonical JSON") from error
    if not isinstance(normalized, dict):
        raise ArgumentError(f"{name} must canonicalize to an object")
    return normalized


def _safe_value(value: Any, name: str) -> tuple[Any, int]:
    size = _json_bytes(value, name, MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_VALUE_BYTES)
    return value, size


def _limited_text_list(name: str, value: Any, maximum: int) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or len(value) > maximum:
        raise ArgumentError(f"{name} is outside its bound")
    normalized = tuple(_text(f"{name}[{index}]", item, 512) for index, item in enumerate(value))
    if len(set(normalized)) != len(normalized):
        raise ArgumentError(f"{name} contains duplicates")
    return normalized


def _requirement_for(evidence_plan: AutonomousEvidencePlan, requirement_id: str) -> AutonomousEvidenceRequirement:
    if not isinstance(evidence_plan, AutonomousEvidencePlan):
        raise ArgumentError("evidence reconciliation requires a typed evidence plan")
    normalized = _identifier("evidence reconciliation requirement_id", requirement_id)
    requirement = next((item for item in evidence_plan.requirements if item.requirement_id == normalized), None)
    if requirement is None:
        raise ArgumentError(f"evidence reconciliation requirement is not in the plan: {normalized}")
    if requirement.domain not in AUTONOMOUS_DOMAIN_NAMES:
        raise ArgumentError("evidence reconciliation requirement domain is unsupported")
    return requirement


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceReconciliationRouteDescriptor:
    source_id: str
    source_digest: str | None
    request_id: str | None
    metadata: Mapping[str, Any]

    def __post_init__(self) -> None:
        _identifier("evidence reconciliation source_id", self.source_id)
        _digest("evidence reconciliation source_digest", self.source_digest, allow_none=True)
        if self.request_id is not None:
            _identifier("evidence reconciliation request_id", self.request_id)
        normalized = _safe_metadata(self.metadata, "evidence reconciliation request metadata")
        object.__setattr__(self, "metadata", normalized)

    @property
    def metadata_digest(self) -> str:
        return content_digest(self.metadata)


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceReconciliationRoute:
    source_id: str
    source_digest: str | None
    request_id: str | None
    metadata: Mapping[str, Any]
    acquirer: Any = field(compare=False, repr=False)

    def __post_init__(self) -> None:
        descriptor = AutonomousEvidenceReconciliationRouteDescriptor(
            self.source_id, self.source_digest, self.request_id, self.metadata,
        )
        if not callable(getattr(self.acquirer, "acquire", None)):
            raise ArgumentError("evidence reconciliation route acquirer is malformed")
        object.__setattr__(self, "source_id", descriptor.source_id)
        object.__setattr__(self, "source_digest", descriptor.source_digest)
        object.__setattr__(self, "request_id", descriptor.request_id)
        object.__setattr__(self, "metadata", descriptor.metadata)

    def descriptor(self) -> AutonomousEvidenceReconciliationRouteDescriptor:
        return AutonomousEvidenceReconciliationRouteDescriptor(self.source_id, self.source_digest, self.request_id, self.metadata)


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceReconciliationRouteProjection:
    source_id: str
    source_digest: str | None
    request_id: str | None
    metadata_digest: str

    def __post_init__(self) -> None:
        _identifier("evidence reconciliation route source_id", self.source_id)
        _digest("evidence reconciliation route source_digest", self.source_digest, allow_none=True)
        if self.request_id is not None:
            _identifier("evidence reconciliation route request_id", self.request_id)
        _digest("evidence reconciliation route metadata_digest", self.metadata_digest)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_SCHEMA,
            "source_id": self.source_id,
            "source_digest": self.source_digest,
            "request_id": self.request_id,
            "metadata_digest": self.metadata_digest,
            "execution": "planned_route_only;source_dispatch_not_started",
            "retention": _ROUTE_RETENTION,
            "secret_material": "never_returned",
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousEvidenceReconciliationRouteProjection":
        if not isinstance(value, Mapping):
            raise ArgumentError("evidence reconciliation route projection is malformed")
        allowed = {"schema", "source_id", "source_digest", "request_id", "metadata_digest", "execution", "retention", "secret_material"}
        if set(value) != allowed or value.get("schema") != AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_SCHEMA or value.get("execution") != "planned_route_only;source_dispatch_not_started" or value.get("retention") != _ROUTE_RETENTION or value.get("secret_material") != "never_returned":
            raise ArgumentError("evidence reconciliation route projection retention is invalid")
        projection = cls(value.get("source_id"), value.get("source_digest"), value.get("request_id"), value.get("metadata_digest"))
        if canonical_json(value) != canonical_json(projection.to_dict()):
            raise ArgumentError("evidence reconciliation route projection is not canonical")
        return projection


def _projection_for_route(route: AutonomousEvidenceReconciliationRoute) -> AutonomousEvidenceReconciliationRouteProjection:
    descriptor = route.descriptor()
    return AutonomousEvidenceReconciliationRouteProjection(
        descriptor.source_id, descriptor.source_digest, descriptor.request_id, descriptor.metadata_digest,
    )


def _parent_digests(value: Any) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or len(value) > MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_PARENT_DIGESTS:
        raise ArgumentError("evidence reconciliation parent evidence digests are outside their bound")
    result = tuple(_digest(f"evidence reconciliation parent_evidence_digests[{index}]", item) for index, item in enumerate(value))
    if len(set(result)) != len(result):
        raise ArgumentError("evidence reconciliation parent evidence digests contain duplicates")
    return result


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceReconciliationPlan:
    evidence_plan_digest: str
    requirement_id: str
    domain: str
    workflow_id: str
    stage_id: str
    routes: tuple[AutonomousEvidenceReconciliationRouteProjection, ...]
    quorum: int
    max_concurrency: int
    require_all: bool
    normalizer_id: str
    normalizer_version: str
    parent_evidence_digests: tuple[str, ...]
    plan_digest: str

    def __post_init__(self) -> None:
        _digest("evidence reconciliation evidence_plan_digest", self.evidence_plan_digest)
        _identifier("evidence reconciliation requirement_id", self.requirement_id)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("evidence reconciliation plan domain is unsupported")
        _identifier("evidence reconciliation workflow_id", self.workflow_id)
        _identifier("evidence reconciliation stage_id", self.stage_id)
        if isinstance(self.routes, (str, bytes)) or not isinstance(self.routes, Sequence) or not 1 <= len(self.routes) <= MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_ROUTES or any(not isinstance(route, AutonomousEvidenceReconciliationRouteProjection) for route in self.routes):
            raise ArgumentError("evidence reconciliation routes are outside their bound")
        if len({route.source_id for route in self.routes}) != len(self.routes):
            raise ArgumentError("evidence reconciliation source IDs must be unique")
        if tuple(sorted(self.routes, key=lambda route: route.source_id)) != tuple(self.routes):
            raise ArgumentError("evidence reconciliation routes must be sorted by source_id")
        _integer("evidence reconciliation quorum", self.quorum, 1, len(self.routes))
        _integer("evidence reconciliation max_concurrency", self.max_concurrency, 1, min(MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_CONCURRENCY, len(self.routes)))
        if not isinstance(self.require_all, bool):
            raise ArgumentError("evidence reconciliation require_all must be boolean")
        _identifier("evidence reconciliation normalizer_id", self.normalizer_id)
        _identifier("evidence reconciliation normalizer_version", self.normalizer_version)
        if self.normalizer_id == "identity" and self.normalizer_version != "1":
            raise ArgumentError("identity normalizer version must be 1")
        if not isinstance(self.parent_evidence_digests, tuple):
            raise ArgumentError("evidence reconciliation parent evidence digests must be a tuple")
        _parent_digests(self.parent_evidence_digests)
        _digest("evidence reconciliation plan_digest", self.plan_digest)
        if content_digest(self._payload()) != self.plan_digest:
            raise ArgumentError("evidence reconciliation plan digest is invalid")

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_RECONCILIATION_PLAN_SCHEMA,
            "evidence_plan_digest": self.evidence_plan_digest,
            "requirement_id": self.requirement_id,
            "domain": self.domain,
            "workflow_id": self.workflow_id,
            "stage_id": self.stage_id,
            "route_count": len(self.routes),
            "routes": [route.to_dict() for route in self.routes],
            "quorum": self.quorum,
            "max_concurrency": self.max_concurrency,
            "require_all": self.require_all,
            "normalizer_id": self.normalizer_id,
            "normalizer_version": self.normalizer_version,
            "parent_evidence_digests": list(self.parent_evidence_digests),
            "approval_required": True,
            "execution": "planning_only;source_dispatch_not_started",
            "retention": _PLAN_RETENTION,
            "secret_material": "never_returned",
        }

    @classmethod
    def create(
        cls,
        *,
        evidence_plan: AutonomousEvidencePlan,
        requirement: AutonomousEvidenceRequirement,
        routes: Sequence[AutonomousEvidenceReconciliationRouteProjection],
        quorum: int,
        max_concurrency: int,
        require_all: bool,
        normalizer_id: str,
        normalizer_version: str,
        parent_evidence_digests: Sequence[str],
    ) -> "AutonomousEvidenceReconciliationPlan":
        projections = tuple(sorted(routes, key=lambda route: route.source_id))
        payload = {
            "schema": AUTONOMOUS_EVIDENCE_RECONCILIATION_PLAN_SCHEMA,
            "evidence_plan_digest": evidence_plan.plan_digest,
            "requirement_id": requirement.requirement_id,
            "domain": requirement.domain,
            "workflow_id": requirement.workflow_id,
            "stage_id": requirement.stage_id,
            "route_count": len(projections),
            "routes": [route.to_dict() for route in projections],
            "quorum": quorum,
            "max_concurrency": max_concurrency,
            "require_all": require_all,
            "normalizer_id": normalizer_id,
            "normalizer_version": normalizer_version,
            "parent_evidence_digests": list(parent_evidence_digests),
            "approval_required": True,
            "execution": "planning_only;source_dispatch_not_started",
            "retention": _PLAN_RETENTION,
            "secret_material": "never_returned",
        }
        return cls(
            evidence_plan_digest=evidence_plan.plan_digest,
            requirement_id=requirement.requirement_id,
            domain=requirement.domain,
            workflow_id=requirement.workflow_id,
            stage_id=requirement.stage_id,
            routes=projections,
            quorum=quorum,
            max_concurrency=max_concurrency,
            require_all=require_all,
            normalizer_id=normalizer_id,
            normalizer_version=normalizer_version,
            parent_evidence_digests=tuple(parent_evidence_digests),
            plan_digest=content_digest(payload),
        )

    def verify(self, evidence_plan: AutonomousEvidencePlan) -> "AutonomousEvidenceReconciliationPlan":
        if not isinstance(evidence_plan, AutonomousEvidencePlan):
            raise ArgumentError("evidence reconciliation verification requires a typed evidence plan")
        if evidence_plan.plan_digest != self.evidence_plan_digest:
            raise ArgumentError("evidence reconciliation evidence plan changed after planning")
        if content_digest(self._payload()) != self.plan_digest:
            raise ArgumentError("evidence reconciliation plan digest is invalid")
        return self

    def to_dict(self) -> dict[str, Any]:
        result = {**self._payload(), "plan_digest": self.plan_digest}
        _json_bytes(result, "evidence reconciliation plan", MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_BYTES, reject_secret_fields=False)
        return result

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousEvidenceReconciliationPlan":
        if not isinstance(value, Mapping):
            raise ArgumentError("evidence reconciliation plan must be a mapping")
        allowed = {
            "schema", "evidence_plan_digest", "requirement_id", "domain", "workflow_id", "stage_id",
            "route_count", "routes", "quorum", "max_concurrency", "require_all", "normalizer_id",
            "normalizer_version", "parent_evidence_digests", "approval_required", "execution", "retention",
            "secret_material", "plan_digest",
        }
        if set(value) != allowed or value.get("schema") != AUTONOMOUS_EVIDENCE_RECONCILIATION_PLAN_SCHEMA or value.get("approval_required") is not True or value.get("execution") != "planning_only;source_dispatch_not_started" or value.get("retention") != _PLAN_RETENTION or value.get("secret_material") != "never_returned":
            raise ArgumentError("evidence reconciliation plan contains unsupported or transient fields")
        raw_routes = value.get("routes")
        if not isinstance(raw_routes, Sequence) or isinstance(raw_routes, (str, bytes, bytearray)):
            raise ArgumentError("evidence reconciliation plan routes must be a sequence")
        routes = tuple(AutonomousEvidenceReconciliationRouteProjection.from_dict(route) for route in raw_routes)
        if value.get("route_count") != len(routes):
            raise ArgumentError("evidence reconciliation plan route_count is inconsistent")
        plan = cls(
            evidence_plan_digest=value.get("evidence_plan_digest"),
            requirement_id=value.get("requirement_id"),
            domain=value.get("domain"),
            workflow_id=value.get("workflow_id"),
            stage_id=value.get("stage_id"),
            routes=routes,
            quorum=value.get("quorum"),
            max_concurrency=value.get("max_concurrency"),
            require_all=value.get("require_all"),
            normalizer_id=value.get("normalizer_id"),
            normalizer_version=value.get("normalizer_version"),
            parent_evidence_digests=tuple(value.get("parent_evidence_digests", ())),
            plan_digest=value.get("plan_digest"),
        )
        if canonical_json(value) != canonical_json(plan.to_dict()):
            raise ArgumentError("evidence reconciliation plan is not canonical")
        return plan


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceReconciliationSourceResult:
    source_id: str
    source_digest: str | None
    request_id: str | None
    request_digest: str
    metadata_digest: str
    status: str
    value_digest: str | None
    value_bytes: int
    normalized_digest: str | None
    failure_class: str | None
    retryable: bool
    limitations: tuple[str, ...]

    def __post_init__(self) -> None:
        _identifier("evidence reconciliation source result source_id", self.source_id)
        _digest("evidence reconciliation source result source_digest", self.source_digest, allow_none=True)
        if self.request_id is not None:
            _identifier("evidence reconciliation source result request_id", self.request_id)
        _digest("evidence reconciliation source result request_digest", self.request_digest)
        _digest("evidence reconciliation source result metadata_digest", self.metadata_digest)
        if self.status not in AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_STATUSES:
            raise ArgumentError("evidence reconciliation source result status is invalid")
        _digest("evidence reconciliation source result value_digest", self.value_digest, allow_none=True)
        _integer("evidence reconciliation source result value_bytes", self.value_bytes, 0, MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_VALUE_BYTES)
        _digest("evidence reconciliation source result normalized_digest", self.normalized_digest, allow_none=True)
        if self.failure_class is not None:
            _identifier("evidence reconciliation source result failure_class", self.failure_class, 128)
        if not isinstance(self.retryable, bool):
            raise ArgumentError("evidence reconciliation source result retryable must be boolean")
        _limited_text_list("evidence reconciliation source result limitations", self.limitations, 32)
        if self.status == "observed" and (self.value_digest is None or self.normalized_digest is None):
            raise ArgumentError("observed evidence reconciliation source result requires value and normalized digests")
        if self.status == "failed" and (self.value_digest is not None or self.normalized_digest is not None or self.value_bytes != 0):
            raise ArgumentError("failed evidence reconciliation source result cannot retain value metadata")

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_SCHEMA,
            "source_id": self.source_id,
            "source_digest": self.source_digest,
            "request_id": self.request_id,
            "request_digest": self.request_digest,
            "metadata_digest": self.metadata_digest,
            "status": self.status,
            "value_digest": self.value_digest,
            "value_bytes": self.value_bytes,
            "normalized_digest": self.normalized_digest,
            "failure_class": self.failure_class,
            "retryable": self.retryable,
            "limitations": list(self.limitations),
            "retention": "metadata_only;source_values_caller_owned",
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        return self._payload()

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousEvidenceReconciliationSourceResult":
        if not isinstance(value, Mapping):
            raise ArgumentError("evidence reconciliation source result must be a mapping")
        allowed = {
            "schema", "source_id", "source_digest", "request_id", "request_digest", "metadata_digest", "status",
            "value_digest", "value_bytes", "normalized_digest", "failure_class", "retryable", "limitations",
            "retention", "secret_material",
        }
        if set(value) != allowed or value.get("schema") != AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_SCHEMA or value.get("retention") != "metadata_only;source_values_caller_owned" or value.get("secret_material") != "never_returned":
            raise ArgumentError("evidence reconciliation source result contains unsupported fields")
        result = cls(
            source_id=value.get("source_id"), source_digest=value.get("source_digest"), request_id=value.get("request_id"),
            request_digest=value.get("request_digest"), metadata_digest=value.get("metadata_digest"), status=value.get("status"),
            value_digest=value.get("value_digest"), value_bytes=value.get("value_bytes"), normalized_digest=value.get("normalized_digest"),
            failure_class=value.get("failure_class"), retryable=value.get("retryable"), limitations=tuple(value.get("limitations", ())),
        )
        if canonical_json(value) != canonical_json(result.to_dict()):
            raise ArgumentError("evidence reconciliation source result is not canonical")
        return result


def _result_payload(
    *,
    evidence_plan_digest: str,
    requirement_id: str,
    domain: str,
    reconciliation_plan_digest: str,
    status: str,
    source_results: Sequence[AutonomousEvidenceReconciliationSourceResult],
    quorum: int,
    consensus_normalized_digest: str | None,
    disagreement_digest: str | None,
) -> dict[str, Any]:
    observed_count = sum(result.status == "observed" for result in source_results)
    failed_count = sum(result.status == "failed" for result in source_results)
    normalized_count = len({result.normalized_digest for result in source_results if result.normalized_digest is not None})
    return {
        "schema": AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_SCHEMA,
        "evidence_plan_digest": evidence_plan_digest,
        "requirement_id": requirement_id,
        "domain": domain,
        "reconciliation_plan_digest": reconciliation_plan_digest,
        "status": status,
        "route_count": len(source_results),
        "observed_count": observed_count,
        "failed_count": failed_count,
        "unique_normalized_count": normalized_count,
        "quorum": quorum,
        "consensus_normalized_digest": consensus_normalized_digest,
        "disagreement_digest": disagreement_digest,
        "source_results": [result.to_dict() for result in source_results],
        "retention": _RETENTION,
        "secret_material": "never_returned",
    }


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceReconciliationResult:
    evidence_plan_digest: str
    requirement_id: str
    domain: str
    reconciliation_plan_digest: str
    status: str
    quorum: int
    consensus_normalized_digest: str | None
    disagreement_digest: str | None
    source_results: tuple[AutonomousEvidenceReconciliationSourceResult, ...]
    result_digest: str
    values: Mapping[str, Any] = field(default_factory=dict, compare=False, repr=False)
    normalized_values: Mapping[str, Any] = field(default_factory=dict, compare=False, repr=False)

    def __post_init__(self) -> None:
        _digest("evidence reconciliation result evidence_plan_digest", self.evidence_plan_digest)
        _identifier("evidence reconciliation result requirement_id", self.requirement_id)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("evidence reconciliation result domain is unsupported")
        _digest("evidence reconciliation result reconciliation_plan_digest", self.reconciliation_plan_digest)
        if self.status not in AUTONOMOUS_EVIDENCE_RECONCILIATION_STATUSES:
            raise ArgumentError("evidence reconciliation result status is invalid")
        _integer("evidence reconciliation result quorum", self.quorum, 1, MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_ROUTES)
        _digest("evidence reconciliation result consensus_normalized_digest", self.consensus_normalized_digest, allow_none=True)
        _digest("evidence reconciliation result disagreement_digest", self.disagreement_digest, allow_none=True)
        if isinstance(self.source_results, (str, bytes)) or not isinstance(self.source_results, Sequence) or not self.source_results or any(not isinstance(item, AutonomousEvidenceReconciliationSourceResult) for item in self.source_results):
            raise ArgumentError("evidence reconciliation result source_results are malformed")
        if tuple(sorted(self.source_results, key=lambda result: result.source_id)) != tuple(self.source_results):
            raise ArgumentError("evidence reconciliation result source_results must be sorted")
        _digest("evidence reconciliation result result_digest", self.result_digest)
        if content_digest(self._payload()) != self.result_digest:
            raise ArgumentError("evidence reconciliation result digest is invalid")
        if not isinstance(self.values, Mapping) or not isinstance(self.normalized_values, Mapping):
            raise ArgumentError("evidence reconciliation transient values are malformed")

    def _payload(self) -> dict[str, Any]:
        return _result_payload(
            evidence_plan_digest=self.evidence_plan_digest,
            requirement_id=self.requirement_id,
            domain=self.domain,
            reconciliation_plan_digest=self.reconciliation_plan_digest,
            status=self.status,
            source_results=self.source_results,
            quorum=self.quorum,
            consensus_normalized_digest=self.consensus_normalized_digest,
            disagreement_digest=self.disagreement_digest,
        )

    @classmethod
    def build(
        cls,
        *,
        evidence_plan_digest: str,
        requirement_id: str,
        domain: str,
        reconciliation_plan_digest: str,
        status: str,
        quorum: int,
        consensus_normalized_digest: str | None,
        disagreement_digest: str | None,
        source_results: Sequence[AutonomousEvidenceReconciliationSourceResult],
        values: Mapping[str, Any],
        normalized_values: Mapping[str, Any],
    ) -> "AutonomousEvidenceReconciliationResult":
        ordered = tuple(sorted(source_results, key=lambda result: result.source_id))
        payload = _result_payload(
            evidence_plan_digest=evidence_plan_digest,
            requirement_id=requirement_id,
            domain=domain,
            reconciliation_plan_digest=reconciliation_plan_digest,
            status=status,
            source_results=ordered,
            quorum=quorum,
            consensus_normalized_digest=consensus_normalized_digest,
            disagreement_digest=disagreement_digest,
        )
        return cls(
            evidence_plan_digest=evidence_plan_digest,
            requirement_id=requirement_id,
            domain=domain,
            reconciliation_plan_digest=reconciliation_plan_digest,
            status=status,
            quorum=quorum,
            consensus_normalized_digest=consensus_normalized_digest,
            disagreement_digest=disagreement_digest,
            source_results=ordered,
            result_digest=content_digest(payload),
            values=dict(values),
            normalized_values=dict(normalized_values),
        )

    def to_dict(self) -> dict[str, Any]:
        result = {**self._payload(), "result_digest": self.result_digest}
        _json_bytes(result, "evidence reconciliation result", MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_BYTES, reject_secret_fields=False)
        return result

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousEvidenceReconciliationResult":
        if not isinstance(value, Mapping):
            raise ArgumentError("evidence reconciliation result must be a mapping")
        allowed = {
            "schema", "evidence_plan_digest", "requirement_id", "domain", "reconciliation_plan_digest", "status",
            "route_count", "observed_count", "failed_count", "unique_normalized_count", "quorum",
            "consensus_normalized_digest", "disagreement_digest", "source_results", "result_digest",
            "retention", "secret_material",
        }
        if set(value) != allowed or value.get("schema") != AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_SCHEMA or value.get("retention") != _RETENTION or value.get("secret_material") != "never_returned":
            raise ArgumentError("evidence reconciliation result contains unsupported fields")
        raw_results = value.get("source_results")
        if not isinstance(raw_results, Sequence) or isinstance(raw_results, (str, bytes, bytearray)):
            raise ArgumentError("evidence reconciliation result source_results must be a sequence")
        source_results = tuple(AutonomousEvidenceReconciliationSourceResult.from_dict(item) for item in raw_results)
        result = cls(
            evidence_plan_digest=value.get("evidence_plan_digest"), requirement_id=value.get("requirement_id"), domain=value.get("domain"),
            reconciliation_plan_digest=value.get("reconciliation_plan_digest"), status=value.get("status"), quorum=value.get("quorum"),
            consensus_normalized_digest=value.get("consensus_normalized_digest"), disagreement_digest=value.get("disagreement_digest"),
            source_results=source_results, result_digest=value.get("result_digest"), values={}, normalized_values={},
        )
        wire = result.to_dict()
        if value.get("route_count") != len(source_results) or value.get("observed_count") != sum(item.status == "observed" for item in source_results) or value.get("failed_count") != sum(item.status == "failed" for item in source_results) or value.get("unique_normalized_count") != len({item.normalized_digest for item in source_results if item.normalized_digest is not None}):
            raise ArgumentError("evidence reconciliation result aggregates are inconsistent")
        if canonical_json(value) != canonical_json(wire):
            raise ArgumentError("evidence reconciliation result is not canonical")
        return result


@dataclass(frozen=True, slots=True)
class _SourceExecution:
    descriptor: AutonomousEvidenceReconciliationRouteDescriptor
    request_digest: str
    status: str
    value: Any
    normalized: Any
    result: AutonomousEvidenceReconciliationSourceResult


def _request_digest(plan_digest: str, requirement_id: str, descriptor: AutonomousEvidenceReconciliationRouteDescriptor) -> str:
    return content_digest({
        "schema": AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
        "plan_digest": plan_digest,
        "requirement_id": requirement_id,
        "source_id": descriptor.source_id,
        "source_digest": descriptor.source_digest,
        "request_id": descriptor.request_id,
        "metadata": descriptor.metadata,
    })


class AutonomousEvidenceSourceReconciler:
    """Plan and execute bounded fan-out over explicit caller-owned source routes."""

    def __init__(self, evidence_plan: AutonomousEvidencePlan) -> None:
        if not isinstance(evidence_plan, AutonomousEvidencePlan):
            raise ArgumentError("evidence reconciler requires a typed evidence plan")
        self.evidence_plan = evidence_plan

    def prepare(
        self,
        requirement_id: str,
        routes: Sequence[AutonomousEvidenceReconciliationRoute],
        *,
        quorum: int | None = None,
        max_concurrency: int | None = None,
        require_all: bool = False,
        normalizer_id: str = "identity",
        normalizer_version: str = "1",
        parent_evidence_digests: Sequence[str] = (),
    ) -> AutonomousEvidenceReconciliationPlan:
        requirement = _requirement_for(self.evidence_plan, requirement_id)
        if isinstance(routes, (str, bytes, bytearray)) or not isinstance(routes, Sequence) or not 1 <= len(routes) <= MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_ROUTES:
            raise ArgumentError("evidence reconciliation routes are outside their bound")
        descriptors = tuple(_projection_for_route(route) for route in routes)
        if len({route.source_id for route in descriptors}) != len(descriptors):
            raise ArgumentError("evidence reconciliation source IDs must be unique")
        resolved_quorum = 1 if len(descriptors) == 1 else 2 if quorum is None else quorum
        resolved_concurrency = min(len(descriptors), MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_CONCURRENCY) if max_concurrency is None else max_concurrency
        if normalizer_id == "identity" and normalizer_version != "1":
            raise ArgumentError("identity normalizer version must be 1")
        if not isinstance(require_all, bool):
            raise ArgumentError("evidence reconciliation require_all must be boolean")
        return AutonomousEvidenceReconciliationPlan.create(
            evidence_plan=self.evidence_plan,
            requirement=requirement,
            routes=descriptors,
            quorum=resolved_quorum,
            max_concurrency=resolved_concurrency,
            require_all=require_all,
            normalizer_id=_identifier("evidence reconciliation normalizer_id", normalizer_id),
            normalizer_version=_identifier("evidence reconciliation normalizer_version", normalizer_version),
            parent_evidence_digests=_parent_digests(parent_evidence_digests),
        )

    def execute(
        self,
        plan: AutonomousEvidenceReconciliationPlan,
        routes: Sequence[AutonomousEvidenceReconciliationRoute],
        *,
        approve_source_dispatch: bool = False,
        normalizer: Callable[[Any, Mapping[str, Any]], Any] | None = None,
        normalizer_id: str | None = None,
        normalizer_version: str | None = None,
    ) -> AutonomousEvidenceReconciliationResult:
        if not isinstance(plan, AutonomousEvidenceReconciliationPlan):
            raise ArgumentError("evidence reconciliation execute requires a typed plan")
        plan.verify(self.evidence_plan)
        if approve_source_dispatch is not True:
            raise ArgumentError("evidence reconciliation source dispatch requires explicit approval")
        if isinstance(routes, (str, bytes, bytearray)) or not isinstance(routes, Sequence) or len(routes) != len(plan.routes):
            raise ArgumentError("evidence reconciliation execution routes do not match its plan")
        descriptors = tuple(sorted((route.descriptor() for route in routes), key=lambda item: item.source_id))
        planned = tuple(sorted(plan.routes, key=lambda item: item.source_id))
        if any(
            descriptor.source_id != expected.source_id
            or descriptor.source_digest != expected.source_digest
            or descriptor.request_id != expected.request_id
            or descriptor.metadata_digest != expected.metadata_digest
            for descriptor, expected in zip(descriptors, planned)
        ):
            raise ArgumentError("evidence reconciliation execution route changed after planning")
        resolved_normalizer_id = plan.normalizer_id if normalizer_id is None else _identifier("evidence reconciliation normalizer_id", normalizer_id)
        resolved_normalizer_version = plan.normalizer_version if normalizer_version is None else _identifier("evidence reconciliation normalizer_version", normalizer_version)
        if resolved_normalizer_id != plan.normalizer_id or resolved_normalizer_version != plan.normalizer_version:
            raise ArgumentError("evidence reconciliation normalizer contract changed after planning")
        if plan.normalizer_id != "identity" and not callable(normalizer):
            raise ArgumentError("evidence reconciliation requires the planned normalizer callback")
        if normalizer is not None and not callable(normalizer):
            raise ArgumentError("evidence reconciliation normalizer is malformed")
        route_by_id = {route.source_id: route for route in routes}
        requirement = _requirement_for(self.evidence_plan, plan.requirement_id)
        with ThreadPoolExecutor(max_workers=plan.max_concurrency, thread_name_prefix="aurora-evidence-reconcile") as executor:
            futures = [executor.submit(self._execute_one, plan, requirement, descriptor, route_by_id[descriptor.source_id], normalizer) for descriptor in descriptors]
            executions = tuple(future.result() for future in futures)
        executions = tuple(sorted(executions, key=lambda item: item.descriptor.source_id))
        observed = tuple(item for item in executions if item.status == "observed" and item.normalized is not None)
        failed = tuple(item for item in executions if item.status == "failed")
        groups: dict[str, dict[str, Any]] = {}
        for execution in observed:
            digest = execution.result.normalized_digest
            if digest is None:
                continue
            group = groups.setdefault(digest, {"count": 0, "source_ids": []})
            group["count"] += 1
            group["source_ids"].append(execution.descriptor.source_id)
        ranked = sorted(groups.items(), key=lambda item: (-item[1]["count"], item[0]))
        winner = ranked[0] if ranked else None
        if not observed:
            status = "failed"
        elif plan.require_all and failed:
            status = "insufficient_evidence"
        elif len(observed) < plan.quorum:
            status = "insufficient_evidence"
        elif winner is None or winner[1]["count"] < plan.quorum:
            status = "disagreement" if len(groups) > 1 else "insufficient_evidence"
        else:
            status = "consensus_with_dissent" if len(groups) > 1 else "consensus"
        group_projection = [
            {"normalized_digest": digest, "count": group["count"], "source_ids": sorted(group["source_ids"])}
            for digest, group in ranked
        ]
        values = {item.descriptor.source_id: item.value for item in executions if item.status == "observed"}
        normalized_values = {item.descriptor.source_id: item.normalized for item in executions if item.status == "observed"}
        return AutonomousEvidenceReconciliationResult.build(
            evidence_plan_digest=self.evidence_plan.plan_digest,
            requirement_id=plan.requirement_id,
            domain=plan.domain,
            reconciliation_plan_digest=plan.plan_digest,
            status=status,
            quorum=plan.quorum,
            consensus_normalized_digest=winner[0] if winner is not None and winner[1]["count"] >= plan.quorum else None,
            disagreement_digest=content_digest(group_projection) if len(groups) > 1 else None,
            source_results=tuple(item.result for item in executions),
            values=values,
            normalized_values=normalized_values,
        )

    def _execute_one(
        self,
        plan: AutonomousEvidenceReconciliationPlan,
        requirement: AutonomousEvidenceRequirement,
        descriptor: AutonomousEvidenceReconciliationRouteDescriptor,
        route: AutonomousEvidenceReconciliationRoute,
        normalizer: Callable[[Any, Mapping[str, Any]], Any] | None,
    ) -> _SourceExecution:
        request_digest = _request_digest(self.evidence_plan.plan_digest, plan.requirement_id, descriptor)
        context = {
            "plan_digest": self.evidence_plan.plan_digest,
            "requirement": requirement,
            "request": {
                "requirement_id": plan.requirement_id,
                "source_id": descriptor.source_id,
                "source_digest": descriptor.source_digest,
                "request_id": descriptor.request_id,
                "metadata": descriptor.metadata,
            },
            "attempt": 1,
            "parent_evidence_digests": list(plan.parent_evidence_digests),
            "execution": "caller_owned_adapter;raw_value_transient",
        }
        try:
            value, value_bytes = _safe_value(route.acquirer.acquire(context), "evidence reconciliation acquired value")
            normalized = value if plan.normalizer_id == "identity" else normalizer(value, context)  # type: ignore[misc]
            normalized, _normalized_bytes = _safe_value(normalized, "evidence reconciliation normalized value")
            result = AutonomousEvidenceReconciliationSourceResult(
                source_id=descriptor.source_id,
                source_digest=descriptor.source_digest,
                request_id=descriptor.request_id,
                request_digest=request_digest,
                metadata_digest=descriptor.metadata_digest,
                status="observed",
                value_digest=content_digest(value),
                value_bytes=value_bytes,
                normalized_digest=content_digest(normalized),
                failure_class=None,
                retryable=False,
                limitations=(),
            )
            return _SourceExecution(descriptor, request_digest, "observed", value, normalized, result)
        except Exception as error:
            classification = classify_autonomous_evidence_acquisition_error(error)
            result = AutonomousEvidenceReconciliationSourceResult(
                source_id=descriptor.source_id,
                source_digest=descriptor.source_digest,
                request_id=descriptor.request_id,
                request_digest=request_digest,
                metadata_digest=descriptor.metadata_digest,
                status="failed",
                value_digest=None,
                value_bytes=0,
                normalized_digest=None,
                failure_class=classification.failure_class,
                retryable=classification.retryable,
                limitations=("source acquisition or normalization failed",),
            )
            return _SourceExecution(descriptor, request_digest, "failed", None, None, result)


def create_autonomous_evidence_source_reconciler(evidence_plan: AutonomousEvidencePlan) -> AutonomousEvidenceSourceReconciler:
    return AutonomousEvidenceSourceReconciler(evidence_plan)


__all__ = [
    "AUTONOMOUS_EVIDENCE_RECONCILIATION_PLAN_SCHEMA",
    "AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_SCHEMA",
    "AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_SCHEMA",
    "MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_ROUTES",
    "MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_CONCURRENCY",
    "MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_METADATA_BYTES",
    "MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_VALUE_BYTES",
    "MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_BYTES",
    "MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_PARENT_DIGESTS",
    "AUTONOMOUS_EVIDENCE_RECONCILIATION_STATUSES",
    "AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_STATUSES",
    "AutonomousEvidenceReconciliationRouteDescriptor",
    "AutonomousEvidenceReconciliationRoute",
    "AutonomousEvidenceReconciliationRouteProjection",
    "AutonomousEvidenceReconciliationPlan",
    "AutonomousEvidenceReconciliationSourceResult",
    "AutonomousEvidenceReconciliationResult",
    "AutonomousEvidenceSourceReconciler",
    "create_autonomous_evidence_source_reconciler",
]
