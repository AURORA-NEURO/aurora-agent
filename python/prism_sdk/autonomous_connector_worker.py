"""Durable, metadata-only connector work execution for the Python autonomous SDK.

The connector runtime in :mod:`autonomous_connectors` deliberately owns only a transient
caller-provided executor.  This module adds the application-side worker boundary that was
previously left to each embedding service:

* one explicit operation contract covers each built-in autonomous domain;
* the queue persists identities, digests, leases, and bounded failure classes only;
* a worker must rehydrate the exact selection plan and request from caller-owned storage;
* replay returns a receipt without a connector value; and
* evaluator feedback is explicit and can never be inferred from transport status.

The in-memory implementations are intentionally dependency-free.  Applications can persist
their verified snapshots in SQLite, Postgres, an object store, or another transactional adapter.
No class in this module discovers providers, opens a network connection, accepts a raw key, or
retains a prompt, request, plan, response, or credential.
"""

from __future__ import annotations

from dataclasses import dataclass, field, replace
import json
import math
import threading
import time
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .autonomous_connectors import (
    AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES,
    AutonomousConnectorDispatchReceipt,
    AutonomousConnectorDispatchRequest,
    AutonomousConnectorRegistry,
    AutonomousConnectorRuntime,
    AutonomousConnectorSelectionPlan,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES, _identifier
from .errors import ArgumentError


AUTONOMOUS_CONNECTOR_OPERATION_REGISTRY_SCHEMA = "bioprism-python-autonomous-connector-operation-registry/0.1"
AUTONOMOUS_CONNECTOR_OPERATION_SCHEMA = "bioprism-python-autonomous-connector-operation/0.1"
AUTONOMOUS_CONNECTOR_WORK_ITEM_SCHEMA = "bioprism-python-autonomous-connector-work-item/0.1"
AUTONOMOUS_CONNECTOR_WORK_QUEUE_SCHEMA = "bioprism-python-autonomous-connector-work-queue/0.1"
AUTONOMOUS_CONNECTOR_WORKER_SCHEMA = "bioprism-python-autonomous-connector-worker/0.1"
AUTONOMOUS_CONNECTOR_FEEDBACK_SCHEMA = "bioprism-python-autonomous-connector-feedback/0.1"
AUTONOMOUS_CONNECTOR_FEEDBACK_LEDGER_SCHEMA = "bioprism-python-autonomous-connector-feedback-ledger/0.1"

MAX_AUTONOMOUS_CONNECTOR_OPERATIONS = 128
MAX_AUTONOMOUS_CONNECTOR_WORK_ITEMS = 4_096
MAX_AUTONOMOUS_CONNECTOR_WORK_ATTEMPTS = 32
MAX_AUTONOMOUS_CONNECTOR_WORK_BATCH = 128
MAX_AUTONOMOUS_CONNECTOR_WORK_LEASE_MS = 600_000
MAX_AUTONOMOUS_CONNECTOR_WORK_SNAPSHOT_BYTES = 8_000_000
MAX_AUTONOMOUS_CONNECTOR_FEEDBACK_ENTRIES = 20_000
MAX_AUTONOMOUS_CONNECTOR_FEEDBACK_SNAPSHOT_BYTES = 8_000_000

_WORK_STATUSES = frozenset({
    "queued", "leased", "completed", "failed", "reconciliation_required", "cancelled",
})
_WORK_FAILURE_CLASSES = frozenset({
    None,
    "rehydration_missing",
    "rehydration_invalid",
    "identity_conflict",
    "lease_expired",
    "approval_required",
    "domain_scope",
    "capability_scope",
    "executor_error",
    "transport_error",
    "unknown",
})
_WORK_ITEM_KEYS = frozenset({
    "schema", "work_id", "operation_id", "operation_digest", "domain", "capability", "connector_id",
    "selection_plan_digest", "request_digest", "dispatch_id", "execution_id", "call_id", "attempt_id",
    "parent_digests", "approved", "max_attempts", "attempts", "status", "available_at", "lease_owner",
    "lease_until", "receipt_digest", "payload_digest", "failure_class", "last_error_class", "created_at",
    "updated_at", "item_digest", "retention", "secret_material",
})


def _capability_identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > 256:
        raise ArgumentError(f"{name} must be a bounded capability identifier")
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+-" for character in value):
        raise ArgumentError(f"{name} must be a bounded capability identifier")
    return value


def _capability_sequence(name: str, value: Any, *, maximum: int) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be a sequence")
    if not value or len(value) > maximum:
        raise ArgumentError(f"{name} must contain between 1 and {maximum} entries")
    result: list[str] = []
    seen: set[str] = set()
    for item in value:
        if not isinstance(item, str) or not item.strip() or "\x00" in item or len(item.encode("utf-8")) > 256:
            raise ArgumentError(f"{name} entry must be a bounded capability identifier")
        if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+-" for character in item):
            raise ArgumentError(f"{name} entry must be a bounded capability identifier")
        if item in seen:
            raise ArgumentError(f"{name} contains a duplicate entry: {item}")
        seen.add(item)
        result.append(item)
    return tuple(result)


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _bounded_timestamp(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 8_640_000_000_000_000:
        raise ArgumentError(f"{name} must be a bounded epoch millisecond timestamp")
    return value


def _bounded_integer(name: str, value: Any, *, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise ArgumentError(f"{name} must be an integer between {minimum} and {maximum}")
    return value


def _now_ms(value: int | None = None) -> int:
    return _bounded_timestamp("time", int(time.time() * 1000) if value is None else value)


def _digest_sequence(name: str, value: Any, *, maximum: int = 128) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)) or len(value) > maximum:
        raise ArgumentError(f"{name} must contain at most {maximum} entries")
    return tuple(_digest(f"{name} entry", item) for item in value)  # type: ignore[misc]


@dataclass(frozen=True, slots=True)
class AutonomousConnectorOperationContract:
    """Bounded operation vocabulary for one built-in autonomous domain."""

    operation_id: str
    domain: str
    capabilities: tuple[str, ...]
    description: str
    evaluator_signals: tuple[str, ...]
    request_fields: tuple[str, ...] = ("operation_id",)
    risk_class: str = "read_only"
    operation_digest: str = field(init=False)

    def __post_init__(self) -> None:
        operation_id = _identifier("autonomous connector operation_id", self.operation_id)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("autonomous connector operation domain is unsupported")
        capabilities = _capability_sequence("autonomous connector operation capabilities", self.capabilities, maximum=128)
        if not isinstance(self.description, str) or not self.description.strip() or "\x00" in self.description or len(self.description.encode("utf-8")) > 1_024:
            raise ArgumentError("autonomous connector operation description is outside its bound")
        request_fields = tuple(_identifier("autonomous connector operation request field", item) for item in self.request_fields)
        if not request_fields or "operation_id" not in request_fields or len(set(request_fields)) != len(request_fields):
            raise ArgumentError("autonomous connector operation request_fields must contain operation_id without duplicates")
        evaluator_signals = tuple(_identifier("autonomous connector operation evaluator signal", item) for item in self.evaluator_signals)
        if not evaluator_signals or len(set(evaluator_signals)) != len(evaluator_signals):
            raise ArgumentError("autonomous connector operation evaluator_signals must be non-empty and unique")
        if self.risk_class not in {"read_only", "side_effecting", "human_review"}:
            raise ArgumentError("autonomous connector operation risk_class is invalid")
        object.__setattr__(self, "operation_id", operation_id)
        object.__setattr__(self, "capabilities", capabilities)
        object.__setattr__(self, "request_fields", request_fields)
        object.__setattr__(self, "evaluator_signals", evaluator_signals)
        payload = self._payload()
        object.__setattr__(self, "operation_digest", content_digest(payload))

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_OPERATION_SCHEMA,
            "operation_id": self.operation_id,
            "domain": self.domain,
            "capabilities": list(self.capabilities),
            "description": self.description,
            "request_fields": list(self.request_fields),
            "evaluator_signals": list(self.evaluator_signals),
            "risk_class": self.risk_class,
        }

    def supports(self, capability: str) -> bool:
        return _capability_identifier("autonomous connector operation capability", capability) in self.capabilities

    def assert_request(self, request: AutonomousConnectorDispatchRequest) -> None:
        if not isinstance(request, AutonomousConnectorDispatchRequest):
            raise ArgumentError("autonomous connector operation request must be typed")
        if len(request.domains) != 1 or request.domains[0] != self.domain:
            raise ArgumentError("autonomous connector operation request must target exactly its contract domain")
        if not self.supports(request.capability):
            raise ArgumentError("autonomous connector operation capability is outside the contract")
        for field_name in self.request_fields:
            if field_name not in request.request:
                raise ArgumentError(f"autonomous connector operation request is missing {field_name}")
        if request.request.get("operation_id") != self.operation_id:
            raise ArgumentError("autonomous connector operation request operation_id does not match the contract")

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "operation_digest": self.operation_digest,
            "retention": "metadata_only_contract_no_request_values",
            "secret_material": "never_returned",
        }


