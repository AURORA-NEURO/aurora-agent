"""Reviewed, provider-neutral evidence execution for the autonomous brain.

The lower-level evidence modules intentionally expose independent primitives: a workflow evidence
plan, an adapter selection, a health ledger, a retry policy, and a runtime.  This module is the
admission boundary that joins those primitives into one digest-bound lifecycle.  Preparation is
side-effect free.  Execution requires an explicit approval bit, rechecks readiness against the
exact reviewed selection, and only then dispatches caller-owned adapters through bounded retry and
failover.  Durable projections contain metadata and digests only; source values and provider
payloads stay with the caller.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence import AutonomousEvidencePlan
from .autonomous_evidence_adapters import (
    AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_STRATEGIES,
    MAX_AUTONOMOUS_EVIDENCE_ADAPTERS,
    AutonomousEvidenceAdapterHealthController,
    AutonomousEvidenceAdapterRegistry,
    AutonomousEvidenceAdapterSelectionPlan,
    AutonomousEvidenceAdapterSelector,
    AutonomousEvidenceFailoverPolicy,
    InMemoryAutonomousEvidenceAdapterHealthStore,
    _digest,
    _domains,
    _finite,
    _identifier,
    _integer,
    _json_bytes,
)
from .autonomous_evidence_provider_contract import AutonomousEvidenceProviderContractRegistry
from .autonomous_evidence_retry import AutonomousEvidenceRetryPolicy
from .autonomous_evidence_runtime import (
    AutonomousEvidenceRuntime,
    AutonomousEvidenceRuntimeJournal,
    AutonomousEvidenceRuntimeResult,
)
from .autonomous_evidence_source import AutonomousEvidenceSourcePolicy
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_EVIDENCE_READINESS_SCHEMA = "bioprism-python-autonomous-evidence-readiness/0.1"
AUTONOMOUS_EVIDENCE_READINESS_DOMAIN_SCHEMA = "bioprism-python-autonomous-evidence-readiness-domain/0.1"
AUTONOMOUS_EVIDENCE_READINESS_POLICY_SCHEMA = "bioprism-python-autonomous-evidence-readiness-policy/0.1"
AUTONOMOUS_EVIDENCE_EXECUTION_PLAN_SCHEMA = "bioprism-python-autonomous-evidence-execution-plan/0.1"
AUTONOMOUS_EVIDENCE_EXECUTION_RESULT_SCHEMA = "bioprism-python-autonomous-evidence-execution-result/0.1"
MAX_AUTONOMOUS_EVIDENCE_READINESS_DOMAINS = len(AUTONOMOUS_DOMAIN_NAMES)
MAX_AUTONOMOUS_EVIDENCE_READINESS_BYTES = 256_000
MAX_AUTONOMOUS_EVIDENCE_EXECUTION_REQUESTS = 128
MAX_AUTONOMOUS_EVIDENCE_EXECUTION_PLAN_BYTES = 512_000

_READINESS_STATUSES = frozenset({"ready", "degraded", "blocked", "missing"})
_OVERALL_STATUSES = frozenset({"ready", "degraded", "blocked"})
_RETENTION = "metadata_only_coverage_selection_health_and_policy"
_EXECUTION = "projection_only;no_source_dispatch"
_EXECUTION_RETENTION = "metadata_only;raw_source_values_and_provider_payloads_caller_owned"
_SECRET_MATERIAL = "never_returned"


def _same_domains(left: Sequence[str], right: Sequence[str]) -> bool:
    return tuple(left) == tuple(right)


def _optional_source_kind(value: Any) -> str | None:
    if value is None:
        return None
    return _identifier("evidence execution source_kind", value)


def _policy_digest(policy: Any) -> str:
    return content_digest(policy.to_dict())


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceReadinessPolicy:
    """Operational admission criteria, never a source or provider authorization."""

    require_health: bool = True
    min_attempts: int = 1
    failure_threshold: float = 0.75
    min_success_rate: float = 0.5

    def __post_init__(self) -> None:
        if not isinstance(self.require_health, bool):
            raise ArgumentError("evidence readiness require_health must be boolean")
        _integer("evidence readiness min_attempts", self.min_attempts, 1, 1_000_000)
        _finite("evidence readiness failure_threshold", self.failure_threshold, 0, 1)
        _finite("evidence readiness min_success_rate", self.min_success_rate, 0, 1)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_READINESS_POLICY_SCHEMA,
            "require_health": self.require_health,
            "min_attempts": self.min_attempts,
            "failure_threshold": float(self.failure_threshold),
            "min_success_rate": float(self.min_success_rate),
            "execution": "audit_only;policy_does_not_authorize_source_dispatch",
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }

    @property
    def policy_digest(self) -> str:
        return _policy_digest(self)

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousEvidenceReadinessPolicy":
        if not isinstance(value, Mapping):
            raise ArgumentError("evidence readiness policy must be a mapping")
        allowed = {
            "schema", "require_health", "min_attempts", "failure_threshold", "min_success_rate",
            "execution", "retention", "secret_material",
        }
        if set(value) != allowed or value.get("schema") != AUTONOMOUS_EVIDENCE_READINESS_POLICY_SCHEMA:
            raise ArgumentError("evidence readiness policy contains unsupported fields")
        if value.get("execution") != "audit_only;policy_does_not_authorize_source_dispatch" or value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
            raise ArgumentError("evidence readiness policy retention contract is invalid")
        result = cls(value.get("require_health"), value.get("min_attempts"), value.get("failure_threshold"), value.get("min_success_rate"))
        if canonical_json(value) != canonical_json(result.to_dict()):
            raise ArgumentError("evidence readiness policy is not canonical")
        return result


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceReadinessHealth:
    observed: bool
    attempts: int
    successes: int
    failures: int
    success_rate: float | None
    failure_rate: float | None
    circuit: str
    manifest_digest: str | None

    def __post_init__(self) -> None:
        if not isinstance(self.observed, bool):
            raise ArgumentError("evidence readiness health observed must be boolean")
        _integer("evidence readiness health attempts", self.attempts, 0, 1_000_000)
        _integer("evidence readiness health successes", self.successes, 0, self.attempts)
        _integer("evidence readiness health failures", self.failures, 0, self.attempts)
        if self.circuit not in {"closed", "open", "unknown"}:
            raise ArgumentError("evidence readiness health circuit is invalid")
        if self.success_rate is not None:
            _finite("evidence readiness health success_rate", self.success_rate, 0, 1)
        if self.failure_rate is not None:
            _finite("evidence readiness health failure_rate", self.failure_rate, 0, 1)
        _digest("evidence readiness health manifest_digest", self.manifest_digest, allow_none=True)
        if self.attempts == 0 and (self.success_rate is not None or self.failure_rate is not None):
            raise ArgumentError("evidence readiness health rates require an observed attempt")

    def to_dict(self) -> dict[str, Any]:
        return {
            "observed": self.observed,
            "attempts": self.attempts,
            "successes": self.successes,
            "failures": self.failures,
            "success_rate": self.success_rate,
            "failure_rate": self.failure_rate,
            "circuit": self.circuit,
            "manifest_digest": self.manifest_digest,
        }


def _health_projection(row: Mapping[str, Any] | None, manifest_digest: str | None) -> AutonomousEvidenceReadinessHealth:
    if row is None:
        return AutonomousEvidenceReadinessHealth(False, 0, 0, 0, None, None, "unknown", manifest_digest)
    attempts = _integer("evidence readiness observed attempts", row.get("attempts"), 0, 1_000_000)
    successes = _integer("evidence readiness observed successes", row.get("successes"), 0, attempts)
    failures = _integer("evidence readiness observed failures", row.get("failures"), 0, attempts)
    if row.get("manifest_digest") != manifest_digest:
        raise ArgumentError("evidence readiness health manifest does not match the selected manifest")
    return AutonomousEvidenceReadinessHealth(
        attempts > 0,
        attempts,
        successes,
        failures,
        None if attempts == 0 else _finite("evidence readiness observed success_rate", row.get("success_rate"), 0, 1),
        None if attempts == 0 else _finite("evidence readiness observed failure_rate", row.get("failure_rate"), 0, 1),
        row.get("circuit"),
        manifest_digest,
    )


def _domain_reason(status: str, selection_reason: str, health: AutonomousEvidenceReadinessHealth) -> str:
    if status == "missing":
        return "no_registered_adapter_matches_domain_and_capability" if selection_reason == "no_matching_adapter" else "no_registered_adapter_matches_requested_readiness_scope"
    if status == "blocked":
        if selection_reason not in AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_STRATEGIES:
            return selection_reason
        if health.circuit == "open":
            return "selected_adapter_health_circuit_open"
        if not health.observed:
            return "selected_adapter_has_no_usable_health_observation"
        return "selected_adapter_health_below_readiness_threshold"
    if status == "degraded":
        return "selected_adapter_has_no_health_observation" if not health.observed else "selected_adapter_is_usable_but_health_is_not_required_or_insufficiently_observed"
    return "selected_adapter_has_current_manifest_and_usable_health"


def _readiness_status(selection_row: Any, coverage: Any, health: AutonomousEvidenceReadinessHealth, policy: AutonomousEvidenceReadinessPolicy) -> str:
    if selection_row.status == "missing":
        return "missing" if coverage.state == "missing" or selection_row.reason == "no_matching_adapter" else "blocked"
    if health.circuit == "open":
        return "blocked"
    if not health.observed or health.attempts < policy.min_attempts or (health.success_rate or 0) < policy.min_success_rate:
        return "blocked" if policy.require_health else "degraded"
    return "ready"


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceReadinessDomain:
    domain: str
    status: str
    coverage_state: str
    adapter_ids: tuple[str, ...]
    selected_adapter_id: str | None
    selected_manifest_digest: str | None
    candidate_count: int
    eligible_candidate_count: int
    selection_reason: str
    selection_strategy: str
    health: AutonomousEvidenceReadinessHealth
    retry_policy_digest: str
    failover_policy_digest: str
    reason: str

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES or self.status not in _READINESS_STATUSES:
            raise ArgumentError("evidence readiness domain or status is invalid")
        if self.coverage_state not in {"complete", "missing"}:
            raise ArgumentError("evidence readiness coverage state is invalid")
        if len(self.adapter_ids) > MAX_AUTONOMOUS_EVIDENCE_ADAPTERS or len(set(self.adapter_ids)) != len(self.adapter_ids):
            raise ArgumentError("evidence readiness adapter ids exceed their bound or repeat")
        for index, adapter_id in enumerate(self.adapter_ids):
            _identifier(f"evidence readiness adapter id {index}", adapter_id)
        if self.selected_adapter_id is not None:
            _identifier("evidence readiness selected adapter id", self.selected_adapter_id)
            if self.selected_adapter_id not in self.adapter_ids:
                raise ArgumentError("evidence readiness selected adapter is not in the candidate set")
        _digest("evidence readiness selected manifest digest", self.selected_manifest_digest, allow_none=True)
        _integer("evidence readiness candidate_count", self.candidate_count, 0, MAX_AUTONOMOUS_EVIDENCE_ADAPTERS)
        _integer("evidence readiness eligible_candidate_count", self.eligible_candidate_count, 0, self.candidate_count)
        _identifier("evidence readiness selection reason", self.selection_reason)
        if self.selection_strategy not in AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_STRATEGIES:
            raise ArgumentError("evidence readiness selection strategy is invalid")
        if not isinstance(self.health, AutonomousEvidenceReadinessHealth):
            raise ArgumentError("evidence readiness health is malformed")
        if self.health.manifest_digest != self.selected_manifest_digest:
            raise ArgumentError("evidence readiness health manifest does not match the selected manifest")
        _digest("evidence readiness retry policy digest", self.retry_policy_digest)
        _digest("evidence readiness failover policy digest", self.failover_policy_digest)
        _identifier("evidence readiness reason", self.reason)
        if self.status == "ready" and self.selected_adapter_id is None:
            raise ArgumentError("ready evidence readiness row requires a selected adapter")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_READINESS_DOMAIN_SCHEMA,
            "domain": self.domain,
            "status": self.status,
            "coverage_state": self.coverage_state,
            "adapter_ids": list(self.adapter_ids),
            "selected_adapter_id": self.selected_adapter_id,
            "selected_manifest_digest": self.selected_manifest_digest,
            "candidate_count": self.candidate_count,
            "eligible_candidate_count": self.eligible_candidate_count,
            "selection_reason": self.selection_reason,
            "selection_strategy": self.selection_strategy,
            "health": self.health.to_dict(),
            "retry_policy_digest": self.retry_policy_digest,
            "failover_policy_digest": self.failover_policy_digest,
            "reason": self.reason,
            "execution": _EXECUTION,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceReadinessReport:
    domains: tuple[AutonomousEvidenceReadinessDomain, ...]
    registry_digest: str
    selection_plan_digest: str
    health_snapshot_digest: str | None
    policy: AutonomousEvidenceReadinessPolicy
    retry_policy: AutonomousEvidenceRetryPolicy
    failover_policy: AutonomousEvidenceFailoverPolicy

    def __post_init__(self) -> None:
        if not 1 <= len(self.domains) <= MAX_AUTONOMOUS_EVIDENCE_READINESS_DOMAINS:
            raise ArgumentError("evidence readiness report domains are outside their bound")
        if len({row.domain for row in self.domains}) != len(self.domains) or any(not isinstance(row, AutonomousEvidenceReadinessDomain) for row in self.domains):
            raise ArgumentError("evidence readiness report rows are malformed")
        _digest("evidence readiness registry digest", self.registry_digest)
        _digest("evidence readiness selection plan digest", self.selection_plan_digest)
        _digest("evidence readiness health snapshot digest", self.health_snapshot_digest, allow_none=True)
        if not isinstance(self.policy, AutonomousEvidenceReadinessPolicy) or not isinstance(self.retry_policy, AutonomousEvidenceRetryPolicy) or not isinstance(self.failover_policy, AutonomousEvidenceFailoverPolicy):
            raise ArgumentError("evidence readiness report policy is malformed")
        if _policy_digest(self.policy) != self.policy_digest:
            raise ArgumentError("evidence readiness policy digest is inconsistent")

    @property
    def status(self) -> str:
        if any(row.status in {"blocked", "missing"} for row in self.domains):
            return "blocked"
        if any(row.status == "degraded" for row in self.domains):
            return "degraded"
        return "ready"

    @property
    def ready_count(self) -> int:
        return sum(row.status == "ready" for row in self.domains)

    @property
    def degraded_count(self) -> int:
        return sum(row.status == "degraded" for row in self.domains)

    @property
    def blocked_count(self) -> int:
        return sum(row.status == "blocked" for row in self.domains)

    @property
    def missing_count(self) -> int:
        return sum(row.status == "missing" for row in self.domains)

    @property
    def complete(self) -> bool:
        return self.status == "ready"

    @property
    def policy_digest(self) -> str:
        return _policy_digest(self.policy)

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_READINESS_SCHEMA,
            "domains": [row.to_dict() for row in self.domains],
            "registry_digest": self.registry_digest,
            "selection_plan_digest": self.selection_plan_digest,
            "health_snapshot_digest": self.health_snapshot_digest,
            "policy_digest": self.policy_digest,
            "readiness_policy": self.policy.to_dict(),
            "retry_policy": self.retry_policy.to_dict(),
            "failover_policy": self.failover_policy.to_dict(),
            "status": self.status,
            "ready_count": self.ready_count,
            "degraded_count": self.degraded_count,
            "blocked_count": self.blocked_count,
            "missing_count": self.missing_count,
            "complete": self.complete,
            "execution": _EXECUTION,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }

    @property
    def report_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        result = {**self._payload(), "report_digest": self.report_digest}
        _json_bytes(result, "evidence readiness report", MAX_AUTONOMOUS_EVIDENCE_READINESS_BYTES)
        return result


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceExecutionPlan:
    evidence_plan_digest: str
    domains: tuple[str, ...]
    registry_digest: str
    provider_contract_registry_digest: str | None
    source_policy_digest: str | None
    source_kind: str | None
    selection_plan: AutonomousEvidenceAdapterSelectionPlan
    readiness: AutonomousEvidenceReadinessReport
    readiness_policy: AutonomousEvidenceReadinessPolicy
    retry_policy: AutonomousEvidenceRetryPolicy
    failover_policy: AutonomousEvidenceFailoverPolicy
    degraded_dispatch_allowed: bool
    status: str
    plan_digest: str

    @classmethod
    def create(
        cls,
        evidence_plan: AutonomousEvidencePlan,
        selection_plan: AutonomousEvidenceAdapterSelectionPlan,
        readiness: AutonomousEvidenceReadinessReport,
        readiness_policy: AutonomousEvidenceReadinessPolicy,
        retry_policy: AutonomousEvidenceRetryPolicy,
        failover_policy: AutonomousEvidenceFailoverPolicy,
        *,
        provider_contracts: AutonomousEvidenceProviderContractRegistry | None = None,
        source_boundary: Mapping[str, Any] | None = None,
        allow_degraded_dispatch: bool = False,
    ) -> "AutonomousEvidenceExecutionPlan":
        if not isinstance(evidence_plan, AutonomousEvidencePlan) or not isinstance(selection_plan, AutonomousEvidenceAdapterSelectionPlan) or not isinstance(readiness, AutonomousEvidenceReadinessReport):
            raise ArgumentError("evidence execution plan requires typed evidence, selection, and readiness objects")
        if not isinstance(readiness_policy, AutonomousEvidenceReadinessPolicy) or not isinstance(retry_policy, AutonomousEvidenceRetryPolicy) or not isinstance(failover_policy, AutonomousEvidenceFailoverPolicy):
            raise ArgumentError("evidence execution plan policies are malformed")
        domains = _domains("evidence execution domains", evidence_plan.domains)
        if not _same_domains(domains, selection_plan.domains) or not _same_domains(domains, tuple(row.domain for row in readiness.domains)):
            raise ArgumentError("evidence execution plan domain scopes do not align")
        if not isinstance(allow_degraded_dispatch, bool):
            raise ArgumentError("evidence execution allow_degraded_dispatch must be boolean")
        if provider_contracts is not None:
            if not isinstance(provider_contracts, AutonomousEvidenceProviderContractRegistry):
                raise ArgumentError("evidence execution provider contract registry is malformed")
            provider_registry_digest = provider_contracts.registry_digest
        else:
            provider_registry_digest = None
        if source_boundary is not None:
            _validate_source_boundary(source_boundary, provider_contracts)
            source_policy_digest = source_boundary["policy"].policy_digest
            source_kind = _optional_source_kind(source_boundary.get("source_kind"))
        else:
            source_policy_digest = None
            source_kind = None
        if readiness.policy_digest != readiness_policy.policy_digest:
            raise ArgumentError("evidence execution readiness policy does not match its report")
        if readiness.registry_digest != selection_plan.registry_digest or readiness.selection_plan_digest != selection_plan.plan_digest:
            raise ArgumentError("evidence execution readiness is not bound to its selection plan")
        if _policy_digest(failover_policy.retry_policy) != _policy_digest(retry_policy):
            raise ArgumentError("evidence execution retry and failover policies do not match")
        status = "ready_for_review" if readiness.status == "ready" or (readiness.status == "degraded" and allow_degraded_dispatch) else "blocked"
        payload = _execution_plan_payload(
            evidence_plan.plan_digest, domains, selection_plan.registry_digest, provider_registry_digest,
            source_policy_digest, source_kind, selection_plan.to_dict(), readiness.to_dict(), readiness_policy.to_dict(),
            retry_policy.to_dict(), failover_policy.to_dict(), allow_degraded_dispatch, status,
        )
        return cls(evidence_plan.plan_digest, domains, selection_plan.registry_digest, provider_registry_digest, source_policy_digest, source_kind, selection_plan, readiness, readiness_policy, retry_policy, failover_policy, allow_degraded_dispatch, status, content_digest(payload))

    def _payload(self) -> dict[str, Any]:
        return _execution_plan_payload(
            self.evidence_plan_digest, self.domains, self.registry_digest, self.provider_contract_registry_digest,
            self.source_policy_digest, self.source_kind, self.selection_plan.to_dict(), self.readiness.to_dict(),
            self.readiness_policy.to_dict(), self.retry_policy.to_dict(), self.failover_policy.to_dict(),
            self.degraded_dispatch_allowed, self.status,
        )

    def verify(
        self,
        registry: AutonomousEvidenceAdapterRegistry,
        evidence_plan: AutonomousEvidencePlan,
        *,
        provider_contracts: AutonomousEvidenceProviderContractRegistry | None = None,
        source_boundary: Mapping[str, Any] | None = None,
    ) -> "AutonomousEvidenceExecutionPlan":
        if not isinstance(registry, AutonomousEvidenceAdapterRegistry) or not isinstance(evidence_plan, AutonomousEvidencePlan):
            raise ArgumentError("evidence execution verification requires typed registry and evidence plan")
        if evidence_plan.plan_digest != self.evidence_plan_digest or registry.registry_digest != self.registry_digest:
            raise ArgumentError("evidence execution plan or registry is stale or tampered")
        self.selection_plan.verify(registry)
        if self.selection_plan.plan_digest != self.readiness.selection_plan_digest:
            raise ArgumentError("evidence execution selection plan is not bound to readiness")
        if self.provider_contract_registry_digest is None:
            if provider_contracts is not None:
                raise ArgumentError("evidence execution plan was not prepared with provider contracts")
        elif not isinstance(provider_contracts, AutonomousEvidenceProviderContractRegistry) or provider_contracts.registry_digest != self.provider_contract_registry_digest:
            raise ArgumentError("evidence execution provider contract registry is stale or tampered")
        if self.source_policy_digest is None:
            if source_boundary is not None:
                raise ArgumentError("evidence execution plan was not prepared with a source boundary")
        else:
            _validate_source_boundary(source_boundary, provider_contracts)
            if source_boundary["policy"].policy_digest != self.source_policy_digest or _optional_source_kind(source_boundary.get("source_kind")) != self.source_kind:
                raise ArgumentError("evidence execution source boundary changed after planning")
        if self.plan_digest != content_digest(self._payload()):
            raise ArgumentError("evidence execution plan digest is invalid")
        return self

    def to_dict(self) -> dict[str, Any]:
        result = {**self._payload(), "plan_digest": self.plan_digest}
        _json_bytes(result, "evidence execution plan", MAX_AUTONOMOUS_EVIDENCE_EXECUTION_PLAN_BYTES)
        return result


def _execution_plan_payload(
    evidence_plan_digest: str,
    domains: Sequence[str],
    registry_digest: str,
    provider_registry_digest: str | None,
    source_policy_digest: str | None,
    source_kind: str | None,
    selection_plan: Mapping[str, Any],
    readiness: Mapping[str, Any],
    readiness_policy: Mapping[str, Any],
    retry_policy: Mapping[str, Any],
    failover_policy: Mapping[str, Any],
    degraded_dispatch_allowed: bool,
    status: str,
) -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_EVIDENCE_EXECUTION_PLAN_SCHEMA,
        "evidence_plan_digest": evidence_plan_digest,
        "domains": list(domains),
        "registry_digest": registry_digest,
        "provider_contract_registry_digest": provider_registry_digest,
        "source_policy_digest": source_policy_digest,
        "source_kind": source_kind,
        "selection_plan": selection_plan,
        "readiness": readiness,
        "readiness_policy": readiness_policy,
        "retry_policy": retry_policy,
        "failover_policy": failover_policy,
        "degraded_dispatch_allowed": degraded_dispatch_allowed,
        "status": status,
        "approval_required": True,
        "execution": "planning_only;source_dispatch_not_started",
        "retention": _EXECUTION_RETENTION,
        "secret_material": _SECRET_MATERIAL,
    }


def _validate_source_boundary(source_boundary: Mapping[str, Any] | None, provider_contracts: AutonomousEvidenceProviderContractRegistry | None) -> None:
    if not isinstance(source_boundary, Mapping) or not isinstance(source_boundary.get("policy"), AutonomousEvidenceSourcePolicy) or not callable(source_boundary.get("describe_source")):
        raise ArgumentError("source-bound evidence execution requires policy and describe_source")
    if provider_contracts is None:
        raise ArgumentError("source-bound evidence execution requires provider contracts")
    provider_contracts.verify()


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceExecutionResult:
    plan: AutonomousEvidenceExecutionPlan
    readiness: AutonomousEvidenceReadinessReport
    runtime: AutonomousEvidenceRuntimeResult

    def __post_init__(self) -> None:
        if not isinstance(self.plan, AutonomousEvidenceExecutionPlan) or not isinstance(self.readiness, AutonomousEvidenceReadinessReport) or not isinstance(self.runtime, AutonomousEvidenceRuntimeResult):
            raise ArgumentError("evidence execution result is malformed")

    @property
    def status(self) -> str:
        return self.runtime.status

    @property
    def result_digest(self) -> str:
        return content_digest({
            "execution_plan_digest": self.plan.plan_digest,
            "readiness_report_digest": self.readiness.report_digest,
            "runtime_result_digest": self.runtime.result_digest,
        })

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_EXECUTION_RESULT_SCHEMA,
            "status": self.status,
            "execution_plan_digest": self.plan.plan_digest,
            "readiness_report_digest": self.readiness.report_digest,
            "runtime": self.runtime.to_dict(),
            "result_digest": self.result_digest,
            "retention": _EXECUTION_RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


class AutonomousEvidenceReadinessAuditor:
    """Readiness projection over one exact adapter selection and optional health ledger."""

    def __init__(self, registry: AutonomousEvidenceAdapterRegistry, health_store: InMemoryAutonomousEvidenceAdapterHealthStore | None = None) -> None:
        if not isinstance(registry, AutonomousEvidenceAdapterRegistry):
            raise ArgumentError("evidence readiness auditor requires a typed registry")
        if health_store is not None and (not callable(getattr(health_store, "health", None)) or not callable(getattr(health_store, "snapshot", None))):
            raise ArgumentError("evidence readiness auditor health store is malformed")
        self.registry = registry
        self.health_store = health_store
        self.selector = AutonomousEvidenceAdapterSelector(registry)

    def audit(
        self,
        requested_domains: Sequence[str],
        *,
        selection_plan: AutonomousEvidenceAdapterSelectionPlan | Mapping[str, Any] | None = None,
        adaptive_selection: bool = False,
        health_selection_options: Mapping[str, Any] | None = None,
        selection_options: Mapping[str, Any] | None = None,
        policy: AutonomousEvidenceReadinessPolicy | None = None,
        retry_policy: AutonomousEvidenceRetryPolicy | None = None,
        failover_policy: AutonomousEvidenceFailoverPolicy | None = None,
    ) -> AutonomousEvidenceReadinessReport:
        requested = _domains("evidence readiness domains", requested_domains)
        readiness_policy = policy or AutonomousEvidenceReadinessPolicy()
        if not isinstance(readiness_policy, AutonomousEvidenceReadinessPolicy):
            raise ArgumentError("evidence readiness policy is malformed")
        retries = retry_policy or (failover_policy.retry_policy if failover_policy is not None else AutonomousEvidenceRetryPolicy())
        if not isinstance(retries, AutonomousEvidenceRetryPolicy):
            raise ArgumentError("evidence readiness retry policy is malformed")
        failover = failover_policy or AutonomousEvidenceFailoverPolicy(retry_policy=retries)
        if not isinstance(failover, AutonomousEvidenceFailoverPolicy):
            raise ArgumentError("evidence readiness failover policy is malformed")
        plan = self._resolve_plan(requested, selection_plan, adaptive_selection, health_selection_options, selection_options)
        plan.verify(self.registry)
        health_snapshot = self.health_store.snapshot() if self.health_store is not None else None
        rows: list[AutonomousEvidenceReadinessDomain] = []
        for domain in requested:
            selection = next((item for item in plan.rows if item.domain == domain), None)
            if selection is None:
                raise ArgumentError(f"evidence readiness selection plan does not cover {domain}")
            coverage = next((item for item in self.registry.coverage() if item.domain == domain), None)
            if coverage is None:
                raise ArgumentError(f"evidence readiness coverage is missing for {domain}")
            raw_health = None
            if selection.adapter_id is not None and selection.manifest_digest is not None and self.health_store is not None:
                result = self.health_store.health(adapter_id=selection.adapter_id, manifest_digest=selection.manifest_digest, domain=domain, min_attempts=readiness_policy.min_attempts, failure_threshold=readiness_policy.failure_threshold)
                raw_health = result[0] if result else None
            health = _health_projection(raw_health, selection.manifest_digest)
            status = _readiness_status(selection, coverage, health, readiness_policy)
            rows.append(AutonomousEvidenceReadinessDomain(
                domain=domain,
                status=status,
                coverage_state=coverage.state,
                adapter_ids=coverage.adapter_ids,
                selected_adapter_id=selection.adapter_id,
                selected_manifest_digest=selection.manifest_digest,
                candidate_count=len(selection.candidate_ids),
                eligible_candidate_count=sum(selection.candidate_eligible),
                selection_reason=selection.reason,
                selection_strategy=plan.strategy,
                health=health,
                retry_policy_digest=_policy_digest(retries),
                failover_policy_digest=content_digest(failover.to_dict()),
                reason=_domain_reason(status, selection.reason, health),
            ))
        snapshot_digest = getattr(health_snapshot, "snapshot_digest", None)
        return AutonomousEvidenceReadinessReport(tuple(rows), self.registry.registry_digest, plan.plan_digest, snapshot_digest, readiness_policy, retries, failover)

    def _resolve_plan(
        self,
        domains: tuple[str, ...],
        selection_plan: AutonomousEvidenceAdapterSelectionPlan | Mapping[str, Any] | None,
        adaptive_selection: bool,
        health_selection_options: Mapping[str, Any] | None,
        selection_options: Mapping[str, Any] | None,
    ) -> AutonomousEvidenceAdapterSelectionPlan:
        if selection_plan is not None:
            plan = selection_plan if isinstance(selection_plan, AutonomousEvidenceAdapterSelectionPlan) else AutonomousEvidenceAdapterSelectionPlan.from_dict(selection_plan)
            if not _same_domains(domains, plan.domains):
                raise ArgumentError("evidence readiness selection plan domains do not match the audit request")
            return plan
        if adaptive_selection:
            if self.health_store is None:
                raise ArgumentError("adaptive evidence readiness selection requires a health store")
            controller = AutonomousEvidenceAdapterHealthController(self.health_store, self.registry)
            return controller.select_adaptive_for_domains(domains, **dict(health_selection_options or {}))
        return self.selector.select_for_domains(domains, **dict(selection_options or {}))


class AutonomousEvidenceExecutionController:
    """Compose plan, readiness, approval, failover, runtime, and restart-safe journaling."""

    def __init__(self, registry: AutonomousEvidenceAdapterRegistry, health_store: InMemoryAutonomousEvidenceAdapterHealthStore | None = None) -> None:
        self.registry = registry
        self.health_store = health_store
        self.selector = AutonomousEvidenceAdapterSelector(registry)
        self.readiness_auditor = AutonomousEvidenceReadinessAuditor(registry, health_store)

    def prepare(
        self,
        evidence_plan: AutonomousEvidencePlan,
        *,
        selection_plan: AutonomousEvidenceAdapterSelectionPlan | Mapping[str, Any] | None = None,
        selection_options: Mapping[str, Any] | None = None,
        adaptive_selection: bool = False,
        health_selection_options: Mapping[str, Any] | None = None,
        readiness_policy: AutonomousEvidenceReadinessPolicy | None = None,
        retry_policy: AutonomousEvidenceRetryPolicy | None = None,
        failover_policy: AutonomousEvidenceFailoverPolicy | None = None,
        provider_contracts: AutonomousEvidenceProviderContractRegistry | None = None,
        source_boundary: Mapping[str, Any] | None = None,
        allow_degraded_dispatch: bool = False,
    ) -> AutonomousEvidenceExecutionPlan:
        if not isinstance(evidence_plan, AutonomousEvidencePlan):
            raise ArgumentError("evidence execution prepare requires a typed evidence plan")
        policy = readiness_policy or AutonomousEvidenceReadinessPolicy()
        retries = retry_policy or (failover_policy.retry_policy if failover_policy is not None else AutonomousEvidenceRetryPolicy())
        failover = failover_policy or AutonomousEvidenceFailoverPolicy(retry_policy=retries)
        if provider_contracts is not None:
            if not isinstance(provider_contracts, AutonomousEvidenceProviderContractRegistry):
                raise ArgumentError("evidence execution provider contract registry is malformed")
            provider_contracts.verify()
        if source_boundary is not None:
            _validate_source_boundary(source_boundary, provider_contracts)
        selection = self.readiness_auditor._resolve_plan(_domains("evidence execution domains", evidence_plan.domains), selection_plan, adaptive_selection, health_selection_options, selection_options)
        readiness = self.readiness_auditor.audit(evidence_plan.domains, selection_plan=selection, policy=policy, retry_policy=retries, failover_policy=failover)
        return AutonomousEvidenceExecutionPlan.create(evidence_plan, selection, readiness, policy, retries, failover, provider_contracts=provider_contracts, source_boundary=source_boundary, allow_degraded_dispatch=allow_degraded_dispatch)

    def execute(
        self,
        execution_plan: AutonomousEvidenceExecutionPlan,
        evidence_plan: AutonomousEvidencePlan,
        requests: Sequence[Mapping[str, Any]],
        *,
        approve_source_dispatch: bool = False,
        provider_contracts: AutonomousEvidenceProviderContractRegistry | None = None,
        source_boundary: Mapping[str, Any] | None = None,
        projector: Any | None = None,
        evaluator: Any | None = None,
        journal: AutonomousEvidenceRuntimeJournal | None = None,
        rehydrate_value: Callable[[Mapping[str, Any]], Any] | None = None,
        parent_evidence_digests: Sequence[str] = (),
        stop_on_failure: bool = False,
        reevaluate_pending: bool = False,
        classify: Callable[[BaseException], Any] | None = None,
        observe_failover: Callable[[Any], Any] | None = None,
        observe_attempt: Callable[[Any], Any] | None = None,
        clock: Callable[[], float] | None = None,
        sleep: Callable[[int], Any] | None = None,
    ) -> AutonomousEvidenceExecutionResult:
        if not isinstance(execution_plan, AutonomousEvidenceExecutionPlan) or not isinstance(evidence_plan, AutonomousEvidencePlan):
            raise ArgumentError("evidence execution requires typed plans")
        execution_plan.verify(self.registry, evidence_plan, provider_contracts=provider_contracts, source_boundary=source_boundary)
        if approve_source_dispatch is not True:
            raise ArgumentError("evidence source dispatch requires explicit approval")
        if execution_plan.status != "ready_for_review":
            raise ArgumentError("evidence execution plan is blocked by its readiness posture")
        if not isinstance(requests, Sequence) or isinstance(requests, (str, bytes, bytearray)) or not 1 <= len(requests) <= MAX_AUTONOMOUS_EVIDENCE_EXECUTION_REQUESTS:
            raise ArgumentError("evidence execution requests are outside their bound")
        current = self.readiness_auditor.audit(execution_plan.domains, selection_plan=execution_plan.selection_plan, policy=execution_plan.readiness_policy, retry_policy=execution_plan.retry_policy, failover_policy=execution_plan.failover_policy)
        if not (current.status == "ready" or current.status == "degraded" and execution_plan.degraded_dispatch_allowed):
            raise ArgumentError("evidence readiness no longer permits the reviewed execution")
        if current.report_digest != execution_plan.readiness.report_digest:
            raise ArgumentError("evidence readiness changed after planning; review is required again")
        from .autonomous_evidence_adapters import create_autonomous_evidence_adapter_failover_acquirer

        acquirer = create_autonomous_evidence_adapter_failover_acquirer(
            self.registry,
            execution_plan.selection_plan,
            policy=execution_plan.failover_policy,
            provider_contracts=provider_contracts,
            source_boundary=source_boundary,
            classify=classify,
            observe_failover=observe_failover,
            observe_attempt=observe_attempt,
            clock=clock,
            sleep=sleep,
        )
        runtime = AutonomousEvidenceRuntime(evidence_plan, journal=journal)
        runtime.rehydrate()
        result = runtime.execute(requests, acquirer=acquirer, projector=projector, evaluator=evaluator, rehydrate_value=rehydrate_value, parent_evidence_digests=parent_evidence_digests, stop_on_failure=stop_on_failure, reevaluate_pending=reevaluate_pending)
        return AutonomousEvidenceExecutionResult(execution_plan, current, result)


__all__ = [
    "AUTONOMOUS_EVIDENCE_READINESS_SCHEMA",
    "AUTONOMOUS_EVIDENCE_READINESS_DOMAIN_SCHEMA",
    "AUTONOMOUS_EVIDENCE_READINESS_POLICY_SCHEMA",
    "AUTONOMOUS_EVIDENCE_EXECUTION_PLAN_SCHEMA",
    "AUTONOMOUS_EVIDENCE_EXECUTION_RESULT_SCHEMA",
    "MAX_AUTONOMOUS_EVIDENCE_READINESS_DOMAINS",
    "MAX_AUTONOMOUS_EVIDENCE_READINESS_BYTES",
    "MAX_AUTONOMOUS_EVIDENCE_EXECUTION_REQUESTS",
    "MAX_AUTONOMOUS_EVIDENCE_EXECUTION_PLAN_BYTES",
    "AutonomousEvidenceReadinessPolicy",
    "AutonomousEvidenceReadinessHealth",
    "AutonomousEvidenceReadinessDomain",
    "AutonomousEvidenceReadinessReport",
    "AutonomousEvidenceReadinessAuditor",
    "AutonomousEvidenceExecutionPlan",
    "AutonomousEvidenceExecutionResult",
    "AutonomousEvidenceExecutionController",
]
