"""Operational readiness projections for reviewed LLM evidence routing.

The adapter orchestration module decides which approved route would be used.  This module
answers the separate operational question: is that route covered, selected, observed, healthy,
and permitted by the caller's stated readiness policy?  It never invokes an adapter, opens a
credential, contacts a provider, or treats readiness as source authorization.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence_adapter_orchestration import (
    AUTONOMOUS_LLM_EVIDENCE_FAILOVER_POLICY_SCHEMA,
    MAX_AUTONOMOUS_LLM_EVIDENCE_ADAPTERS,
    AutonomousLLMEvidenceAdapterRegistry,
    AutonomousLLMEvidenceAdapterSelectionPlan,
    AutonomousLLMEvidenceAdapterSelector,
    AutonomousLLMEvidenceFailoverPolicy,
    InMemoryAutonomousLLMEvidenceAdapterHealthStore,
    _digest,
    _domains,
    _finite,
    _identifier,
    _integer,
    _json_bytes,
    _optional_digest,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError
from .autonomous_evidence_retry import AutonomousEvidenceRetryPolicy


AUTONOMOUS_LLM_EVIDENCE_READINESS_SCHEMA = "bioprism-python-autonomous-llm-evidence-readiness/0.1"
AUTONOMOUS_LLM_EVIDENCE_READINESS_DOMAIN_SCHEMA = "bioprism-python-autonomous-llm-evidence-readiness-domain/0.1"
AUTONOMOUS_LLM_EVIDENCE_READINESS_POLICY_SCHEMA = "bioprism-python-autonomous-llm-evidence-readiness-policy/0.1"
AUTONOMOUS_LLM_EVIDENCE_READINESS_HEALTH_SCHEMA = "bioprism-python-autonomous-llm-evidence-readiness-health/0.1"
MAX_AUTONOMOUS_LLM_EVIDENCE_READINESS_DOMAINS = len(AUTONOMOUS_DOMAIN_NAMES)
MAX_AUTONOMOUS_LLM_EVIDENCE_READINESS_BYTES = 256_000

_READINESS_STATUSES = frozenset({"ready", "degraded", "blocked", "missing"})
_SELECTION_STRATEGIES = frozenset({"lexicographic_adapter_id", "weighted_evidence"})
_RETENTION = "metadata_only_coverage_selection_health_and_policy"
_EXECUTION = "readiness_projection_only;no_source_dispatch"


@dataclass(frozen=True, slots=True)
class AutonomousLLMEvidenceReadinessPolicy:
    """Caller-defined evidence-route usability thresholds.

    A policy is an audit criterion, not an authorization grant.  ``require_health=False`` is
    useful for startup and review screens where an unobserved route should be shown as degraded
    rather than blocking the entire UI; dispatch still requires the ordinary evidence approvals.
    """

    require_health: bool = True
    min_attempts: int = 1
    failure_threshold: float = 0.75
    min_success_rate: float = 0.5

    def __post_init__(self) -> None:
        if not isinstance(self.require_health, bool):
            raise ArgumentError("LLM evidence readiness require_health must be boolean")
        _integer("LLM evidence readiness min_attempts", self.min_attempts, 1, 1_000_000)
        _finite("LLM evidence readiness failure_threshold", self.failure_threshold, 0, 1)
        _finite("LLM evidence readiness min_success_rate", self.min_success_rate, 0, 1)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_READINESS_POLICY_SCHEMA,
            "require_health": self.require_health,
            "min_attempts": self.min_attempts,
            "failure_threshold": self.failure_threshold,
            "min_success_rate": self.min_success_rate,
            "execution": "audit_only;policy_does_not_authorize_source_dispatch",
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousLLMEvidenceReadinessPolicy":
        if not isinstance(value, Mapping):
            raise ArgumentError("LLM evidence readiness policy must be a mapping")
        allowed = {
            "schema", "require_health", "min_attempts", "failure_threshold", "min_success_rate",
            "execution", "retention", "secret_material",
        }
        if set(value) != allowed or value.get("schema") != AUTONOMOUS_LLM_EVIDENCE_READINESS_POLICY_SCHEMA:
            raise ArgumentError("LLM evidence readiness policy contains unsupported fields")
        if value.get("execution") != "audit_only;policy_does_not_authorize_source_dispatch" or value.get("retention") != _RETENTION or value.get("secret_material") != "never_returned":
            raise ArgumentError("LLM evidence readiness policy retention is invalid")
        policy = cls(
            require_health=value.get("require_health"),
            min_attempts=value.get("min_attempts"),
            failure_threshold=value.get("failure_threshold"),
            min_success_rate=value.get("min_success_rate"),
        )
        if canonical_json(value) != canonical_json(policy.to_dict()):
            raise ArgumentError("LLM evidence readiness policy is not canonical")
        return policy


@dataclass(frozen=True, slots=True)
class AutonomousLLMEvidenceReadinessHealth:
    """Secret-free health information associated with the selected manifest."""

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
            raise ArgumentError("LLM evidence readiness health observed must be boolean")
        _integer("LLM evidence readiness health attempts", self.attempts, 0, 1_000_000)
        _integer("LLM evidence readiness health successes", self.successes, 0, self.attempts)
        _integer("LLM evidence readiness health failures", self.failures, 0, self.attempts)
        if self.circuit not in {"closed", "open", "unknown"}:
            raise ArgumentError("LLM evidence readiness health circuit is invalid")
        if self.success_rate is not None:
            _finite("LLM evidence readiness health success_rate", self.success_rate, 0, 1)
        if self.failure_rate is not None:
            _finite("LLM evidence readiness health failure_rate", self.failure_rate, 0, 1)
        _optional_digest("LLM evidence readiness health manifest_digest", self.manifest_digest)
        if self.attempts == 0 and (self.success_rate is not None or self.failure_rate is not None):
            raise ArgumentError("LLM evidence readiness health rates require an observed attempt")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_READINESS_HEALTH_SCHEMA,
            "observed": self.observed,
            "attempts": self.attempts,
            "successes": self.successes,
            "failures": self.failures,
            "success_rate": self.success_rate,
            "failure_rate": self.failure_rate,
            "circuit": self.circuit,
            "manifest_digest": self.manifest_digest,
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousLLMEvidenceReadinessHealth":
        if not isinstance(value, Mapping):
            raise ArgumentError("LLM evidence readiness health must be a mapping")
        allowed = {
            "schema", "observed", "attempts", "successes", "failures", "success_rate", "failure_rate",
            "circuit", "manifest_digest", "retention", "secret_material",
        }
        if set(value) != allowed or value.get("schema") != AUTONOMOUS_LLM_EVIDENCE_READINESS_HEALTH_SCHEMA:
            raise ArgumentError("LLM evidence readiness health contains unsupported fields")
        if value.get("retention") != _RETENTION or value.get("secret_material") != "never_returned":
            raise ArgumentError("LLM evidence readiness health retention is invalid")
        health = cls(
            observed=value.get("observed"),
            attempts=value.get("attempts"),
            successes=value.get("successes"),
            failures=value.get("failures"),
            success_rate=value.get("success_rate"),
            failure_rate=value.get("failure_rate"),
            circuit=value.get("circuit"),
            manifest_digest=value.get("manifest_digest"),
        )
        if canonical_json(value) != canonical_json(health.to_dict()):
            raise ArgumentError("LLM evidence readiness health is not canonical")
        return health


def _unobserved_health(manifest_digest: str | None = None) -> AutonomousLLMEvidenceReadinessHealth:
    return AutonomousLLMEvidenceReadinessHealth(
        observed=False,
        attempts=0,
        successes=0,
        failures=0,
        success_rate=None,
        failure_rate=None,
        circuit="unknown",
        manifest_digest=manifest_digest,
    )


def _health_projection(
    row: Mapping[str, Any] | None,
    *,
    manifest_digest: str | None,
) -> AutonomousLLMEvidenceReadinessHealth:
    if row is None:
        return _unobserved_health(manifest_digest)
    attempts = _integer("LLM evidence readiness observed attempts", row.get("attempts"), 0, 1_000_000)
    successes = _integer("LLM evidence readiness observed successes", row.get("successes"), 0, attempts)
    failures = _integer("LLM evidence readiness observed failures", row.get("failures"), 0, attempts)
    return AutonomousLLMEvidenceReadinessHealth(
        observed=attempts > 0,
        attempts=attempts,
        successes=successes,
        failures=failures,
        success_rate=None if attempts == 0 else _finite("LLM evidence readiness observed success_rate", row.get("success_rate"), 0, 1),
        failure_rate=None if attempts == 0 else _finite("LLM evidence readiness observed failure_rate", row.get("failure_rate"), 0, 1),
        circuit=row.get("circuit"),
        manifest_digest=manifest_digest,
    )


@dataclass(frozen=True, slots=True)
class AutonomousLLMEvidenceReadinessDomain:
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
    health: AutonomousLLMEvidenceReadinessHealth
    failover_policy_digest: str
    reason: str

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES or self.status not in _READINESS_STATUSES:
            raise ArgumentError("LLM evidence readiness domain or status is invalid")
        if self.coverage_state not in {"complete", "missing"}:
            raise ArgumentError("LLM evidence readiness coverage state is invalid")
        if len(self.adapter_ids) > MAX_AUTONOMOUS_LLM_EVIDENCE_ADAPTERS or len(set(self.adapter_ids)) != len(self.adapter_ids):
            raise ArgumentError("LLM evidence readiness adapter ids exceed their bound or repeat")
        for index, adapter_id in enumerate(self.adapter_ids):
            _identifier(f"LLM evidence readiness adapter id {index}", adapter_id)
        if self.selected_adapter_id is not None:
            _identifier("LLM evidence readiness selected adapter id", self.selected_adapter_id)
            if self.selected_adapter_id not in self.adapter_ids:
                raise ArgumentError("LLM evidence readiness selected adapter is not in the candidate set")
        _optional_digest("LLM evidence readiness selected manifest digest", self.selected_manifest_digest)
        _integer("LLM evidence readiness candidate_count", self.candidate_count, 0, MAX_AUTONOMOUS_LLM_EVIDENCE_ADAPTERS)
        _integer("LLM evidence readiness eligible_candidate_count", self.eligible_candidate_count, 0, self.candidate_count)
        _identifier("LLM evidence readiness selection reason", self.selection_reason)
        if self.selection_strategy not in _SELECTION_STRATEGIES:
            raise ArgumentError("LLM evidence readiness selection strategy is invalid")
        if not isinstance(self.health, AutonomousLLMEvidenceReadinessHealth):
            raise ArgumentError("LLM evidence readiness health is malformed")
        if self.health.manifest_digest != self.selected_manifest_digest:
            raise ArgumentError("LLM evidence readiness health manifest does not match the selected manifest")
        _digest("LLM evidence readiness failover policy digest", self.failover_policy_digest)
        _identifier("LLM evidence readiness reason", self.reason)
        if self.status == "ready" and self.selected_adapter_id is None:
            raise ArgumentError("ready LLM evidence readiness row requires a selected adapter")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_READINESS_DOMAIN_SCHEMA,
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
            "failover_policy_digest": self.failover_policy_digest,
            "reason": self.reason,
            "execution": _EXECUTION,
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousLLMEvidenceReadinessDomain":
        if not isinstance(value, Mapping):
            raise ArgumentError("LLM evidence readiness domain must be a mapping")
        allowed = {
            "schema", "domain", "status", "coverage_state", "adapter_ids", "selected_adapter_id",
            "selected_manifest_digest", "candidate_count", "eligible_candidate_count", "selection_reason",
            "selection_strategy", "health", "failover_policy_digest", "reason", "execution", "retention",
            "secret_material",
        }
        if set(value) != allowed or value.get("schema") != AUTONOMOUS_LLM_EVIDENCE_READINESS_DOMAIN_SCHEMA:
            raise ArgumentError("LLM evidence readiness domain contains unsupported fields")
        if value.get("execution") != _EXECUTION or value.get("retention") != _RETENTION or value.get("secret_material") != "never_returned":
            raise ArgumentError("LLM evidence readiness domain retention is invalid")
        raw_ids = value.get("adapter_ids")
        if not isinstance(raw_ids, Sequence) or isinstance(raw_ids, (str, bytes)):
            raise ArgumentError("LLM evidence readiness adapter_ids must be a sequence")
        health = AutonomousLLMEvidenceReadinessHealth.from_dict(value.get("health"))
        row = cls(
            domain=_identifier("LLM evidence readiness domain", value.get("domain")),
            status=value.get("status"),
            coverage_state=value.get("coverage_state"),
            adapter_ids=tuple(_identifier("LLM evidence readiness adapter id", item) for item in raw_ids),
            selected_adapter_id=None if value.get("selected_adapter_id") is None else _identifier("LLM evidence readiness selected adapter id", value.get("selected_adapter_id")),
            selected_manifest_digest=value.get("selected_manifest_digest"),
            candidate_count=value.get("candidate_count"),
            eligible_candidate_count=value.get("eligible_candidate_count"),
            selection_reason=value.get("selection_reason"),
            selection_strategy=value.get("selection_strategy"),
            health=health,
            failover_policy_digest=value.get("failover_policy_digest"),
            reason=value.get("reason"),
        )
        if canonical_json(value) != canonical_json(row.to_dict()):
            raise ArgumentError("LLM evidence readiness domain is not canonical")
        return row


def _domain_reason(status: str, selection_reason: str, health: AutonomousLLMEvidenceReadinessHealth) -> str:
    if status == "missing":
        return (
            "no_registered_adapter_matches_domain_and_capability"
            if selection_reason == "no_matching_adapter"
            else "no_registered_adapter_matches_requested_readiness_scope"
        )
    if status == "blocked":
        if selection_reason != "lexicographic_adapter_id" and selection_reason != "weighted_evidence":
            return selection_reason
        if health.circuit == "open":
            return "selected_adapter_health_circuit_open"
        if not health.observed:
            return "selected_adapter_has_no_usable_health_observation"
        return "selected_adapter_health_below_readiness_threshold"
    if status == "degraded":
        return "selected_adapter_has_no_health_observation" if not health.observed else "selected_adapter_is_usable_but_health_is_below_strict_threshold"
    return "selected_adapter_has_current_manifest_and_usable_health"


@dataclass(frozen=True, slots=True)
class AutonomousLLMEvidenceReadinessReport:
    domains: tuple[AutonomousLLMEvidenceReadinessDomain, ...]
    registry_digest: str
    selection_plan_digest: str
    health_snapshot_digest: str | None
    policy: AutonomousLLMEvidenceReadinessPolicy
    failover_policy: AutonomousLLMEvidenceFailoverPolicy

    def __post_init__(self) -> None:
        if not 1 <= len(self.domains) <= MAX_AUTONOMOUS_LLM_EVIDENCE_READINESS_DOMAINS:
            raise ArgumentError("LLM evidence readiness report domains are outside their bound")
        if len({row.domain for row in self.domains}) != len(self.domains):
            raise ArgumentError("LLM evidence readiness report domains contain duplicates")
        if any(not isinstance(row, AutonomousLLMEvidenceReadinessDomain) for row in self.domains):
            raise ArgumentError("LLM evidence readiness report rows are malformed")
        _digest("LLM evidence readiness registry digest", self.registry_digest)
        _digest("LLM evidence readiness selection plan digest", self.selection_plan_digest)
        _optional_digest("LLM evidence readiness health snapshot digest", self.health_snapshot_digest)
        if not isinstance(self.policy, AutonomousLLMEvidenceReadinessPolicy):
            raise ArgumentError("LLM evidence readiness report policy is malformed")
        if not isinstance(self.failover_policy, AutonomousLLMEvidenceFailoverPolicy):
            raise ArgumentError("LLM evidence readiness report failover policy is malformed")

    @property
    def status(self) -> str:
        if any(row.status in {"blocked", "missing"} for row in self.domains):
            return "blocked"
        if any(row.status == "degraded" for row in self.domains):
            return "degraded"
        return "ready"

    @property
    def complete(self) -> bool:
        return self.status == "ready"

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

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_READINESS_SCHEMA,
            "domains": [row.to_dict() for row in self.domains],
            "registry_digest": self.registry_digest,
            "selection_plan_digest": self.selection_plan_digest,
            "health_snapshot_digest": self.health_snapshot_digest,
            "policy_digest": content_digest(self.policy.to_dict()),
            "readiness_policy": self.policy.to_dict(),
            "failover_policy": self.failover_policy.to_dict(),
            "status": self.status,
            "ready_count": self.ready_count,
            "degraded_count": self.degraded_count,
            "blocked_count": self.blocked_count,
            "missing_count": self.missing_count,
            "complete": self.complete,
            "execution": _EXECUTION,
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }

    @property
    def report_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        payload = {**self._payload(), "report_digest": self.report_digest}
        _json_bytes(payload, "LLM evidence readiness report", MAX_AUTONOMOUS_LLM_EVIDENCE_READINESS_BYTES)
        return payload

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousLLMEvidenceReadinessReport":
        if not isinstance(value, Mapping):
            raise ArgumentError("LLM evidence readiness report must be a mapping")
        allowed = {
            "schema", "domains", "registry_digest", "selection_plan_digest", "health_snapshot_digest",
            "policy_digest", "readiness_policy", "failover_policy", "status", "ready_count",
            "degraded_count", "blocked_count", "missing_count", "complete", "execution", "retention",
            "secret_material", "report_digest",
        }
        if set(value) != allowed or value.get("schema") != AUTONOMOUS_LLM_EVIDENCE_READINESS_SCHEMA:
            raise ArgumentError("LLM evidence readiness report contains unsupported fields")
        if value.get("execution") != _EXECUTION or value.get("retention") != _RETENTION or value.get("secret_material") != "never_returned":
            raise ArgumentError("LLM evidence readiness report retention is invalid")
        raw_domains = value.get("domains")
        if not isinstance(raw_domains, Sequence) or isinstance(raw_domains, (str, bytes)):
            raise ArgumentError("LLM evidence readiness report domains must be a sequence")
        rows = tuple(AutonomousLLMEvidenceReadinessDomain.from_dict(item) for item in raw_domains)
        policy = AutonomousLLMEvidenceReadinessPolicy.from_dict(value.get("readiness_policy"))
        raw_failover = value.get("failover_policy")
        if not isinstance(raw_failover, Mapping):
            raise ArgumentError("LLM evidence readiness failover policy must be a mapping")
        failover_allowed = {"schema", "max_failovers", "retry_policy", "execution", "retention", "secret_material"}
        if set(raw_failover) != failover_allowed or raw_failover.get("schema") != AUTONOMOUS_LLM_EVIDENCE_FAILOVER_POLICY_SCHEMA:
            raise ArgumentError("LLM evidence readiness failover policy is malformed")
        if raw_failover.get("execution") != "caller_controlled_reviewed_candidate_failover;no_fuzzy_selection" or raw_failover.get("retention") != "metadata_only_candidate_identity_and_failure_class" or raw_failover.get("secret_material") != "never_returned":
            raise ArgumentError("LLM evidence readiness failover policy retention is invalid")
        raw_retry = raw_failover.get("retry_policy")
        retry_allowed = {
            "schema", "max_attempts", "base_delay_ms", "max_delay_ms", "retryable_failure_classes",
            "execution", "retention", "secret_material",
        }
        if not isinstance(raw_retry, Mapping) or set(raw_retry) != retry_allowed:
            raise ArgumentError("LLM evidence readiness retry policy is malformed")
        if raw_retry.get("schema") != "bioprism-python-autonomous-evidence-retry-policy/0.1" or raw_retry.get("execution") != "caller_controlled_bounded_retry;no_authorization" or raw_retry.get("retention") != "metadata_only_policy;no_errors_or_values" or raw_retry.get("secret_material") != "never_returned":
            raise ArgumentError("LLM evidence readiness retry policy retention is invalid")
        retry_policy = AutonomousEvidenceRetryPolicy(
            max_attempts=raw_retry.get("max_attempts"),
            base_delay_ms=raw_retry.get("base_delay_ms"),
            max_delay_ms=raw_retry.get("max_delay_ms"),
            retryable_failure_classes=tuple(raw_retry.get("retryable_failure_classes", ())),
        )
        if canonical_json(raw_retry) != canonical_json(retry_policy.to_dict()):
            raise ArgumentError("LLM evidence readiness retry policy is not canonical")
        failover_policy = AutonomousLLMEvidenceFailoverPolicy(
            max_failovers=raw_failover.get("max_failovers"),
            retry_policy=retry_policy,
        )
        if canonical_json(raw_failover) != canonical_json(failover_policy.to_dict()):
            raise ArgumentError("LLM evidence readiness failover policy is not canonical")
        report = cls(
            domains=rows,
            registry_digest=_digest("LLM evidence readiness registry digest", value.get("registry_digest")),
            selection_plan_digest=_digest("LLM evidence readiness selection plan digest", value.get("selection_plan_digest")),
            health_snapshot_digest=_optional_digest("LLM evidence readiness health snapshot digest", value.get("health_snapshot_digest")),
            policy=policy,
            failover_policy=failover_policy,
        )
        expected_failover_digest = content_digest(failover_policy.to_dict())
        if any(row.failover_policy_digest != expected_failover_digest for row in report.domains):
            raise ArgumentError("LLM evidence readiness domain failover policy digest is inconsistent")
        if value.get("policy_digest") != content_digest(policy.to_dict()):
            raise ArgumentError("LLM evidence readiness policy digest is invalid")
        if value.get("status") != report.status or value.get("ready_count") != report.ready_count or value.get("degraded_count") != report.degraded_count or value.get("blocked_count") != report.blocked_count or value.get("missing_count") != report.missing_count or value.get("complete") != report.complete:
            raise ArgumentError("LLM evidence readiness report aggregates are inconsistent")
        if value.get("report_digest") != report.report_digest:
            raise ArgumentError("LLM evidence readiness report digest is invalid")
        if canonical_json(value) != canonical_json(report.to_dict()):
            raise ArgumentError("LLM evidence readiness report is not canonical")
        return report


def _selection_status(
    selection: Any,
    coverage_state: str,
    health: AutonomousLLMEvidenceReadinessHealth,
    policy: AutonomousLLMEvidenceReadinessPolicy,
) -> str:
    if selection.status == "missing":
        return "missing" if coverage_state == "missing" or selection.reason == "no_matching_adapter" else "blocked"
    if health.circuit == "open":
        return "blocked"
    healthy = (
        health.observed
        and health.attempts >= policy.min_attempts
        and (health.success_rate or 0.0) >= policy.min_success_rate
    )
    return "ready" if healthy else "blocked" if policy.require_health else "degraded"


class AutonomousLLMEvidenceReadinessAuditor:
    """Audit adapter coverage and health without dispatching evidence."""

    def __init__(
        self,
        registry: AutonomousLLMEvidenceAdapterRegistry,
        health_store: InMemoryAutonomousLLMEvidenceAdapterHealthStore | None = None,
    ) -> None:
        if not isinstance(registry, AutonomousLLMEvidenceAdapterRegistry):
            raise ArgumentError("LLM evidence readiness auditor requires a typed registry")
        if health_store is not None and not isinstance(health_store, InMemoryAutonomousLLMEvidenceAdapterHealthStore):
            raise ArgumentError("LLM evidence readiness auditor health store is malformed")
        self.registry = registry
        self.health_store = health_store
        self.selector = AutonomousLLMEvidenceAdapterSelector(registry)

    def audit(
        self,
        domains: Sequence[str] = AUTONOMOUS_DOMAIN_NAMES,
        *,
        selection_plan: AutonomousLLMEvidenceAdapterSelectionPlan | Mapping[str, Any] | None = None,
        adaptive_selection: bool = False,
        selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
        strategy: str = "lexicographic_adapter_id",
        capability: str | None = "llm_evidence",
        min_score: float = 0.0,
        min_margin: float = 0.0,
        policy: AutonomousLLMEvidenceReadinessPolicy | Mapping[str, Any] | None = None,
        failover_policy: AutonomousLLMEvidenceFailoverPolicy | None = None,
    ) -> AutonomousLLMEvidenceReadinessReport:
        requested = _domains(domains)
        if not isinstance(adaptive_selection, bool):
            raise ArgumentError("LLM evidence readiness adaptive_selection must be boolean")
        resolved_policy = (
            AutonomousLLMEvidenceReadinessPolicy()
            if policy is None
            else policy if isinstance(policy, AutonomousLLMEvidenceReadinessPolicy)
            else AutonomousLLMEvidenceReadinessPolicy.from_dict(policy)
        )
        resolved_failover = failover_policy or AutonomousLLMEvidenceFailoverPolicy()
        if not isinstance(resolved_failover, AutonomousLLMEvidenceFailoverPolicy):
            raise ArgumentError("LLM evidence readiness failover policy is malformed")
        if selection_plan is not None:
            plan = selection_plan if isinstance(selection_plan, AutonomousLLMEvidenceAdapterSelectionPlan) else AutonomousLLMEvidenceAdapterSelectionPlan.from_dict(selection_plan)
            if plan.domains != requested:
                raise ArgumentError("LLM evidence readiness selection plan domains do not match the audit request")
        elif adaptive_selection:
            if self.health_store is None:
                raise ArgumentError("adaptive LLM evidence readiness selection requires a health store")
            signals = selection_signals
            if signals is None:
                signals = self.health_store.selection_signals(
                    manifest_digests={manifest.adapter_id: manifest.manifest_digest for manifest in self.registry.manifests()}
                )
            plan = self.selector.select_adaptive_for_domains(
                requested,
                signals,
                capability=capability,
                min_score=min_score,
                min_margin=min_margin,
            )
        else:
            plan = self.selector.select_for_domains(
                requested,
                capability=capability,
                strategy=strategy,
                selection_signals=selection_signals,
                min_score=min_score,
                min_margin=min_margin,
            )
        self.registry.verify_selection(plan)
        health_snapshot = None if self.health_store is None else self.health_store.snapshot()
        rows: list[AutonomousLLMEvidenceReadinessDomain] = []
        for selection in plan.rows:
            candidates = self.registry.candidates(selection.domain, plan.capability)
            coverage_state = "complete" if candidates else "missing"
            health_row = None
            if self.health_store is not None and selection.adapter_id is not None and selection.manifest_digest is not None:
                health_values = self.health_store.health(
                    adapter_id=selection.adapter_id,
                    manifest_digest=selection.manifest_digest,
                    domain=selection.domain,
                    min_attempts=resolved_policy.min_attempts,
                    failure_threshold=resolved_policy.failure_threshold,
                    limit=1,
                )
                health_row = health_values[0] if health_values else None
            health = _health_projection(health_row, manifest_digest=selection.manifest_digest)
            status = _selection_status(selection, coverage_state, health, resolved_policy)
            rows.append(
                AutonomousLLMEvidenceReadinessDomain(
                    domain=selection.domain,
                    status=status,
                    coverage_state=coverage_state,
                    adapter_ids=tuple(selection.candidate_ids),
                    selected_adapter_id=selection.adapter_id,
                    selected_manifest_digest=selection.manifest_digest,
                    candidate_count=len(selection.candidate_ids),
                    eligible_candidate_count=sum(bool(item) for item in selection.candidate_eligible),
                    selection_reason=selection.reason,
                    selection_strategy=plan.strategy,
                    health=health,
                    failover_policy_digest=content_digest(resolved_failover.to_dict()),
                    reason=_domain_reason(status, selection.reason, health),
                )
            )
        return AutonomousLLMEvidenceReadinessReport(
            domains=tuple(rows),
            registry_digest=self.registry.registry_digest,
            selection_plan_digest=plan.plan_digest,
            health_snapshot_digest=None if health_snapshot is None else health_snapshot["snapshot_digest"],
            policy=resolved_policy,
            failover_policy=resolved_failover,
        )


__all__ = [
    "AUTONOMOUS_LLM_EVIDENCE_READINESS_SCHEMA",
    "AUTONOMOUS_LLM_EVIDENCE_READINESS_DOMAIN_SCHEMA",
    "AUTONOMOUS_LLM_EVIDENCE_READINESS_POLICY_SCHEMA",
    "AUTONOMOUS_LLM_EVIDENCE_READINESS_HEALTH_SCHEMA",
    "MAX_AUTONOMOUS_LLM_EVIDENCE_READINESS_DOMAINS",
    "MAX_AUTONOMOUS_LLM_EVIDENCE_READINESS_BYTES",
    "AutonomousLLMEvidenceReadinessPolicy",
    "AutonomousLLMEvidenceReadinessHealth",
    "AutonomousLLMEvidenceReadinessDomain",
    "AutonomousLLMEvidenceReadinessReport",
    "AutonomousLLMEvidenceReadinessAuditor",
]