def _caps(*values: str) -> tuple[str, ...]:
    return tuple(dict.fromkeys(values))


def default_autonomous_connector_operation_contracts() -> tuple[AutonomousConnectorOperationContract, ...]:
    """Return one explicit operation contract for every built-in autonomous domain."""

    return (
        AutonomousConnectorOperationContract("coding.repository_change_analysis", "coding", _caps("review", "debugging", "implementation", "testing", "review+debugging", "review+implementation", "review+testing"), "Inspect repository state and return caller-owned change observations.", ("correctness", "testability", "reproducibility")),
        AutonomousConnectorOperationContract("browser.web_evidence_retrieval", "browser", _caps("web_research", "navigation", "source_comparison", "web_research+navigation", "web_research+source_comparison"), "Acquire bounded web evidence through a caller-managed browser connector.", ("source_quality", "citation_completeness", "freshness")),
        AutonomousConnectorOperationContract("data.dataset_quality_profile", "data", _caps("schema_validation", "lineage", "quality_control", "data_analysis", "quality_control+data_analysis", "data_analysis+schema_validation"), "Profile a caller-owned dataset and expose quality or lineage observations.", ("schema_validity", "lineage_completeness", "quality")),
        AutonomousConnectorOperationContract("science.reproducible_evidence_acquisition", "science", _caps("hypothesis", "literature", "statistics", "experiment", "reproducibility", "hypothesis+statistics", "experiment+statistics"), "Acquire and align scientific evidence under explicit reproducibility boundaries.", ("evidence_strength", "reproducibility", "uncertainty")),
        AutonomousConnectorOperationContract("biomedical.clinical_data_review", "biomedical", _caps("biomedical_review", "safety_boundary", "provenance", "human_review", "biomedical_review+safety_boundary"), "Review biomedical evidence with provenance, safety, and human-review boundaries.", ("safety", "provenance", "review_completeness"), risk_class="human_review"),
        AutonomousConnectorOperationContract("neuroscience.signal_study_analysis", "neuroscience", _caps("neuroscience_analysis", "signal_interpretation", "study_design", "reproducibility", "neuroscience_analysis+signal_interpretation", "study_design+reproducibility"), "Analyze neuroscience signal or study evidence without retaining raw participant data.", ("signal_quality", "study_design", "reproducibility")),
        AutonomousConnectorOperationContract("operations.incident_runbook_observation", "operations", _caps("observability", "incident_response", "risk_review", "rollback", "approval", "runbook", "observability+incident_response"), "Observe operational incidents and runbooks while leaving mutation authorization to the caller.", ("incident_completeness", "risk_containment", "runbook_alignment"), risk_class="side_effecting"),
        AutonomousConnectorOperationContract("enterprise.workflow_record_governance", "enterprise", _caps("workflow", "coordination", "governance", "compliance", "analytics", "workflow+coordination", "governance+compliance", "analytics+governance", "governance+analytics"), "Inspect enterprise workflow records for governance, compliance, and coordination evidence.", ("policy_compliance", "workflow_integrity", "record_completeness")),
        AutonomousConnectorOperationContract("multi_agent.delegated_consensus_handoff", "multi_agent", _caps("delegation", "coordination", "consensus", "conflict_resolution", "handoff", "delegation+coordination", "consensus+conflict_resolution", "handoff+coordination"), "Coordinate delegated agent evidence and handoffs without granting implicit authority.", ("delegation_quality", "consensus", "handoff_integrity")),
        AutonomousConnectorOperationContract("multimodal.asset_alignment", "multimodal", _caps("document", "cross_modal_alignment", "image", "audio", "video", "document+cross_modal_alignment", "image+audio+video+document"), "Align caller-owned document, image, audio, and video observations by digest.", ("modality_support", "alignment_quality", "comparability")),
        AutonomousConnectorOperationContract("cross_domain.evidence_fanout_synthesis", "cross_domain", _caps("routing", "synthesis", "evidence_alignment", "workflow_composition", "routing+synthesis"), "Fan out bounded evidence work and synthesize cross-domain metadata.", ("coverage", "alignment", "synthesis_quality")),
        AutonomousConnectorOperationContract("evaluation.benchmark_replay_analysis", "evaluation", _caps("rubric", "benchmarking", "replay", "failure_analysis", "reproducibility"), "Run evaluator-owned benchmark and replay analysis over metadata-only outcomes.", ("benchmark_integrity", "replay_fidelity", "failure_coverage")),
    )


class AutonomousConnectorOperationRegistry:
    """Digest-addressed operation catalogue that must cover all twelve domains."""

    def __init__(self, contracts: Sequence[AutonomousConnectorOperationContract] | None = None) -> None:
        values = tuple(default_autonomous_connector_operation_contracts() if contracts is None else contracts)
        if not 1 <= len(values) <= MAX_AUTONOMOUS_CONNECTOR_OPERATIONS:
            raise ArgumentError("autonomous connector operation registry size is outside its bound")
        self._contracts: dict[str, AutonomousConnectorOperationContract] = {}
        for contract in values:
            self._add(contract)
        self._assert_coverage()

    def _add(self, contract: AutonomousConnectorOperationContract, *, replace_existing: bool = False) -> None:
        if not isinstance(contract, AutonomousConnectorOperationContract):
            raise ArgumentError("autonomous connector operation contract is invalid")
        if contract.operation_id in self._contracts and not replace_existing:
            raise ArgumentError(f"autonomous connector operation is already registered: {contract.operation_id}")
        if contract.operation_id not in self._contracts and len(self._contracts) >= MAX_AUTONOMOUS_CONNECTOR_OPERATIONS:
            raise ArgumentError("autonomous connector operation registry is full")
        self._contracts[contract.operation_id] = contract

    def _assert_coverage(self) -> None:
        domains = {contract.domain for contract in self._contracts.values()}
        if domains != set(AUTONOMOUS_DOMAIN_NAMES):
            raise ArgumentError("autonomous connector operation registry must cover every autonomous domain")

    def register(self, contract: AutonomousConnectorOperationContract, *, replace: bool = False) -> AutonomousConnectorOperationContract:
        previous = self._contracts.get(contract.operation_id) if isinstance(contract, AutonomousConnectorOperationContract) else None
        self._add(contract, replace_existing=replace)
        try:
            self._assert_coverage()
        except Exception:
            if previous is None:
                self._contracts.pop(contract.operation_id, None)
            else:
                self._contracts[previous.operation_id] = previous
            raise
        return contract

    def resolve(self, operation_id: str) -> AutonomousConnectorOperationContract:
        normalized = _identifier("autonomous connector operation_id", operation_id)
        contract = self._contracts.get(normalized)
        if contract is None:
            raise ArgumentError(f"autonomous connector operation is not registered: {normalized}")
        return contract

    def operations(self) -> tuple[AutonomousConnectorOperationContract, ...]:
        return tuple(self._contracts[key] for key in sorted(self._contracts))

    def for_domain(self, domain: str) -> tuple[AutonomousConnectorOperationContract, ...]:
        if domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("autonomous connector operation domain is unsupported")
        return tuple(contract for contract in self.operations() if contract.domain == domain)

    @property
    def digest(self) -> str:
        return content_digest([contract.to_dict() for contract in self.operations()])

    def assert_request(self, operation_id: str, request: AutonomousConnectorDispatchRequest) -> AutonomousConnectorOperationContract:
        contract = self.resolve(operation_id)
        contract.assert_request(request)
        return contract

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_OPERATION_REGISTRY_SCHEMA,
            "digest": self.digest,
            "operations": [contract.to_dict() for contract in self.operations()],
            "operation_count": len(self._contracts),
            "coverage": {domain: [contract.operation_id for contract in self.for_domain(domain)] for domain in AUTONOMOUS_DOMAIN_NAMES},
            "retention": "metadata_only_contract_catalogue",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorWorkItem:
    """One queue row containing no raw request, plan, or connector value."""

    work_id: str
    operation_id: str
    operation_digest: str
    domain: str
    capability: str
    connector_id: str
    selection_plan_digest: str
    request_digest: str
    dispatch_id: str
    execution_id: str
    call_id: str
    attempt_id: str | None
    parent_digests: tuple[str, ...]
    approved: bool
    max_attempts: int
    attempts: int
    status: str
    available_at: int
    lease_owner: str | None
    lease_until: int | None
    receipt_digest: str | None
    payload_digest: str | None
    failure_class: str | None
    last_error_class: str | None
    created_at: int
    updated_at: int
    item_digest: str = field(default="")

    def __post_init__(self) -> None:
        for name, value in (("work_id", self.work_id), ("operation_id", self.operation_id), ("capability", self.capability), ("connector_id", self.connector_id), ("dispatch_id", self.dispatch_id), ("execution_id", self.execution_id), ("call_id", self.call_id)):
            _identifier(f"autonomous connector work {name}", value)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("autonomous connector work domain is unsupported")
        _capability_identifier("autonomous connector work capability", self.capability)
        for name, value in (("operation_digest", self.operation_digest), ("selection_plan_digest", self.selection_plan_digest), ("request_digest", self.request_digest)):
            _digest(f"autonomous connector work {name}", value)
        if self.item_digest:
            _digest("autonomous connector work item_digest", self.item_digest)
        _digest("autonomous connector work receipt_digest", self.receipt_digest, allow_none=True)
        _digest("autonomous connector work payload_digest", self.payload_digest, allow_none=True)
        if self.attempt_id is not None:
            _identifier("autonomous connector work attempt_id", self.attempt_id)
        parents = _digest_sequence("autonomous connector work parent_digests", self.parent_digests)
        if not isinstance(self.approved, bool):
            raise ArgumentError("autonomous connector work approved must be boolean")
        max_attempts = _bounded_integer("autonomous connector work max_attempts", self.max_attempts, minimum=1, maximum=MAX_AUTONOMOUS_CONNECTOR_WORK_ATTEMPTS)
        attempts = _bounded_integer("autonomous connector work attempts", self.attempts, minimum=0, maximum=MAX_AUTONOMOUS_CONNECTOR_WORK_ATTEMPTS)
        if attempts > max_attempts or self.status not in _WORK_STATUSES:
            raise ArgumentError("autonomous connector work attempt or status is invalid")
        available_at = _bounded_timestamp("autonomous connector work available_at", self.available_at)
        created_at = _bounded_timestamp("autonomous connector work created_at", self.created_at)
        updated_at = _bounded_timestamp("autonomous connector work updated_at", self.updated_at)
        if self.lease_owner is not None:
            _identifier("autonomous connector work lease_owner", self.lease_owner)
        if self.lease_until is not None:
            _bounded_timestamp("autonomous connector work lease_until", self.lease_until)
        if self.status == "leased":
            if self.lease_owner is None or self.lease_until is None:
                raise ArgumentError("autonomous connector work leased state requires an owner and expiry")
        elif self.lease_owner is not None or self.lease_until is not None:
            raise ArgumentError("autonomous connector work non-leased state cannot retain a lease")
        if self.failure_class not in _WORK_FAILURE_CLASSES or self.last_error_class not in _WORK_FAILURE_CLASSES:
            raise ArgumentError("autonomous connector work failure class is invalid")
        object.__setattr__(self, "parent_digests", parents)
        object.__setattr__(self, "available_at", available_at)
        object.__setattr__(self, "created_at", created_at)
        object.__setattr__(self, "updated_at", updated_at)
        if not self.item_digest:
            object.__setattr__(self, "item_digest", self.computed_digest)

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_WORK_ITEM_SCHEMA,
            "work_id": self.work_id,
            "operation_id": self.operation_id,
            "operation_digest": self.operation_digest,
            "domain": self.domain,
            "capability": self.capability,
            "connector_id": self.connector_id,
            "selection_plan_digest": self.selection_plan_digest,
            "request_digest": self.request_digest,
            "dispatch_id": self.dispatch_id,
            "execution_id": self.execution_id,
            "call_id": self.call_id,
            "attempt_id": self.attempt_id,
            "parent_digests": list(self.parent_digests),
            "approved": self.approved,
            "max_attempts": self.max_attempts,
            "attempts": self.attempts,
            "status": self.status,
            "available_at": self.available_at,
            "lease_owner": self.lease_owner,
            "lease_until": self.lease_until,
            "receipt_digest": self.receipt_digest,
            "payload_digest": self.payload_digest,
            "failure_class": self.failure_class,
            "last_error_class": self.last_error_class,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "retention": "metadata_only_request_plan_and_payload_not_retained",
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "item_digest": self.item_digest}

    @property
    def computed_digest(self) -> str:
        return content_digest(self._payload())


def _work_item_from_mapping(value: Mapping[str, Any], operation_registry: AutonomousConnectorOperationRegistry) -> AutonomousConnectorWorkItem:
    if not isinstance(value, Mapping) or set(value) != _WORK_ITEM_KEYS:
        raise ArgumentError("autonomous connector work item is malformed")
    if value.get("retention") != "metadata_only_request_plan_and_payload_not_retained" or value.get("secret_material") != "never_returned":
        raise ArgumentError("autonomous connector work item retention is invalid")
    raw_parent_digests = value.get("parent_digests")
    if not isinstance(raw_parent_digests, Sequence) or isinstance(raw_parent_digests, (str, bytes)):
        raise ArgumentError("autonomous connector work parent_digests must be a sequence")
    item = AutonomousConnectorWorkItem(
        work_id=value.get("work_id"), operation_id=value.get("operation_id"), operation_digest=value.get("operation_digest"), domain=value.get("domain"), capability=value.get("capability"), connector_id=value.get("connector_id"), selection_plan_digest=value.get("selection_plan_digest"), request_digest=value.get("request_digest"), dispatch_id=value.get("dispatch_id"), execution_id=value.get("execution_id"), call_id=value.get("call_id"), attempt_id=value.get("attempt_id"), parent_digests=tuple(raw_parent_digests), approved=value.get("approved"), max_attempts=value.get("max_attempts"), attempts=value.get("attempts"), status=value.get("status"), available_at=value.get("available_at"), lease_owner=value.get("lease_owner"), lease_until=value.get("lease_until"), receipt_digest=value.get("receipt_digest"), payload_digest=value.get("payload_digest"), failure_class=value.get("failure_class"), last_error_class=value.get("last_error_class"), created_at=value.get("created_at"), updated_at=value.get("updated_at"), item_digest=value.get("item_digest"),
    )
    contract = operation_registry.resolve(item.operation_id)
    if contract.operation_digest != item.operation_digest or contract.domain != item.domain or not contract.supports(item.capability):
        raise ArgumentError("autonomous connector work operation identity is stale or invalid")
    if item.item_digest != item.computed_digest:
        raise ArgumentError("autonomous connector work item digest is invalid")
    return item


class InMemoryAutonomousConnectorWorkQueue:
    """Thread-safe, metadata-only queue with lease fencing and bounded retry."""

    def __init__(self, operation_registry: AutonomousConnectorOperationRegistry | None = None, *, max_items: int = MAX_AUTONOMOUS_CONNECTOR_WORK_ITEMS) -> None:
        if isinstance(max_items, bool) or not isinstance(max_items, int) or not 1 <= max_items <= MAX_AUTONOMOUS_CONNECTOR_WORK_ITEMS:
            raise ArgumentError("autonomous connector work queue max_items is outside its bound")
        self.operation_registry = operation_registry or AutonomousConnectorOperationRegistry()
        if not isinstance(self.operation_registry, AutonomousConnectorOperationRegistry):
            raise ArgumentError("autonomous connector work queue requires an operation registry")
        self.max_items = max_items
        self._items: dict[str, AutonomousConnectorWorkItem] = {}
        self._lock = threading.RLock()

    @staticmethod
    def _refresh(item: AutonomousConnectorWorkItem, now: int, **updates: Any) -> AutonomousConnectorWorkItem:
        candidate = replace(item, **updates, updated_at=now, item_digest="")
        return replace(candidate, item_digest=candidate.computed_digest)

    def enqueue(self, *, work_id: str, operation_id: str, request: AutonomousConnectorDispatchRequest, selection_plan_digest: str | None = None, max_attempts: int = 3, available_at: int | None = None, now: int | None = None) -> AutonomousConnectorWorkItem:
        work_id = _identifier("autonomous connector work_id", work_id)
        if not isinstance(request, AutonomousConnectorDispatchRequest):
            raise ArgumentError("autonomous connector work enqueue requires a typed request")
        contract = self.operation_registry.assert_request(operation_id, request)
        selection_digest = _digest("autonomous connector work selection_plan_digest", request.selection_plan_digest if selection_plan_digest is None else selection_plan_digest)
        if request.selection_plan_digest != selection_digest:
            raise ArgumentError("autonomous connector work selection plan digest does not match request")
        current = _now_ms(now)
        max_attempts = _bounded_integer("autonomous connector work max_attempts", max_attempts, minimum=1, maximum=MAX_AUTONOMOUS_CONNECTOR_WORK_ATTEMPTS)
        with self._lock:
            existing = self._items.get(work_id)
            if existing is not None:
                if existing.operation_id != contract.operation_id or existing.request_digest != request.request_digest or existing.selection_plan_digest != selection_digest:
                    raise ArgumentError("autonomous connector work identity conflicts with an existing work item")
                return existing
            if len(self._items) >= self.max_items:
                raise ArgumentError("autonomous connector work queue is full")
            available = _bounded_timestamp("autonomous connector work available_at", current if available_at is None else available_at)
            item = AutonomousConnectorWorkItem(
                work_id=work_id,
                operation_id=contract.operation_id,
                operation_digest=contract.operation_digest,
                domain=contract.domain,
                capability=request.capability,
                connector_id=request.connector_id,
                selection_plan_digest=selection_digest,
                request_digest=request.request_digest,
                dispatch_id=request.dispatch_id,
                execution_id=request.execution_id,
                call_id=request.call_id,
                attempt_id=request.attempt_id,
                parent_digests=request.parent_digests,
                approved=request.approved,
                max_attempts=max_attempts,
                attempts=0,
                status="queued",
                available_at=available,
                lease_owner=None,
                lease_until=None,
                receipt_digest=None,
                payload_digest=None,
                failure_class=None,
                last_error_class=None,
                created_at=current,
                updated_at=current,
            )
            item = replace(item, item_digest=item.computed_digest)
            self._items[work_id] = item
            return item

    def get(self, work_id: str) -> AutonomousConnectorWorkItem | None:
        with self._lock:
            return self._items.get(_identifier("autonomous connector work_id", work_id))

    def pending(self, *, limit: int = 64, now: int | None = None) -> tuple[AutonomousConnectorWorkItem, ...]:
        current = _now_ms(now)
        limit = _bounded_integer("autonomous connector work pending limit", limit, minimum=1, maximum=min(MAX_AUTONOMOUS_CONNECTOR_WORK_BATCH, self.max_items))
        with self._lock:
            values = [item for item in self._items.values() if ((item.status == "queued" and item.available_at <= current and item.attempts < item.max_attempts) or (item.status == "leased" and item.lease_until is not None and item.lease_until <= current and item.attempts < item.max_attempts))]
        return tuple(sorted(values, key=lambda item: (item.available_at, item.created_at, item.work_id))[:limit])

    def claim(self, work_id: str, worker_id: str, *, lease_ms: int = 30_000, now: int | None = None) -> AutonomousConnectorWorkItem | None:
        work_id = _identifier("autonomous connector work_id", work_id)
        worker_id = _identifier("autonomous connector worker_id", worker_id)
        lease_ms = _bounded_integer("autonomous connector work lease_ms", lease_ms, minimum=1, maximum=MAX_AUTONOMOUS_CONNECTOR_WORK_LEASE_MS)
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status in {"completed", "failed", "reconciliation_required", "cancelled"}:
                return None
            if item.status == "leased" and item.lease_until is not None and item.lease_until > current:
                return None
            if item.attempts >= item.max_attempts:
                expired = self._refresh(item, current, status="reconciliation_required", failure_class="lease_expired", last_error_class="lease_expired", lease_owner=None, lease_until=None)
                self._items[work_id] = expired
                return None
            claimed = self._refresh(item, current, status="leased", attempts=item.attempts + 1, lease_owner=worker_id, lease_until=current + lease_ms, last_error_class=None)
            self._items[work_id] = claimed
            return claimed

    def renew(self, work_id: str, worker_id: str, *, lease_ms: int = 30_000, now: int | None = None) -> AutonomousConnectorWorkItem:
        work_id = _identifier("autonomous connector work_id", work_id)
        worker_id = _identifier("autonomous connector worker_id", worker_id)
        lease_ms = _bounded_integer("autonomous connector work lease_ms", lease_ms, minimum=1, maximum=MAX_AUTONOMOUS_CONNECTOR_WORK_LEASE_MS)
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status != "leased" or item.lease_owner != worker_id or item.lease_until is None or item.lease_until <= current:
                raise ArgumentError("autonomous connector work lease cannot be renewed by this worker")
            renewed = self._refresh(item, current, lease_until=current + lease_ms)
            self._items[work_id] = renewed
            return renewed

    def complete(self, work_id: str, worker_id: str, receipt: AutonomousConnectorDispatchReceipt, *, now: int | None = None) -> AutonomousConnectorWorkItem:
        work_id = _identifier("autonomous connector work_id", work_id)
        worker_id = _identifier("autonomous connector worker_id", worker_id)
        if not isinstance(receipt, AutonomousConnectorDispatchReceipt):
            raise ArgumentError("autonomous connector work completion requires a typed receipt")
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status != "leased" or item.lease_owner != worker_id or item.lease_until is None or item.lease_until <= current:
                raise ArgumentError("autonomous connector work completion is fenced by an expired or foreign lease")
            if (receipt.request_digest, receipt.dispatch_id, receipt.execution_id, receipt.call_id, receipt.connector_id) != (item.request_digest, item.dispatch_id, item.execution_id, item.call_id, item.connector_id):
                raise ArgumentError("autonomous connector work receipt identity conflicts with the work item")
            completed = self._refresh(item, current, status="completed", lease_owner=None, lease_until=None, receipt_digest=content_digest(receipt.to_dict()), payload_digest=receipt.payload_digest)
            self._items[work_id] = completed
            return completed

    def fail(self, work_id: str, worker_id: str, error_class: str, *, retryable: bool, now: int | None = None, receipt: AutonomousConnectorDispatchReceipt | None = None) -> AutonomousConnectorWorkItem:
        work_id = _identifier("autonomous connector work_id", work_id)
        worker_id = _identifier("autonomous connector worker_id", worker_id)
        if error_class not in _WORK_FAILURE_CLASSES or error_class is None:
            error_class = "unknown"
        if receipt is not None and not isinstance(receipt, AutonomousConnectorDispatchReceipt):
            raise ArgumentError("autonomous connector work failure receipt must be typed")
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status != "leased" or item.lease_owner != worker_id or item.lease_until is None or item.lease_until <= current:
                raise ArgumentError("autonomous connector work failure is fenced by an expired or foreign lease")
            can_retry = retryable and item.attempts < item.max_attempts
            delay = min(3_600_000, 1_000 * (2 ** max(0, item.attempts - 1)))
            failed = self._refresh(item, current, status="queued" if can_retry else "failed", available_at=current + delay if can_retry else item.available_at, lease_owner=None, lease_until=None, receipt_digest=item.receipt_digest if receipt is None else content_digest(receipt.to_dict()), payload_digest=item.payload_digest if receipt is None else receipt.payload_digest, failure_class=None if can_retry else error_class, last_error_class=error_class)
            self._items[work_id] = failed
            return failed

    def reconcile(self, work_id: str, worker_id: str, error_class: str = "rehydration_missing", *, now: int | None = None) -> AutonomousConnectorWorkItem:
        if error_class not in _WORK_FAILURE_CLASSES or error_class is None:
            error_class = "unknown"
        work_id = _identifier("autonomous connector work_id", work_id)
        worker_id = _identifier("autonomous connector worker_id", worker_id)
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status != "leased" or item.lease_owner != worker_id or item.lease_until is None or item.lease_until <= current:
                raise ArgumentError("autonomous connector reconciliation is fenced by an expired or foreign lease")
            reconciled = self._refresh(item, current, status="reconciliation_required", lease_owner=None, lease_until=None, failure_class=error_class, last_error_class=error_class)
            self._items[work_id] = reconciled
            return reconciled

    def cancel(self, work_id: str, reason: str = "unknown", *, now: int | None = None) -> AutonomousConnectorWorkItem:
        work_id = _identifier("autonomous connector work_id", work_id)
        if reason not in _WORK_FAILURE_CLASSES or reason is None:
            reason = "unknown"
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status in {"completed", "failed", "reconciliation_required", "cancelled"}:
                raise ArgumentError("autonomous connector work cannot be cancelled in its current state")
            cancelled = self._refresh(item, current, status="cancelled", lease_owner=None, lease_until=None, failure_class=reason, last_error_class=reason)
            self._items[work_id] = cancelled
            return cancelled

    def rows(self) -> tuple[AutonomousConnectorWorkItem, ...]:
        with self._lock:
            return tuple(sorted(self._items.values(), key=lambda item: (item.created_at, item.work_id)))

    def verify_integrity(self) -> dict[str, Any]:
        with self._lock:
            for item in self._items.values():
                if item.item_digest != item.computed_digest:
                    raise ArgumentError("autonomous connector work queue item digest is invalid")
                contract = self.operation_registry.resolve(item.operation_id)
                if contract.operation_digest != item.operation_digest or contract.domain != item.domain or not contract.supports(item.capability):
                    raise ArgumentError("autonomous connector work queue operation identity is invalid")
            count = len(self._items)
        return {"schema": AUTONOMOUS_CONNECTOR_WORK_QUEUE_SCHEMA, "verified": True, "items": count, "operation_registry_digest": self.operation_registry.digest, "retention": "metadata_only_request_plan_and_payload_not_retained", "secret_material": "never_returned"}

    def snapshot(self) -> dict[str, Any]:
        self.verify_integrity()
        descriptor = {"schema": AUTONOMOUS_CONNECTOR_WORK_QUEUE_SCHEMA, "operation_registry_digest": self.operation_registry.digest, "items": [item.to_dict() for item in self.rows()], "retention": "metadata_only_request_plan_and_payload_not_retained", "secret_material": "never_returned"}
        snapshot = {**descriptor, "snapshot_digest": content_digest(descriptor)}
        encoded = json.dumps(snapshot, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
        if len(encoded) > MAX_AUTONOMOUS_CONNECTOR_WORK_SNAPSHOT_BYTES:
            raise ArgumentError("autonomous connector work queue snapshot exceeds its bound")
        return snapshot

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        if not isinstance(snapshot, Mapping) or snapshot.get("schema") != AUTONOMOUS_CONNECTOR_WORK_QUEUE_SCHEMA or not isinstance(snapshot.get("items"), Sequence) or isinstance(snapshot.get("items"), (str, bytes)):
            raise ArgumentError("autonomous connector work queue snapshot is malformed")
        if snapshot.get("retention") != "metadata_only_request_plan_and_payload_not_retained" or snapshot.get("secret_material") != "never_returned":
            raise ArgumentError("autonomous connector work queue snapshot retention is invalid")
        if snapshot.get("operation_registry_digest") != self.operation_registry.digest:
            raise ArgumentError("autonomous connector work queue snapshot operation registry is stale")
        observed = snapshot.get("snapshot_digest")
        descriptor = {key: value for key, value in snapshot.items() if key != "snapshot_digest"}
        if _digest("autonomous connector work queue snapshot digest", observed) != content_digest(descriptor):
            raise ArgumentError("autonomous connector work queue snapshot digest is invalid")
        raw_items = tuple(snapshot.get("items"))
        if len(raw_items) > self.max_items:
            raise ArgumentError("autonomous connector work queue snapshot exceeds max_items")
        restored: dict[str, AutonomousConnectorWorkItem] = {}
        for raw in raw_items:
            if not isinstance(raw, Mapping):
                raise ArgumentError("autonomous connector work queue snapshot item is malformed")
            item = _work_item_from_mapping(raw, self.operation_registry)
            if item.work_id in restored:
                raise ArgumentError("autonomous connector work queue snapshot contains duplicate work ids")
            restored[item.work_id] = item
        with self._lock:
            self._items = restored


class AutonomousConnectorWorkQueuePersistenceCoordinator:
    """Coordinate verified queue snapshots with a caller-owned persistence adapter."""

    def __init__(self, queue: InMemoryAutonomousConnectorWorkQueue, persistence: Any) -> None:
        if not isinstance(queue, InMemoryAutonomousConnectorWorkQueue):
            raise ArgumentError("autonomous connector work persistence requires a typed queue")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise ArgumentError("autonomous connector work persistence adapter is malformed")
        self.queue = queue
        self.persistence = persistence

    def restore(self) -> dict[str, Any]:
        snapshot = self.persistence.read()
        if snapshot is None:
            return {"status": "empty", "snapshot_digest": None, "items": 0}
        self.queue.restore(snapshot)
        return {"status": "restored", "snapshot_digest": snapshot["snapshot_digest"], "items": self.queue.verify_integrity()["items"]}

    def flush(self) -> dict[str, Any]:
        snapshot = self.queue.snapshot()
        self.persistence.write(snapshot)
        return snapshot


@dataclass(frozen=True, slots=True)
class AutonomousConnectorWorkerRow:
    work_id: str
    outcome: str
    attempts: int
    receipt: dict[str, Any] | None
    value_retained: bool
    payload_digest: str | None
    error_class: str | None

    def to_dict(self) -> dict[str, Any]:
        return {"work_id": self.work_id, "outcome": self.outcome, "attempts": self.attempts, "receipt": self.receipt, "value_retained": False, "payload_digest": self.payload_digest, "error_class": self.error_class}


class AutonomousConnectorWorker:
    """Rehydrate caller-owned state, verify identities, and invoke a planned connector once."""

    def __init__(self, runtime: AutonomousConnectorRuntime, queue: InMemoryAutonomousConnectorWorkQueue, rehydrate: Callable[[AutonomousConnectorWorkItem], Any]) -> None:
        if not isinstance(runtime, AutonomousConnectorRuntime):
            raise ArgumentError("autonomous connector worker requires a connector runtime")
        if not isinstance(queue, InMemoryAutonomousConnectorWorkQueue):
            raise ArgumentError("autonomous connector worker requires a typed work queue")
        if not callable(rehydrate):
            raise ArgumentError("autonomous connector worker requires a rehydrator")
        self.runtime = runtime
        self.queue = queue
        self.rehydrate = rehydrate

    def run(self, *, worker_id: str = "connector-worker", limit: int = 64, lease_ms: int = 30_000, now: int | None = None, aborted: Callable[[], bool] | None = None) -> dict[str, Any]:
        worker_id = _identifier("autonomous connector worker_id", worker_id)
        limit = _bounded_integer("autonomous connector worker limit", limit, minimum=1, maximum=MAX_AUTONOMOUS_CONNECTOR_WORK_BATCH)
        lease_ms = _bounded_integer("autonomous connector worker lease_ms", lease_ms, minimum=1, maximum=MAX_AUTONOMOUS_CONNECTOR_WORK_LEASE_MS)
        deterministic_now = now is not None
        current = _now_ms(now)
        candidates = self.queue.pending(limit=limit, now=current)
        rows: list[AutonomousConnectorWorkerRow] = []
        for candidate in candidates:
            if aborted is not None and aborted():
                break
            claimed = self.queue.claim(candidate.work_id, worker_id, lease_ms=lease_ms, now=current)
            if claimed is None:
                rows.append(AutonomousConnectorWorkerRow(candidate.work_id, "leased_elsewhere", candidate.attempts, None, False, candidate.payload_digest, None))
                continue
            finish_now = lambda: current if deterministic_now else _now_ms()
            try:
                hydrated = self.rehydrate(claimed)
                if not isinstance(hydrated, Mapping) or not isinstance(hydrated.get("request"), AutonomousConnectorDispatchRequest):
                    reconciled = self.queue.reconcile(claimed.work_id, worker_id, "rehydration_missing", now=finish_now())
                    rows.append(self._row(reconciled, "reconciliation_required", None, "rehydration_missing"))
                    continue
                raw_plan = hydrated.get("plan")
                plan = AutonomousConnectorSelectionPlan.from_mapping(raw_plan) if isinstance(raw_plan, Mapping) else raw_plan
                request = hydrated["request"]
                if not isinstance(plan, AutonomousConnectorSelectionPlan):
                    raise ArgumentError("autonomous connector worker rehydrated plan is invalid")
                self._assert_identity(claimed, plan, request)
                result = self.runtime.dispatch_from_plan(plan, request)
                if result.receipt.status in {"observed", "partial"}:
                    completed = self.queue.complete(claimed.work_id, worker_id, result.receipt, now=finish_now())
                    rows.append(self._row(completed, "replayed" if result.replay == "replayed" else "completed", result.receipt, None))
                else:
                    failure = result.receipt.failure_class if result.receipt.failure_class in _WORK_FAILURE_CLASSES else "unknown"
                    failed = self.queue.fail(claimed.work_id, worker_id, failure or "unknown", retryable=result.receipt.status in {"error", "unknown"}, now=finish_now(), receipt=result.receipt)
                    rows.append(self._row(failed, "retry_scheduled" if failed.status == "queued" else "failed", result.receipt, failure or "unknown"))
            except Exception as error:
                failure = self._classify(error)
                if failure in {"rehydration_missing", "rehydration_invalid", "identity_conflict"}:
                    reconciled = self.queue.reconcile(claimed.work_id, worker_id, failure, now=finish_now())
                    rows.append(self._row(reconciled, "reconciliation_required", None, failure))
                else:
                    failed = self.queue.fail(claimed.work_id, worker_id, failure, retryable=failure in {"executor_error", "transport_error", "unknown"}, now=finish_now())
                    rows.append(self._row(failed, "retry_scheduled" if failed.status == "queued" else "failed", None, failed.failure_class))
        return {
            "schema": AUTONOMOUS_CONNECTOR_WORKER_SCHEMA,
            "worker_id": worker_id,
            "inspected": len(candidates),
            "completed": sum(row.outcome in {"completed", "replayed"} for row in rows),
            "retried": sum(row.outcome == "retry_scheduled" for row in rows),
            "failed": sum(row.outcome == "failed" for row in rows),
            "reconciled": sum(row.outcome == "reconciliation_required" for row in rows),
            "leased_elsewhere": sum(row.outcome == "leased_elsewhere" for row in rows),
            "rows": [row.to_dict() for row in rows],
            "retention": "metadata_only_receipts_no_request_or_plan_or_payload_values",
            "secret_material": "never_returned",
        }

    def _assert_identity(self, item: AutonomousConnectorWorkItem, plan: AutonomousConnectorSelectionPlan, request: AutonomousConnectorDispatchRequest) -> None:
        if (request.request_digest, request.selection_plan_digest, request.dispatch_id, request.execution_id, request.call_id, request.connector_id, request.capability, request.attempt_id, request.approved, tuple(request.domains)) != (item.request_digest, item.selection_plan_digest, item.dispatch_id, item.execution_id, item.call_id, item.connector_id, item.capability, item.attempt_id, item.approved, (item.domain,)):
            raise ArgumentError("autonomous connector worker hydrated request identity conflicts with the work item")
        self.queue.operation_registry.assert_request(item.operation_id, request)
        plan.verify(self.runtime.registry)
        if not plan.complete or plan.plan_digest != item.selection_plan_digest or plan.capability != request.capability or tuple(plan.domains) != (item.domain,):
            raise ArgumentError("autonomous connector worker hydrated selection plan is stale or incomplete")
        if not plan.rows[0].connector_id == item.connector_id:
            raise ArgumentError("autonomous connector worker hydrated selection plan selects a different connector")

    @staticmethod
    def _row(item: AutonomousConnectorWorkItem, outcome: str, receipt: AutonomousConnectorDispatchReceipt | None, error_class: str | None) -> AutonomousConnectorWorkerRow:
        return AutonomousConnectorWorkerRow(item.work_id, outcome, item.attempts, None if receipt is None else receipt.to_dict(), False, None if receipt is None else receipt.payload_digest or item.payload_digest, error_class)

    @staticmethod
    def _classify(error: Exception) -> str:
        message = str(error).lower()
        if "missing" in message and "rehydrat" in message:
            return "rehydration_missing"
        if "request identity" in message or "identity conflicts" in message:
            return "identity_conflict"
        if "rehydrat" in message or "selection plan" in message or "operation" in message:
            return "rehydration_invalid"
        if "approval_required" in message:
            return "approval_required"
        if "domain_scope" in message:
            return "domain_scope"
        if "capability_scope" in message:
            return "capability_scope"
        if "executor" in message:
            return "executor_error"
        if "transport" in message:
            return "transport_error"
        return "unknown"


class InMemoryAutonomousConnectorFeedbackLedger:
    """Explicit evaluator feedback ledger; transport outcomes never become reward."""

    def __init__(self, *, max_entries: int = MAX_AUTONOMOUS_CONNECTOR_FEEDBACK_ENTRIES) -> None:
        if isinstance(max_entries, bool) or not isinstance(max_entries, int) or not 1 <= max_entries <= MAX_AUTONOMOUS_CONNECTOR_FEEDBACK_ENTRIES:
            raise ArgumentError("autonomous connector feedback ledger max_entries is outside its bound")
        self.max_entries = max_entries
        self._entries: dict[str, dict[str, Any]] = {}
        self._lock = threading.RLock()

    def record(self, *, feedback: Mapping[str, Any], receipt: AutonomousConnectorDispatchReceipt, now: int | None = None) -> dict[str, Any]:
        if not isinstance(feedback, Mapping) or feedback.get("source") != "caller_evaluator":
            raise ArgumentError("autonomous connector feedback must be explicitly caller_evaluator sourced")
        if not isinstance(receipt, AutonomousConnectorDispatchReceipt):
            raise ArgumentError("autonomous connector feedback requires a typed receipt")
        allowed = {"feedback_id", "domain", "evaluator_id", "evaluator_version", "reward", "passed", "source", "evidence_digest", "failure_class", "created_at"}
        if set(feedback).difference(allowed):
            raise ArgumentError("autonomous connector feedback contains unsupported fields")
        feedback_id = _identifier("autonomous connector feedback_id", feedback.get("feedback_id"))
        selected_domain = feedback.get("domain", receipt.domains[0] if receipt.domains else None)
        if selected_domain not in receipt.domains or selected_domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("autonomous connector feedback domain is not present on the receipt")
        reward = feedback.get("reward")
        if isinstance(reward, bool) or not isinstance(reward, (int, float)) or not math.isfinite(float(reward)) or not -1.0 <= float(reward) <= 1.0:
            raise ArgumentError("autonomous connector evaluator reward must be between -1 and 1")
        if not isinstance(feedback.get("passed"), bool):
            raise ArgumentError("autonomous connector evaluator passed must be boolean")
        evidence_digest = _digest("autonomous connector feedback evidence_digest", feedback.get("evidence_digest"), allow_none=True)
        failure_class = feedback.get("failure_class")
        if failure_class is not None:
            failure_class = _identifier("autonomous connector feedback failure_class", failure_class)
        created_at = _now_ms(feedback.get("created_at", now))
        entry = {
            "schema": AUTONOMOUS_CONNECTOR_FEEDBACK_SCHEMA,
            "feedback_id": feedback_id,
            "domain": selected_domain,
            "capability": _capability_identifier("autonomous connector feedback capability", receipt.capability),
            "connector_id": _identifier("autonomous connector feedback connector_id", receipt.connector_id),
            "receipt_digest": content_digest(receipt.to_dict()),
            "evaluator_id": _identifier("autonomous connector evaluator_id", feedback.get("evaluator_id")),
            "evaluator_version": _identifier("autonomous connector evaluator_version", feedback.get("evaluator_version")),
            "reward": float(reward),
            "passed": feedback["passed"],
            "evidence_digest": evidence_digest,
            "failure_class": failure_class,
            "created_at": created_at,
            "retention": "metadata_only_explicit_evaluator_signal_no_request_or_payload",
            "secret_material": "never_returned",
        }
        entry["entry_digest"] = content_digest({key: value for key, value in entry.items() if key != "entry_digest"})
        with self._lock:
            existing = self._entries.get(feedback_id)
            if existing is not None:
                if existing != entry:
                    raise ArgumentError("autonomous connector feedback identity conflicts with an existing entry")
                return dict(existing)
            if len(self._entries) >= self.max_entries:
                raise ArgumentError("autonomous connector feedback ledger is full")
            self._entries[feedback_id] = entry
            return dict(entry)

    def rows(self) -> tuple[dict[str, Any], ...]:
        with self._lock:
            return tuple(dict(entry) for entry in sorted(self._entries.values(), key=lambda row: (row["created_at"], row["feedback_id"])))

    def signals(self, *, domain: str | None = None, capability: str | None = None) -> dict[str, dict[str, Any]]:
        if domain is not None and domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("autonomous connector feedback signal domain is unsupported")
        if capability is not None:
            capability = _capability_identifier("autonomous connector feedback signal capability", capability)
        grouped: dict[str, list[dict[str, Any]]] = {}
        for entry in self.rows():
            if domain is not None and entry["domain"] != domain:
                continue
            if capability is not None and entry["capability"] != capability:
                continue
            grouped.setdefault(entry["connector_id"], []).append(entry)
        result: dict[str, dict[str, Any]] = {}
        for connector_id in sorted(grouped):
            entries = grouped[connector_id]
            reward = sum(float(entry["reward"]) for entry in entries) / len(entries)
            passed = sum(bool(entry["passed"]) for entry in entries) / len(entries)
            result[connector_id] = {"eligible": True, "health": (reward + 1.0) / 2.0, "success_rate": passed, "evaluator_reward": reward, "latency_ms": None, "cost_per_million_tokens": None}
        return result

    def verify_integrity(self) -> dict[str, Any]:
        for entry in self.rows():
            if entry["entry_digest"] != content_digest({key: value for key, value in entry.items() if key != "entry_digest"}):
                raise ArgumentError("autonomous connector feedback ledger entry digest is invalid")
        return {"schema": AUTONOMOUS_CONNECTOR_FEEDBACK_LEDGER_SCHEMA, "verified": True, "entries": len(self._entries), "retention": "metadata_only_explicit_evaluator_signal_no_request_or_payload", "secret_material": "never_returned"}

    def snapshot(self) -> dict[str, Any]:
        self.verify_integrity()
        descriptor = {"schema": AUTONOMOUS_CONNECTOR_FEEDBACK_LEDGER_SCHEMA, "entries": list(self.rows()), "retention": "metadata_only_explicit_evaluator_signal_no_request_or_payload", "secret_material": "never_returned"}
        snapshot = {**descriptor, "snapshot_digest": content_digest(descriptor)}
        encoded = json.dumps(snapshot, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
        if len(encoded) > MAX_AUTONOMOUS_CONNECTOR_FEEDBACK_SNAPSHOT_BYTES:
            raise ArgumentError("autonomous connector feedback snapshot exceeds its bound")
        return snapshot

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        if not isinstance(snapshot, Mapping) or snapshot.get("schema") != AUTONOMOUS_CONNECTOR_FEEDBACK_LEDGER_SCHEMA or not isinstance(snapshot.get("entries"), Sequence) or isinstance(snapshot.get("entries"), (str, bytes)):
            raise ArgumentError("autonomous connector feedback snapshot is malformed")
        if snapshot.get("retention") != "metadata_only_explicit_evaluator_signal_no_request_or_payload" or snapshot.get("secret_material") != "never_returned":
            raise ArgumentError("autonomous connector feedback snapshot retention is invalid")
        descriptor = {key: value for key, value in snapshot.items() if key != "snapshot_digest"}
        if _digest("autonomous connector feedback snapshot digest", snapshot.get("snapshot_digest")) != content_digest(descriptor):
            raise ArgumentError("autonomous connector feedback snapshot digest is invalid")
        entries: dict[str, dict[str, Any]] = {}
        for raw in snapshot["entries"]:
            if not isinstance(raw, Mapping) or raw.get("schema") != AUTONOMOUS_CONNECTOR_FEEDBACK_SCHEMA or raw.get("retention") != "metadata_only_explicit_evaluator_signal_no_request_or_payload" or raw.get("secret_material") != "never_returned":
                raise ArgumentError("autonomous connector feedback snapshot entry is malformed")
            entry = dict(raw)
            if entry.get("entry_digest") != content_digest({key: value for key, value in entry.items() if key != "entry_digest"}):
                raise ArgumentError("autonomous connector feedback snapshot entry digest is invalid")
            feedback_id = _identifier("autonomous connector feedback_id", entry.get("feedback_id"))
            if feedback_id in entries:
                raise ArgumentError("autonomous connector feedback snapshot contains duplicate feedback ids")
            entries[feedback_id] = entry
        if len(entries) > self.max_entries:
            raise ArgumentError("autonomous connector feedback snapshot exceeds max_entries")
        with self._lock:
            self._entries = entries


__all__ = [
    "AUTONOMOUS_CONNECTOR_OPERATION_REGISTRY_SCHEMA",
    "AUTONOMOUS_CONNECTOR_OPERATION_SCHEMA",
    "AUTONOMOUS_CONNECTOR_WORK_ITEM_SCHEMA",
    "AUTONOMOUS_CONNECTOR_WORK_QUEUE_SCHEMA",
    "AUTONOMOUS_CONNECTOR_WORKER_SCHEMA",
    "AUTONOMOUS_CONNECTOR_FEEDBACK_SCHEMA",
    "AUTONOMOUS_CONNECTOR_FEEDBACK_LEDGER_SCHEMA",
    "MAX_AUTONOMOUS_CONNECTOR_OPERATIONS",
    "MAX_AUTONOMOUS_CONNECTOR_WORK_ITEMS",
    "MAX_AUTONOMOUS_CONNECTOR_WORK_ATTEMPTS",
    "MAX_AUTONOMOUS_CONNECTOR_WORK_BATCH",
    "MAX_AUTONOMOUS_CONNECTOR_WORK_LEASE_MS",
    "MAX_AUTONOMOUS_CONNECTOR_WORK_SNAPSHOT_BYTES",
    "MAX_AUTONOMOUS_CONNECTOR_FEEDBACK_ENTRIES",
    "MAX_AUTONOMOUS_CONNECTOR_FEEDBACK_SNAPSHOT_BYTES",
    "AutonomousConnectorOperationContract",
    "AutonomousConnectorOperationRegistry",
    "default_autonomous_connector_operation_contracts",
    "AutonomousConnectorWorkItem",
    "InMemoryAutonomousConnectorWorkQueue",
    "AutonomousConnectorWorkQueuePersistenceCoordinator",
    "AutonomousConnectorWorkerRow",
    "AutonomousConnectorWorker",
    "InMemoryAutonomousConnectorFeedbackLedger",
]
