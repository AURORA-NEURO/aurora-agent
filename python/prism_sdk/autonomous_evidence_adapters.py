"""Generic, metadata-only evidence adapter selection and failover.

The LLM evidence adapter orchestration is intentionally provider-specific.  This module is the
provider-neutral companion for file, browser, database, scientific, enterprise, and connector
backed evidence sources.  Applications register the actual adapter callable; the SDK retains only
its reviewed manifest and bounded outcome metadata.

Selection is a planning decision, never source authorization.  Health is an observation overlay,
not evaluator truth.  Failover is explicitly budgeted and uses the existing typed retry policy;
it never silently discovers an unreviewed source.  Persisted snapshots contain no acquired values,
prompts, credentials, headers, arguments, responses, or exception messages.
"""

from __future__ import annotations

from dataclasses import dataclass
import copy
import json
import math
import threading
import time
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence_retry import (
    AutonomousEvidenceRetryAttempt,
    AutonomousEvidenceRetryClassification,
    AutonomousEvidenceRetryPolicy,
    classify_autonomous_evidence_acquisition_error,
    create_autonomous_evidence_retrying_acquirer,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_EVIDENCE_ADAPTER_REGISTRY_SCHEMA = "bioprism-python-autonomous-evidence-adapter-registry/0.1"
AUTONOMOUS_EVIDENCE_ADAPTER_MANIFEST_SCHEMA = "bioprism-python-autonomous-evidence-adapter-manifest/0.1"
AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SCHEMA = "bioprism-python-autonomous-evidence-adapter-selection/0.1"
AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_ROW_SCHEMA = "bioprism-python-autonomous-evidence-adapter-selection-row/0.1"
AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SCHEMA = "bioprism-python-autonomous-evidence-adapter-health/0.1"
AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_OBSERVATION_SCHEMA = "bioprism-python-autonomous-evidence-adapter-health-observation/0.1"
AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA = "bioprism-python-autonomous-evidence-adapter-health-event/0.1"
AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_RECEIPT_SCHEMA = "bioprism-python-autonomous-evidence-adapter-health-receipt/0.1"
AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-evidence-adapter-health-snapshot/0.1"
AUTONOMOUS_EVIDENCE_FAILOVER_POLICY_SCHEMA = "bioprism-python-autonomous-evidence-failover-policy/0.1"
AUTONOMOUS_EVIDENCE_FAILOVER_EVENT_SCHEMA = "bioprism-python-autonomous-evidence-failover-event/0.1"

MAX_AUTONOMOUS_EVIDENCE_ADAPTERS = 256
MAX_AUTONOMOUS_EVIDENCE_ADAPTER_DOMAINS = len(AUTONOMOUS_DOMAIN_NAMES)
MAX_AUTONOMOUS_EVIDENCE_ADAPTER_CAPABILITIES = 64
MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SOURCE_KINDS = 32
MAX_AUTONOMOUS_EVIDENCE_ADAPTER_REGISTRY_BYTES = 256_000
MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_CANDIDATES = 256
MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SIGNAL_BYTES = 64_000
MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS = 16_384
MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_QUERY_LIMIT = 512
MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_BYTES = 512_000
MAX_AUTONOMOUS_EVIDENCE_FAILOVERS = 7

AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_STRATEGIES = ("lexicographic_adapter_id", "weighted_evidence")
_HEALTH_OUTCOMES = frozenset({"success", "failure", "unknown"})
_HEALTH_KINDS = frozenset({"acquisition", "evaluation"})
_RETENTION = "metadata_only;raw_source_values_credentials_prompts_and_errors_never_persisted"
_MANIFEST_RETENTION = "manifest_only;credentials_and_raw_source_values_never_persisted"
_SELECTION_RETENTION = "metadata_only_manifest_and_health_evidence"
_IDENTIFIER_CHARS = frozenset("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+-")
_DIGEST_CHARS = frozenset("0123456789abcdef")
_FORBIDDEN_KEYS = frozenset({
    "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
    "token", "privatekey", "prompt", "messages", "response", "raw", "rawvalue", "payload",
    "arguments", "output", "task", "content", "body", "headers", "input",
})
_SELECTION_ROW_KEYS = frozenset({
    "schema", "domain", "status", "adapter_id", "manifest_digest", "candidate_ids",
    "candidate_manifest_digests", "candidate_scores", "candidate_eligible", "reason",
    "retention", "secret_material",
})
_SELECTION_PLAN_KEYS = frozenset({
    "schema", "domains", "capability", "registry_digest", "rows", "strategy", "signal_digest",
    "complete", "plan_digest", "execution", "retention", "secret_material",
})
_HEALTH_OBSERVATION_KEYS = frozenset({
    "schema", "adapter_id", "manifest_digest", "domain", "observation_kind", "outcome", "status",
    "latency_ms", "cost_units", "failure_class", "evaluator_reward", "evaluator_passed",
    "evaluator_id", "evaluator_version", "evidence_digest", "retention", "secret_material",
})
_HEALTH_EVENT_KEYS = frozenset({
    "schema", "sequence", "observation", "previous_digest", "created_at", "event_digest",
    "retention", "secret_material",
})
_HEALTH_SNAPSHOT_KEYS = frozenset({
    "schema", "sequence", "head_digest", "events", "snapshot_digest", "retention", "secret_material",
})


def _text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value.strip()


def _identifier(name: str, value: Any, maximum: int = 256) -> str:
    result = _text(name, value, maximum)
    if any(character not in _IDENTIFIER_CHARS for character in result):
        raise ArgumentError(f"{name} is outside its identifier contract")
    return result


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in _DIGEST_CHARS for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _finite(name: str, value: Any, minimum: float, maximum: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be between {minimum} and {maximum}")
    result = float(value)
    if result < minimum or result > maximum:
        raise ArgumentError(f"{name} must be between {minimum} and {maximum}")
    return result


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum or value > maximum:
        raise ArgumentError(f"{name} must be an integer between {minimum} and {maximum}")
    return value


def _domains(name: str, value: Any) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or not 1 <= len(value) <= MAX_AUTONOMOUS_EVIDENCE_ADAPTER_DOMAINS:
        raise ArgumentError(f"{name} must contain 1..{MAX_AUTONOMOUS_EVIDENCE_ADAPTER_DOMAINS} domains")
    result = tuple(_identifier(f"{name}[{index}]", item) for index, item in enumerate(value))
    if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in result):
        raise ArgumentError(f"{name} contains an unsupported domain")
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate domains")
    return result


def _bounded_list(name: str, value: Any, maximum: int) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or not 1 <= len(value) <= maximum:
        raise ArgumentError(f"{name} must contain 1..{maximum} entries")
    result = tuple(sorted(_identifier(f"{name}[{index}]", item) for index, item in enumerate(value)))
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate entries")
    return result


def _safe_metadata(value: Any, name: str = "adapter metadata", depth: int = 0) -> None:
    if depth > 16:
        raise ArgumentError(f"{name} is too deeply nested")
    if isinstance(value, Mapping):
        if len(value) > 512:
            raise ArgumentError(f"{name} contains too many fields")
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip() or "\x00" in key:
                raise ArgumentError(f"{name} contains an invalid key")
            normalized = "".join(character for character in key.lower() if character.isalnum())
            if normalized in _FORBIDDEN_KEYS:
                raise ArgumentError(f"{name} contains transient or secret-shaped fields")
            _safe_metadata(child, f"{name}.{key}", depth + 1)
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        if len(value) > 512:
            raise ArgumentError(f"{name} contains too many entries")
        for index, child in enumerate(value):
            _safe_metadata(child, f"{name}[{index}]", depth + 1)
    elif isinstance(value, float) and not math.isfinite(value):
        raise ArgumentError(f"{name} contains a non-finite number")


def _json_bytes(value: Any, name: str, maximum: int) -> None:
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be canonical JSON") from error
    if len(encoded) > maximum:
        raise ArgumentError(f"{name} exceeds its byte bound")


def _copy(value: Any) -> Any:
    return copy.deepcopy(value)


def _manifest_descriptor(adapter_id: str, version: str, domains: Sequence[str], capabilities: Sequence[str], source_kinds: Sequence[str]) -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_EVIDENCE_ADAPTER_MANIFEST_SCHEMA,
        "adapter_id": adapter_id,
        "version": version,
        "domains": list(domains),
        "capabilities": list(capabilities),
        "source_kinds": list(source_kinds),
        "execution": "caller_owned_source_adapter;raw_value_transient",
        "retention": _MANIFEST_RETENTION,
        "secret_material": "never_returned",
    }


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceAdapterManifest:
    adapter_id: str
    version: str
    domains: tuple[str, ...]
    capabilities: tuple[str, ...]
    source_kinds: tuple[str, ...]
    manifest_digest: str

    def __post_init__(self) -> None:
        adapter_id = _identifier("evidence adapter manifest adapter_id", self.adapter_id)
        version = _identifier("evidence adapter manifest version", self.version)
        domains = _domains("evidence adapter manifest domains", self.domains)
        capabilities = _bounded_list("evidence adapter manifest capabilities", self.capabilities, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_CAPABILITIES)
        source_kinds = _bounded_list("evidence adapter manifest source_kinds", self.source_kinds, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SOURCE_KINDS)
        _digest("evidence adapter manifest manifest_digest", self.manifest_digest)
        descriptor = _manifest_descriptor(adapter_id, version, tuple(sorted(domains)), capabilities, source_kinds)
        if content_digest(descriptor) != self.manifest_digest:
            raise ArgumentError("evidence adapter manifest digest is invalid")
        object.__setattr__(self, "adapter_id", adapter_id)
        object.__setattr__(self, "version", version)
        object.__setattr__(self, "domains", tuple(sorted(domains)))
        object.__setattr__(self, "capabilities", capabilities)
        object.__setattr__(self, "source_kinds", source_kinds)

    def to_dict(self) -> dict[str, Any]:
        return {**_manifest_descriptor(self.adapter_id, self.version, self.domains, self.capabilities, self.source_kinds), "manifest_digest": self.manifest_digest}


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceAdapterCoverage:
    domain: str
    adapter_ids: tuple[str, ...]
    capability_union: tuple[str, ...]
    state: str

    def to_dict(self) -> dict[str, Any]:
        return {"domain": self.domain, "adapter_ids": list(self.adapter_ids), "capability_union": list(self.capability_union), "state": self.state}


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceAdapterRegistration:
    adapter_id: str
    version: str
    domains: tuple[str, ...]
    capabilities: tuple[str, ...]
    source_kinds: tuple[str, ...]
    acquire: Callable[[Mapping[str, Any]], Any]
    project: Callable[[Any, Mapping[str, Any]], Any] | None = None


@dataclass(frozen=True, slots=True)
class _AdapterEntry:
    manifest: AutonomousEvidenceAdapterManifest
    acquire: Callable[[Mapping[str, Any]], Any]
    project: Callable[[Any, Mapping[str, Any]], Any] | None


class AutonomousEvidenceAdapterRegistry:
    """Digest-bound registry of caller-owned, domain-scoped source adapters."""

    def __init__(self, registrations: Sequence[Any] = ()) -> None:
        if isinstance(registrations, (str, bytes, bytearray)) or not isinstance(registrations, Sequence):
            raise ArgumentError("evidence adapter registrations must be a sequence")
        if len(registrations) > MAX_AUTONOMOUS_EVIDENCE_ADAPTERS:
            raise ArgumentError("evidence adapter registry is full")
        self._entries: dict[str, _AdapterEntry] = {}
        for registration in registrations:
            self.register(registration)

    def register(
        self,
        registration: AutonomousEvidenceAdapterRegistration | Mapping[str, Any] | None = None,
        *,
        adapter_id: str | None = None,
        version: str | None = None,
        domains: Sequence[str] | None = None,
        capabilities: Sequence[str] | None = None,
        source_kinds: Sequence[str] | None = None,
        acquire: Callable[[Mapping[str, Any]], Any] | None = None,
        project: Callable[[Any, Mapping[str, Any]], Any] | None = None,
        replace: bool = False,
    ) -> AutonomousEvidenceAdapterManifest:
        if registration is not None:
            if any(value is not None for value in (adapter_id, version, domains, capabilities, source_kinds, acquire, project)):
                raise ArgumentError("evidence adapter registration cannot mix positional and keyword forms")
            if isinstance(registration, AutonomousEvidenceAdapterRegistration):
                adapter_id, version, domains, capabilities, source_kinds, acquire, project = (
                    registration.adapter_id, registration.version, registration.domains, registration.capabilities,
                    registration.source_kinds, registration.acquire, registration.project,
                )
            elif isinstance(registration, Mapping):
                adapter_id = registration.get("adapter_id", registration.get("adapterId"))
                version = registration.get("version")
                domains = registration.get("domains")
                capabilities = registration.get("capabilities")
                source_kinds = registration.get("source_kinds", registration.get("sourceKinds"))
                acquire = registration.get("acquire")
                project = registration.get("project")
            else:
                raise ArgumentError("evidence adapter registration is malformed")
        if not callable(acquire):
            raise ArgumentError("evidence adapter registration requires an acquire callback")
        if project is not None and not callable(project):
            raise ArgumentError("evidence adapter registration project callback is malformed")
        normalized_id = _identifier("evidence adapter adapter_id", adapter_id)
        normalized_version = _identifier("evidence adapter version", version)
        normalized_domains = tuple(sorted(_domains("evidence adapter domains", domains)))
        normalized_capabilities = _bounded_list("evidence adapter capabilities", capabilities, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_CAPABILITIES)
        normalized_source_kinds = _bounded_list("evidence adapter source_kinds", source_kinds, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SOURCE_KINDS)
        descriptor = _manifest_descriptor(normalized_id, normalized_version, normalized_domains, normalized_capabilities, normalized_source_kinds)
        manifest = AutonomousEvidenceAdapterManifest(
            normalized_id, normalized_version, normalized_domains, normalized_capabilities, normalized_source_kinds, content_digest(descriptor)
        )
        if normalized_id in self._entries and not replace:
            raise ArgumentError(f"evidence adapter {normalized_id} is already registered")
        if normalized_id not in self._entries and len(self._entries) >= MAX_AUTONOMOUS_EVIDENCE_ADAPTERS:
            raise ArgumentError("evidence adapter registry is full")
        self._entries[normalized_id] = _AdapterEntry(manifest, acquire, project)
        self._assert_size()
        return _copy(manifest)

    def unregister(self, adapter_id: str) -> bool:
        return self._entries.pop(_identifier("evidence adapter adapter_id", adapter_id), None) is not None

    def manifests(self) -> tuple[AutonomousEvidenceAdapterManifest, ...]:
        return tuple(_copy(self._entries[key].manifest) for key in sorted(self._entries))

    def coverage(self) -> tuple[AutonomousEvidenceAdapterCoverage, ...]:
        return tuple(
            AutonomousEvidenceAdapterCoverage(
                domain,
                tuple(sorted(entry.manifest.adapter_id for entry in self._entries.values() if domain in entry.manifest.domains)),
                tuple(sorted({capability for entry in self._entries.values() if domain in entry.manifest.domains for capability in entry.manifest.capabilities})),
                "complete" if any(domain in entry.manifest.domains for entry in self._entries.values()) else "missing",
            )
            for domain in AUTONOMOUS_DOMAIN_NAMES
        )

    @property
    def registry_digest(self) -> str:
        return content_digest(self._registry_descriptor())

    def _registry_descriptor(self) -> dict[str, Any]:
        coverage = [item.to_dict() for item in self.coverage()]
        return {
            "schema": AUTONOMOUS_EVIDENCE_ADAPTER_REGISTRY_SCHEMA,
            "adapters": [manifest.to_dict() for manifest in self.manifests()],
            "coverage": coverage,
            "coverage_digest": content_digest(coverage),
            "execution": "registry_projection_only;no_source_dispatch",
            "retention": _MANIFEST_RETENTION,
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        descriptor = self._registry_descriptor()
        _json_bytes(descriptor, "evidence adapter registry", MAX_AUTONOMOUS_EVIDENCE_ADAPTER_REGISTRY_BYTES)
        return {**descriptor, "registry_digest": content_digest(descriptor)}

    def _assert_size(self) -> None:
        self.to_dict()

    def candidates(self, domain: str, capability: str | None = None) -> tuple[AutonomousEvidenceAdapterManifest, ...]:
        normalized_domain = _identifier("evidence adapter candidate domain", domain)
        if normalized_domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("evidence adapter candidate domain is unsupported")
        normalized_capability = None if capability is None else _identifier("evidence adapter candidate capability", capability)
        candidates = tuple(manifest for manifest in self.manifests() if normalized_domain in manifest.domains and (normalized_capability is None or normalized_capability in manifest.capabilities))
        if len(candidates) > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_CANDIDATES:
            raise ArgumentError("evidence adapter candidate set exceeds its bound")
        return candidates

    def resolve(self, domain: str, adapter_id: str | None = None) -> AutonomousEvidenceAdapterManifest:
        normalized_domain = _identifier("evidence adapter resolution domain", domain)
        if normalized_domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("evidence adapter resolution domain is unsupported")
        candidates = self.candidates(normalized_domain)
        if adapter_id is not None:
            normalized_id = _identifier("evidence adapter selected adapter_id", adapter_id)
            entry = self._entries.get(normalized_id)
            if entry is None or normalized_domain not in entry.manifest.domains:
                raise ArgumentError(f"evidence adapter {normalized_id} is not registered for {normalized_domain}")
            return _copy(entry.manifest)
        if not candidates:
            raise ArgumentError(f"no evidence adapter is registered for {normalized_domain}")
        if len(candidates) > 1:
            raise ArgumentError(f"evidence adapter selection is ambiguous for {normalized_domain}")
        return _copy(candidates[0])

    def create_acquirer(self, adapter_id_for_domain: Mapping[str, str] | None = None) -> Any:
        if adapter_id_for_domain is not None and not isinstance(adapter_id_for_domain, Mapping):
            raise ArgumentError("evidence adapter acquirer routes are malformed")
        registry = self

        class Acquirer:
            def acquire(self, context: Mapping[str, Any]) -> Any:
                if not isinstance(context, Mapping):
                    raise ArgumentError("evidence adapter acquire context is malformed")
                requirement = context.get("requirement")
                domain = requirement.get("domain") if isinstance(requirement, Mapping) else getattr(requirement, "domain", None)
                if domain not in AUTONOMOUS_DOMAIN_NAMES:
                    raise ArgumentError("evidence adapter acquire context has an unsupported domain")
                requested = None if adapter_id_for_domain is None else adapter_id_for_domain.get(domain)
                manifest = registry.resolve(domain, requested)
                entry = registry._entries.get(manifest.adapter_id)
                if entry is None:
                    raise ArgumentError("evidence adapter disappeared during resolution")
                return entry.acquire(context)

        return Acquirer()

    def create_projector(self, adapter_id_for_domain: Mapping[str, str] | None = None) -> Any:
        if adapter_id_for_domain is not None and not isinstance(adapter_id_for_domain, Mapping):
            raise ArgumentError("evidence adapter projector routes are malformed")
        registry = self

        class Projector:
            def project(self, value: Any, context: Mapping[str, Any]) -> Any:
                if not isinstance(context, Mapping):
                    raise ArgumentError("evidence adapter project context is malformed")
                requirement = context.get("requirement")
                domain = requirement.get("domain") if isinstance(requirement, Mapping) else getattr(requirement, "domain", None)
                if domain not in AUTONOMOUS_DOMAIN_NAMES:
                    raise ArgumentError("evidence adapter project context has an unsupported domain")
                requested = None if adapter_id_for_domain is None else adapter_id_for_domain.get(domain)
                manifest = registry.resolve(domain, requested)
                entry = registry._entries.get(manifest.adapter_id)
                if entry is None or entry.project is None:
                    raise ArgumentError(f"evidence adapter {manifest.adapter_id} does not provide a projector")
                return entry.project(value, context)

        return Projector()

    def verify_selection(self, plan: "AutonomousEvidenceAdapterSelectionPlan") -> None:
        if not isinstance(plan, AutonomousEvidenceAdapterSelectionPlan):
            raise ArgumentError("evidence adapter selection plan is malformed")
        plan.verify(self)


def register_autonomous_evidence_adapters_for_all_domains(
    registry: AutonomousEvidenceAdapterRegistry,
    factory: Callable[[str], Mapping[str, Any] | AutonomousEvidenceAdapterRegistration],
    *,
    replace: bool = False,
) -> tuple[AutonomousEvidenceAdapterManifest, ...]:
    if not isinstance(registry, AutonomousEvidenceAdapterRegistry) or not callable(factory):
        raise ArgumentError("all-domain evidence adapter registration requires a registry and factory")
    manifests: list[AutonomousEvidenceAdapterManifest] = []
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        registration = factory(domain)
        if isinstance(registration, AutonomousEvidenceAdapterRegistration):
            if domain not in registration.domains:
                registration = AutonomousEvidenceAdapterRegistration(registration.adapter_id, registration.version, (domain,), registration.capabilities, registration.source_kinds, registration.acquire, registration.project)
        elif isinstance(registration, Mapping):
            registration = dict(registration)
            registration.setdefault("domains", (domain,))
        else:
            raise ArgumentError(f"evidence adapter factory returned no registration for {domain}")
        manifests.append(registry.register(registration, replace=replace))
    return tuple(manifests)


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceAdapterSelectionSignal:
    adapter_id: str
    eligible: bool
    health: float
    success_rate: float
    evaluator_reward: float
    latency_ms: float | None
    cost_units: float | None
    score: float

    def to_dict(self) -> dict[str, Any]:
        return {"adapter_id": self.adapter_id, "eligible": self.eligible, "health": self.health, "success_rate": self.success_rate, "evaluator_reward": self.evaluator_reward, "latency_ms": self.latency_ms, "cost_units": self.cost_units, "score": self.score}


def _normalize_signal(adapter_id: str, raw: Mapping[str, Any] | None) -> AutonomousEvidenceAdapterSelectionSignal:
    if raw is not None:
        _safe_metadata(raw, f"evidence adapter selection signal {adapter_id}")
        allowed = {"eligible", "health", "success_rate", "evaluator_reward", "latency_ms", "cost_units"}
        if set(raw) - allowed:
            raise ArgumentError(f"evidence adapter selection signal for {adapter_id} contains unsupported fields")
    missing = raw is None
    eligible = False if missing else raw.get("eligible", True)
    if not isinstance(eligible, bool):
        raise ArgumentError(f"evidence adapter selection signal {adapter_id} eligible must be boolean")
    health = _finite(f"evidence adapter selection signal {adapter_id} health", 0 if missing else raw.get("health", 0), 0, 1)
    success_rate = _finite(f"evidence adapter selection signal {adapter_id} success_rate", 0 if missing else raw.get("success_rate", health), 0, 1)
    reward = _finite(f"evidence adapter selection signal {adapter_id} evaluator_reward", 0 if missing else raw.get("evaluator_reward", 0), -1, 1)
    latency = None if missing or raw.get("latency_ms") is None else _finite(f"evidence adapter selection signal {adapter_id} latency_ms", raw["latency_ms"], 0, 86_400_000)
    cost = None if missing or raw.get("cost_units") is None else _finite(f"evidence adapter selection signal {adapter_id} cost_units", raw["cost_units"], 0, 1_000_000)
    latency_score = 0.5 if latency is None else 1 / (1 + latency / 1_000)
    cost_score = 0.5 if cost is None else 1 / (1 + cost / 100)
    score = round(0.35 * health + 0.25 * success_rate + 0.25 * ((reward + 1) / 2) + 0.10 * latency_score + 0.05 * cost_score, 12)
    return AutonomousEvidenceAdapterSelectionSignal(adapter_id, eligible, health, success_rate, reward, latency, cost, score)


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceAdapterSelectionRow:
    domain: str
    status: str
    adapter_id: str | None
    manifest_digest: str | None
    candidate_ids: tuple[str, ...]
    candidate_manifest_digests: tuple[str, ...]
    candidate_scores: tuple[float, ...]
    candidate_eligible: tuple[bool, ...]
    reason: str

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES or self.status not in {"selected", "missing"}:
            raise ArgumentError("evidence adapter selection row domain or status is invalid")
        if len(self.candidate_ids) > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_CANDIDATES:
            raise ArgumentError("evidence adapter selection row exceeds its candidate bound")
        if not (len(self.candidate_ids) == len(self.candidate_manifest_digests) == len(self.candidate_scores) == len(self.candidate_eligible)):
            raise ArgumentError("evidence adapter selection row candidate arrays must align")
        ids = tuple(_identifier(f"evidence adapter selection candidate {index}", value) for index, value in enumerate(self.candidate_ids))
        digests = tuple(_digest(f"evidence adapter selection candidate digest {index}", value) for index, value in enumerate(self.candidate_manifest_digests))
        scores = tuple(_finite(f"evidence adapter selection candidate score {index}", value, 0, 1) for index, value in enumerate(self.candidate_scores))
        eligible = tuple(self.candidate_eligible)
        if len(set(ids)) != len(ids) or any(not isinstance(value, bool) for value in eligible):
            raise ArgumentError("evidence adapter selection row candidate metadata is invalid")
        if self.status == "selected":
            if self.adapter_id is None or self.manifest_digest is None:
                raise ArgumentError("selected evidence adapter row requires adapter and manifest identities")
            selected_id = _identifier("evidence adapter selection adapter_id", self.adapter_id)
            selected_digest = _digest("evidence adapter selection manifest_digest", self.manifest_digest)
            index = ids.index(selected_id) if selected_id in ids else -1
            if index < 0 or not eligible[index] or digests[index] != selected_digest:
                raise ArgumentError("selected evidence adapter row does not match an eligible candidate")
            object.__setattr__(self, "adapter_id", selected_id)
            object.__setattr__(self, "manifest_digest", selected_digest)
        elif self.adapter_id is not None or self.manifest_digest is not None:
            raise ArgumentError("missing evidence adapter row cannot select an adapter")
        object.__setattr__(self, "candidate_ids", ids)
        object.__setattr__(self, "candidate_manifest_digests", digests)
        object.__setattr__(self, "candidate_scores", scores)
        object.__setattr__(self, "candidate_eligible", eligible)
        object.__setattr__(self, "reason", _identifier("evidence adapter selection row reason", self.reason))

    def to_dict(self) -> dict[str, Any]:
        return {"schema": AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_ROW_SCHEMA, "domain": self.domain, "status": self.status, "adapter_id": self.adapter_id, "manifest_digest": self.manifest_digest, "candidate_ids": list(self.candidate_ids), "candidate_manifest_digests": list(self.candidate_manifest_digests), "candidate_scores": list(self.candidate_scores), "candidate_eligible": list(self.candidate_eligible), "reason": self.reason, "retention": _SELECTION_RETENTION, "secret_material": "never_returned"}


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceAdapterSelectionPlan:
    domains: tuple[str, ...]
    capability: str | None
    registry_digest: str
    rows: tuple[AutonomousEvidenceAdapterSelectionRow, ...]
    strategy: str = "lexicographic_adapter_id"
    signal_digest: str | None = None

    def __post_init__(self) -> None:
        normalized_domains = _domains("evidence adapter selection domains", self.domains)
        if tuple(row.domain for row in self.rows) != normalized_domains or any(not isinstance(row, AutonomousEvidenceAdapterSelectionRow) for row in self.rows):
            raise ArgumentError("evidence adapter selection rows must align with domains")
        if self.capability is not None:
            _identifier("evidence adapter selection capability", self.capability)
        _digest("evidence adapter selection registry_digest", self.registry_digest)
        if self.strategy not in AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_STRATEGIES:
            raise ArgumentError("evidence adapter selection strategy is invalid")
        _digest("evidence adapter selection signal_digest", self.signal_digest, allow_none=True)
        object.__setattr__(self, "domains", normalized_domains)

    @property
    def complete(self) -> bool:
        return all(row.status == "selected" for row in self.rows)

    def _payload(self) -> dict[str, Any]:
        return {"schema": AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SCHEMA, "domains": list(self.domains), "capability": self.capability, "registry_digest": self.registry_digest, "rows": [row.to_dict() for row in self.rows], "strategy": self.strategy, "signal_digest": self.signal_digest}

    @property
    def plan_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "complete": self.complete, "plan_digest": self.plan_digest, "execution": "planning_only;selection_does_not_authorize_source_dispatch", "retention": _SELECTION_RETENTION, "secret_material": "never_returned"}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousEvidenceAdapterSelectionPlan":
        if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SCHEMA:
            raise ArgumentError("evidence adapter selection plan is malformed")
        _safe_metadata(value, "evidence adapter selection plan")
        if set(value) != _SELECTION_PLAN_KEYS:
            raise ArgumentError("evidence adapter selection plan contains unsupported fields")
        if value.get("execution") != "planning_only;selection_does_not_authorize_source_dispatch" or value.get("retention") != _SELECTION_RETENTION or value.get("secret_material") != "never_returned":
            raise ArgumentError("evidence adapter selection plan retention is invalid")
        raw_rows = value.get("rows")
        if not isinstance(raw_rows, Sequence) or isinstance(raw_rows, (str, bytes, bytearray)):
            raise ArgumentError("evidence adapter selection plan rows are malformed")
        rows: list[AutonomousEvidenceAdapterSelectionRow] = []
        for raw in raw_rows:
            if not isinstance(raw, Mapping) or raw.get("schema") != AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_ROW_SCHEMA or raw.get("retention") != _SELECTION_RETENTION or raw.get("secret_material") != "never_returned":
                raise ArgumentError("evidence adapter selection row is malformed")
            if set(raw) != _SELECTION_ROW_KEYS:
                raise ArgumentError("evidence adapter selection row contains unsupported fields")
            rows.append(AutonomousEvidenceAdapterSelectionRow(raw["domain"], raw["status"], raw.get("adapter_id"), raw.get("manifest_digest"), tuple(raw.get("candidate_ids", ())), tuple(raw.get("candidate_manifest_digests", ())), tuple(raw.get("candidate_scores", ())), tuple(raw.get("candidate_eligible", ())), raw["reason"]))
        plan = cls(tuple(value.get("domains", ())), value.get("capability"), value.get("registry_digest"), tuple(rows), value.get("strategy"), value.get("signal_digest"))
        if value.get("complete") != plan.complete or value.get("plan_digest") != plan.plan_digest:
            raise ArgumentError("evidence adapter selection plan digest or completeness is invalid")
        return plan

    def verify(self, registry: AutonomousEvidenceAdapterRegistry) -> "AutonomousEvidenceAdapterSelectionPlan":
        if not isinstance(registry, AutonomousEvidenceAdapterRegistry):
            raise ArgumentError("evidence adapter selection verification requires a typed registry")
        if self.registry_digest != registry.registry_digest:
            raise ArgumentError("evidence adapter selection registry is stale or tampered")
        for row in self.rows:
            candidates = registry.candidates(row.domain, self.capability)
            if tuple(item.adapter_id for item in candidates) != row.candidate_ids or tuple(item.manifest_digest for item in candidates) != row.candidate_manifest_digests:
                raise ArgumentError("evidence adapter selection candidate set changed")
            if row.status == "selected" and registry.resolve(row.domain, row.adapter_id).manifest_digest != row.manifest_digest:
                raise ArgumentError("evidence adapter selected manifest changed")
        return self


class AutonomousEvidenceAdapterSelector:
    def __init__(self, registry: AutonomousEvidenceAdapterRegistry) -> None:
        if not isinstance(registry, AutonomousEvidenceAdapterRegistry):
            raise ArgumentError("evidence adapter selector requires a typed registry")
        self.registry = registry

    def select_for_domains(
        self,
        requested_domains: Sequence[str],
        *,
        capability: str | None = None,
        strategy: str = "lexicographic_adapter_id",
        selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
        min_score: float = 0,
        min_margin: float = 0,
    ) -> AutonomousEvidenceAdapterSelectionPlan:
        requested = _domains("evidence adapter selection domains", requested_domains)
        selected_capability = None if capability is None else _identifier("evidence adapter selection capability", capability)
        if strategy not in AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_STRATEGIES:
            raise ArgumentError("evidence adapter selector strategy is invalid")
        if strategy == "lexicographic_adapter_id" and selection_signals is not None:
            raise ArgumentError("lexicographic evidence adapter selection cannot consume signals")
        if strategy == "weighted_evidence" and selection_signals is None:
            raise ArgumentError("weighted evidence adapter selection requires explicit signals")
        minimum_score = _finite("evidence adapter selection min_score", min_score, 0, 1)
        minimum_margin = _finite("evidence adapter selection min_margin", min_margin, 0, 1)
        signals: dict[str, AutonomousEvidenceAdapterSelectionSignal] = {}
        if selection_signals is not None:
            if not isinstance(selection_signals, Mapping) or len(selection_signals) > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_CANDIDATES:
                raise ArgumentError("evidence adapter selection signals are malformed or exceed their bound")
            known = {manifest.adapter_id for manifest in self.registry.manifests()}
            for adapter_id, raw in selection_signals.items():
                normalized_id = _identifier("evidence adapter selection signal adapter_id", adapter_id)
                if normalized_id not in known:
                    raise ArgumentError(f"evidence adapter selection signal names an unknown adapter: {normalized_id}")
                if not isinstance(raw, Mapping):
                    raise ArgumentError("evidence adapter selection signal must be a mapping")
                _json_bytes(raw, "evidence adapter selection signal", MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SIGNAL_BYTES)
                signals[normalized_id] = _normalize_signal(normalized_id, raw)
        signal_digest = content_digest([signals[key].to_dict() for key in sorted(signals)]) if strategy == "weighted_evidence" else None
        rows: list[AutonomousEvidenceAdapterSelectionRow] = []
        for domain in requested:
            candidates = self.registry.candidates(domain, selected_capability)
            descriptors = [signals.get(item.adapter_id, _normalize_signal(item.adapter_id, None if strategy == "weighted_evidence" else {"eligible": True})) for item in candidates]
            eligible = tuple(descriptor.eligible if strategy == "weighted_evidence" else True for descriptor in descriptors)
            scores = tuple(descriptor.score if strategy == "weighted_evidence" else 0.0 for descriptor in descriptors)
            ranked = sorted((index for index, value in enumerate(eligible) if value), key=lambda index: (-scores[index], candidates[index].adapter_id))
            top = ranked[0] if ranked else None
            top_score = scores[top] if top is not None else 0.0
            second_score = scores[ranked[1]] if len(ranked) > 1 else 0.0
            margin = top_score - second_score if top is not None else 0.0
            reason = (
                "no_eligible_adapter" if top is None and candidates else
                "no_matching_adapter" if top is None else
                "selection_below_min_score" if top_score < minimum_score else
                "insufficient_selection_margin" if margin < minimum_margin else strategy
            )
            selected = candidates[top] if top is not None and reason == strategy else None
            rows.append(AutonomousEvidenceAdapterSelectionRow(domain, "selected" if selected else "missing", None if selected is None else selected.adapter_id, None if selected is None else selected.manifest_digest, tuple(item.adapter_id for item in candidates), tuple(item.manifest_digest for item in candidates), scores, eligible, reason))
        return AutonomousEvidenceAdapterSelectionPlan(requested, selected_capability, self.registry.registry_digest, tuple(rows), strategy, signal_digest)

    def select_adaptive_for_domains(self, domains: Sequence[str], selection_signals: Mapping[str, Mapping[str, Any]], **options: Any) -> AutonomousEvidenceAdapterSelectionPlan:
        return self.select_for_domains(domains, strategy="weighted_evidence", selection_signals=selection_signals, **options)

    def create_acquirer_from_selection(self, plan: AutonomousEvidenceAdapterSelectionPlan | Mapping[str, Any]) -> Any:
        typed = plan if isinstance(plan, AutonomousEvidenceAdapterSelectionPlan) else AutonomousEvidenceAdapterSelectionPlan.from_dict(plan)
        typed.verify(self.registry)
        routes: dict[str, str] = {}
        for row in typed.rows:
            if row.status != "selected" or row.adapter_id is None:
                raise ArgumentError(f"evidence adapter selection is incomplete for {row.domain}")
            routes[row.domain] = row.adapter_id
        return self.registry.create_acquirer(routes)


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceAdapterHealthObservation:
    adapter_id: str
    manifest_digest: str
    domain: str
    observation_kind: str
    outcome: str
    status: str
    latency_ms: float
    cost_units: float | None = None
    failure_class: str | None = None
    evaluator_reward: float | None = None
    evaluator_passed: bool | None = None
    evaluator_id: str | None = None
    evaluator_version: str | None = None
    evidence_digest: str | None = None

    def __post_init__(self) -> None:
        _identifier("adapter health adapter_id", self.adapter_id)
        _digest("adapter health manifest_digest", self.manifest_digest)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES or self.observation_kind not in _HEALTH_KINDS or self.outcome not in _HEALTH_OUTCOMES:
            raise ArgumentError("adapter health observation kind, outcome, or domain is invalid")
        _identifier("adapter health status", self.status)
        _finite("adapter health latency_ms", self.latency_ms, 0, 86_400_000)
        if self.cost_units is not None:
            _finite("adapter health cost_units", self.cost_units, 0, 1_000_000)
        if self.failure_class is not None:
            _identifier("adapter health failure_class", self.failure_class)
        if self.evaluator_reward is not None:
            _finite("adapter health evaluator_reward", self.evaluator_reward, -1, 1)
        if self.evaluator_passed is not None and not isinstance(self.evaluator_passed, bool):
            raise ArgumentError("adapter health evaluator_passed must be boolean or null")
        for name, value in (("evaluator_id", self.evaluator_id), ("evaluator_version", self.evaluator_version)):
            if value is not None:
                _identifier(f"adapter health {name}", value)
        _digest("adapter health evidence_digest", self.evidence_digest, allow_none=True)
        if self.observation_kind == "evaluation":
            if self.evaluator_reward is None or self.outcome != "unknown":
                raise ArgumentError("adapter health evaluation requires unknown outcome and evaluator reward")

    def to_dict(self) -> dict[str, Any]:
        return {"schema": AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_OBSERVATION_SCHEMA, "adapter_id": self.adapter_id, "manifest_digest": self.manifest_digest, "domain": self.domain, "observation_kind": self.observation_kind, "outcome": self.outcome, "status": self.status, "latency_ms": self.latency_ms, "cost_units": self.cost_units, "failure_class": self.failure_class, "evaluator_reward": self.evaluator_reward, "evaluator_passed": self.evaluator_passed, "evaluator_id": self.evaluator_id, "evaluator_version": self.evaluator_version, "evidence_digest": self.evidence_digest, "retention": _RETENTION, "secret_material": "never_returned"}


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceAdapterHealthEvent:
    sequence: int
    observation: AutonomousEvidenceAdapterHealthObservation
    previous_digest: str
    created_at: float
    event_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {"schema": AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA, "sequence": self.sequence, "observation": self.observation.to_dict(), "previous_digest": self.previous_digest, "created_at": self.created_at, "event_digest": self.event_digest, "retention": _RETENTION, "secret_material": "never_returned"}


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceAdapterHealthReceipt:
    sequence: int
    event_digest: str
    adapter_id: str
    manifest_digest: str
    domain: str
    observation_kind: str

    def to_dict(self) -> dict[str, Any]:
        return {"schema": AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_RECEIPT_SCHEMA, "sequence": self.sequence, "event_digest": self.event_digest, "adapter_id": self.adapter_id, "manifest_digest": self.manifest_digest, "domain": self.domain, "observation_kind": self.observation_kind, "retention": _RETENTION, "secret_material": "never_returned"}


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceAdapterHealthSnapshot:
    sequence: int
    head_digest: str
    events: tuple[AutonomousEvidenceAdapterHealthEvent, ...]
    snapshot_digest: str

    def _descriptor(self) -> dict[str, Any]:
        return {"schema": AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA, "sequence": self.sequence, "head_digest": self.head_digest, "events": [event.to_dict() for event in self.events], "retention": _RETENTION, "secret_material": "never_returned"}

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "snapshot_digest": self.snapshot_digest}


def _observation_from_dict(value: Mapping[str, Any]) -> AutonomousEvidenceAdapterHealthObservation:
    if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_OBSERVATION_SCHEMA or value.get("retention") != _RETENTION or value.get("secret_material") != "never_returned":
        raise ArgumentError("adapter health observation is malformed")
    _safe_metadata(value, "adapter health observation")
    if set(value) - _HEALTH_OBSERVATION_KEYS:
        raise ArgumentError("adapter health observation contains unsupported fields")
    return AutonomousEvidenceAdapterHealthObservation(value["adapter_id"], value["manifest_digest"], value["domain"], value["observation_kind"], value["outcome"], value["status"], value["latency_ms"], value.get("cost_units"), value.get("failure_class"), value.get("evaluator_reward"), value.get("evaluator_passed"), value.get("evaluator_id"), value.get("evaluator_version"), value.get("evidence_digest"))


def validate_autonomous_evidence_adapter_health_snapshot(value: Mapping[str, Any] | AutonomousEvidenceAdapterHealthSnapshot, max_events: int = MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS) -> AutonomousEvidenceAdapterHealthSnapshot:
    raw = value.to_dict() if isinstance(value, AutonomousEvidenceAdapterHealthSnapshot) else value
    if not isinstance(raw, Mapping) or raw.get("schema") != AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA or raw.get("retention") != _RETENTION or raw.get("secret_material") != "never_returned":
        raise ArgumentError("adapter health snapshot is malformed")
    _safe_metadata(raw, "adapter health snapshot")
    if set(raw) != _HEALTH_SNAPSHOT_KEYS:
        raise ArgumentError("adapter health snapshot contains unsupported fields")
    _integer("adapter health snapshot max_events", max_events, 1, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS)
    events_raw = raw.get("events")
    if isinstance(events_raw, (str, bytes, bytearray)) or not isinstance(events_raw, Sequence) or len(events_raw) > max_events:
        raise ArgumentError("adapter health snapshot events exceed their bound")
    sequence = _integer("adapter health snapshot sequence", raw.get("sequence"), 0, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS)
    if sequence != len(events_raw):
        raise ArgumentError("adapter health snapshot sequence is inconsistent")
    head_value = raw.get("head_digest")
    if len(events_raw) == 0:
        if head_value != "":
            raise ArgumentError("empty adapter health snapshot must have an empty head digest")
        head = ""
    else:
        head = _digest("adapter health snapshot head_digest", head_value)
    supplied = _digest("adapter health snapshot snapshot_digest", raw.get("snapshot_digest"))
    events: list[AutonomousEvidenceAdapterHealthEvent] = []
    previous = ""
    for index, raw_event in enumerate(events_raw):
        if not isinstance(raw_event, Mapping) or raw_event.get("schema") != AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA or raw_event.get("retention") != _RETENTION or raw_event.get("secret_material") != "never_returned":
            raise ArgumentError(f"adapter health event {index + 1} is malformed")
        if set(raw_event) != _HEALTH_EVENT_KEYS:
            raise ArgumentError(f"adapter health event {index + 1} contains unsupported fields")
        event_sequence = _integer(f"adapter health event {index + 1} sequence", raw_event.get("sequence"), index + 1, index + 1)
        if raw_event.get("previous_digest") != previous:
            raise ArgumentError(f"adapter health event chain is invalid at sequence {event_sequence}")
        observation = _observation_from_dict(raw_event.get("observation"))
        created_at = _finite(f"adapter health event {event_sequence} created_at", raw_event.get("created_at"), 0, float("1.7976931348623157e308"))
        event_digest = _digest(f"adapter health event {event_sequence} event_digest", raw_event.get("event_digest"))
        descriptor = {key: raw_event[key] for key in raw_event if key != "event_digest"}
        if content_digest(descriptor) != event_digest:
            raise ArgumentError(f"adapter health event digest is invalid at sequence {event_sequence}")
        events.append(AutonomousEvidenceAdapterHealthEvent(event_sequence, observation, previous, created_at, event_digest))
        previous = event_digest
    if head != previous:
        raise ArgumentError("adapter health snapshot head digest is inconsistent")
    descriptor = {key: raw[key] for key in raw if key != "snapshot_digest"}
    if content_digest(descriptor) != supplied:
        raise ArgumentError("adapter health snapshot digest is invalid")
    snapshot = AutonomousEvidenceAdapterHealthSnapshot(sequence, head, tuple(events), supplied)
    _json_bytes(snapshot.to_dict(), "adapter health snapshot", MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_BYTES)
    return _copy(snapshot)


class InMemoryAutonomousEvidenceAdapterHealthStore:
    def __init__(self, *, max_events: int = MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS, clock: Callable[[], float] | None = None) -> None:
        self.max_events = _integer("adapter health max_events", max_events, 1, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS)
        self._clock = clock or (lambda: time.time())
        if not callable(self._clock):
            raise ArgumentError("adapter health clock is malformed")
        self._events: list[AutonomousEvidenceAdapterHealthEvent] = []
        self._lock = threading.RLock()

    @property
    def events(self) -> tuple[AutonomousEvidenceAdapterHealthEvent, ...]:
        with self._lock:
            return _copy(tuple(self._events))

    def record(self, observation: AutonomousEvidenceAdapterHealthObservation | Mapping[str, Any]) -> AutonomousEvidenceAdapterHealthReceipt:
        if isinstance(observation, AutonomousEvidenceAdapterHealthObservation):
            normalized = observation
        elif isinstance(observation, Mapping):
            if set(observation) - _HEALTH_OBSERVATION_KEYS:
                raise ArgumentError("adapter health observation contains unsupported fields")
            normalized = AutonomousEvidenceAdapterHealthObservation(observation.get("adapter_id"), observation.get("manifest_digest"), observation.get("domain"), observation.get("observation_kind", "acquisition"), observation.get("outcome"), observation.get("status"), observation.get("latency_ms"), observation.get("cost_units"), observation.get("failure_class"), observation.get("evaluator_reward"), observation.get("evaluator_passed"), observation.get("evaluator_id"), observation.get("evaluator_version"), observation.get("evidence_digest"))
        else:
            normalized = None
        if not isinstance(normalized, AutonomousEvidenceAdapterHealthObservation):
            raise ArgumentError("adapter health observation is malformed")
        with self._lock:
            if len(self._events) >= self.max_events:
                raise ArgumentError("adapter health event capacity is exhausted")
            sequence = len(self._events) + 1
            previous = self._events[-1].event_digest if self._events else ""
            created_at = _finite("adapter health clock", self._clock(), 0, float("1.7976931348623157e308"))
            descriptor = {"schema": AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA, "sequence": sequence, "observation": normalized.to_dict(), "previous_digest": previous, "created_at": created_at, "retention": _RETENTION, "secret_material": "never_returned"}
            event = AutonomousEvidenceAdapterHealthEvent(sequence, normalized, previous, created_at, content_digest(descriptor))
            self._events.append(event)
            return AutonomousEvidenceAdapterHealthReceipt(sequence, event.event_digest, normalized.adapter_id, normalized.manifest_digest, normalized.domain, normalized.observation_kind)

    def record_acquisition(self, **input_value: Any) -> AutonomousEvidenceAdapterHealthReceipt:
        input_value.update({"schema": AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_OBSERVATION_SCHEMA, "observation_kind": "acquisition", "retention": _RETENTION, "secret_material": "never_returned"})
        if input_value.get("outcome") == "unknown":
            raise ArgumentError("adapter health acquisition outcome cannot be unknown")
        return self.record(input_value)

    def record_evaluation(self, **input_value: Any) -> AutonomousEvidenceAdapterHealthReceipt:
        input_value.update({"schema": AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_OBSERVATION_SCHEMA, "observation_kind": "evaluation", "outcome": "unknown", "latency_ms": 0, "retention": _RETENTION, "secret_material": "never_returned"})
        return self.record(input_value)

    def health(self, *, adapter_id: str | None = None, manifest_digest: str | None = None, domain: str | None = None, min_attempts: int = 3, failure_threshold: float = 0.75, limit: int = MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_QUERY_LIMIT) -> tuple[dict[str, Any], ...]:
        if adapter_id is not None:
            _identifier("adapter health query adapter_id", adapter_id)
        _digest("adapter health query manifest_digest", manifest_digest, allow_none=True)
        if domain is not None and domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("adapter health query domain is unsupported")
        minimum_attempts = _integer("adapter health min_attempts", min_attempts, 1, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS)
        threshold = _finite("adapter health failure_threshold", failure_threshold, 0, 1)
        query_limit = _integer("adapter health limit", limit, 1, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_QUERY_LIMIT)
        aggregates: dict[tuple[str, str, str], dict[str, Any]] = {}
        for event in self.events:
            observation = event.observation
            if adapter_id is not None and observation.adapter_id != adapter_id or manifest_digest is not None and observation.manifest_digest != manifest_digest or domain is not None and observation.domain != domain:
                continue
            key = (observation.adapter_id, observation.manifest_digest, observation.domain)
            entry = aggregates.setdefault(key, {"adapter_id": observation.adapter_id, "manifest_digest": observation.manifest_digest, "domain": observation.domain, "attempts": 0, "successes": 0, "failures": 0, "unknown": 0, "total_latency": 0.0, "total_cost": 0.0, "cost_observations": 0, "quality_observations": 0, "reward_total": 0.0, "quality_passed": 0, "consecutive_failures": 0, "last_status": None, "last_outcome": None, "last_sequence": 0})
            entry["last_status"], entry["last_outcome"], entry["last_sequence"] = observation.status, observation.outcome, event.sequence
            if observation.observation_kind == "acquisition":
                entry["attempts"] += 1
                entry["total_latency"] += float(observation.latency_ms)
                if observation.cost_units is not None:
                    entry["total_cost"] += float(observation.cost_units)
                    entry["cost_observations"] += 1
                if observation.outcome == "success":
                    entry["successes"] += 1
                    entry["consecutive_failures"] = 0
                elif observation.outcome == "failure":
                    entry["failures"] += 1
                    entry["consecutive_failures"] += 1
                else:
                    entry["unknown"] += 1
            elif observation.evaluator_reward is not None:
                entry["quality_observations"] += 1
                entry["reward_total"] += float(observation.evaluator_reward)
                entry["quality_passed"] += 1 if observation.evaluator_passed else 0
        result: list[dict[str, Any]] = []
        for entry in sorted(aggregates.values(), key=lambda item: (item["domain"], item["adapter_id"], item["manifest_digest"]))[:query_limit]:
            attempts = entry["attempts"]
            quality = entry["quality_observations"]
            result.append({"schema": AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SCHEMA, "adapter_id": entry["adapter_id"], "manifest_digest": entry["manifest_digest"], "domain": entry["domain"], "attempts": attempts, "successes": entry["successes"], "failures": entry["failures"], "unknown": entry["unknown"], "success_rate": 0 if attempts == 0 else entry["successes"] / attempts, "failure_rate": 0 if attempts == 0 else entry["failures"] / attempts, "mean_latency_ms": 0 if attempts == 0 else entry["total_latency"] / attempts, "mean_cost_units": None if entry["cost_observations"] == 0 else entry["total_cost"] / entry["cost_observations"], "quality_observations": quality, "evaluator_reward_mean": None if quality == 0 else entry["reward_total"] / quality, "evaluator_pass_rate": None if quality == 0 else entry["quality_passed"] / quality, "consecutive_failures": entry["consecutive_failures"], "last_status": entry["last_status"], "last_outcome": entry["last_outcome"], "last_sequence": entry["last_sequence"], "circuit": "open" if attempts >= minimum_attempts and entry["failures"] / attempts >= threshold else "closed", "retention": "aggregated_metadata_only", "secret_material": "never_returned"})
        return tuple(result)

    def selection_signals(self, *, domain: str | None = None, manifest_digests: Mapping[str, str] | None = None, min_attempts: int = 3, failure_threshold: float = 0.75) -> dict[str, dict[str, Any]]:
        rows = self.health(domain=domain, min_attempts=min_attempts, failure_threshold=failure_threshold)
        expected = {} if manifest_digests is None else dict(manifest_digests)
        signals: dict[str, dict[str, Any]] = {}
        for row in rows:
            if expected and expected.get(row["adapter_id"]) != row["manifest_digest"]:
                continue
            signals[row["adapter_id"]] = {"eligible": row["attempts"] > 0 and row["circuit"] == "closed", "health": row["success_rate"], "success_rate": row["success_rate"], "evaluator_reward": row["evaluator_reward_mean"] if row["evaluator_reward_mean"] is not None else 0, "latency_ms": row["mean_latency_ms"], "cost_units": row["mean_cost_units"]}
        return signals

    def snapshot(self) -> AutonomousEvidenceAdapterHealthSnapshot:
        with self._lock:
            events = tuple(_copy(self._events))
            head = events[-1].event_digest if events else ""
            descriptor = {"schema": AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA, "sequence": len(events), "head_digest": head, "events": [event.to_dict() for event in events], "retention": _RETENTION, "secret_material": "never_returned"}
            snapshot = AutonomousEvidenceAdapterHealthSnapshot(len(events), head, events, content_digest(descriptor))
            _json_bytes(snapshot.to_dict(), "adapter health snapshot", MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_BYTES)
            return snapshot

    def restore(self, snapshot: Mapping[str, Any] | AutonomousEvidenceAdapterHealthSnapshot) -> None:
        validated = validate_autonomous_evidence_adapter_health_snapshot(snapshot, self.max_events)
        with self._lock:
            self._events = list(validated.events)

    def verify_integrity(self) -> dict[str, Any]:
        snapshot = self.snapshot()
        validate_autonomous_evidence_adapter_health_snapshot(snapshot, self.max_events)
        return {"verified": True, "events": snapshot.sequence, "head_digest": snapshot.head_digest}


class AutonomousEvidenceAdapterHealthSnapshotTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class TransactionalAutonomousEvidenceAdapterHealthSnapshotTextStore(AutonomousEvidenceAdapterHealthSnapshotTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class JsonAutonomousEvidenceAdapterHealthPersistence:
    def __init__(self, store: AutonomousEvidenceAdapterHealthSnapshotTextStore, *, max_events: int = MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS, max_bytes: int = MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_BYTES) -> None:
        if not callable(getattr(store, "read", None)) or not callable(getattr(store, "write", None)):
            raise ArgumentError("adapter health JSON persistence requires a text store")
        self.store = store
        self.max_events = _integer("adapter health persistence max_events", max_events, 1, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS)
        self.max_bytes = _integer("adapter health persistence max_bytes", max_bytes, 1, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_BYTES)

    def read(self) -> AutonomousEvidenceAdapterHealthSnapshot | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("adapter health JSON persistence text exceeds its bound")
        try:
            value = json.loads(encoded)
        except (TypeError, ValueError) as error:
            raise ArgumentError("adapter health JSON persistence text is invalid JSON") from error
        if canonical_json(value) != encoded:
            raise ArgumentError("adapter health JSON persistence text is not canonical")
        return validate_autonomous_evidence_adapter_health_snapshot(value, self.max_events)

    def write(self, snapshot: Mapping[str, Any] | AutonomousEvidenceAdapterHealthSnapshot) -> None:
        validated = validate_autonomous_evidence_adapter_health_snapshot(snapshot, self.max_events)
        encoded = canonical_json(validated.to_dict())
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("adapter health JSON persistence snapshot exceeds its bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousEvidenceAdapterHealthPersistence(JsonAutonomousEvidenceAdapterHealthPersistence):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any] | AutonomousEvidenceAdapterHealthSnapshot) -> bool:
        _digest("adapter health expected snapshot digest", expected_snapshot_digest, allow_none=True)
        if not callable(getattr(self.store, "write_if_unchanged", None)):
            raise ArgumentError("adapter health store does not support compare-and-swap")
        validated = validate_autonomous_evidence_adapter_health_snapshot(snapshot, self.max_events)
        encoded = canonical_json(validated.to_dict())
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("adapter health JSON persistence snapshot exceeds its bound")
        result = self.store.write_if_unchanged(expected_snapshot_digest, encoded)  # type: ignore[attr-defined]
        if not isinstance(result, bool):
            raise ArgumentError("adapter health compare-and-swap returned a non-boolean")
        return result


class AutonomousEvidenceAdapterHealthPersistenceCoordinator:
    def __init__(self, store: InMemoryAutonomousEvidenceAdapterHealthStore, persistence: Any) -> None:
        if not isinstance(store, InMemoryAutonomousEvidenceAdapterHealthStore) or not callable(getattr(persistence, "read", None)) or not callable(getattr(persistence, "write", None)):
            raise ArgumentError("adapter health persistence coordinator is malformed")
        self.store = store
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None
        self._lock = threading.RLock()

    def restore(self) -> dict[str, Any]:
        with self._lock:
            snapshot = self.persistence.read()
            if snapshot is None:
                self._expected_snapshot_digest = None
            else:
                self.store.restore(snapshot)
                self._expected_snapshot_digest = snapshot.snapshot_digest
            return self.store.verify_integrity()

    def flush(self) -> AutonomousEvidenceAdapterHealthSnapshot:
        with self._lock:
            snapshot = self.store.snapshot()
            writer = getattr(self.persistence, "write_if_unchanged", None)
            if callable(writer):
                if not writer(self._expected_snapshot_digest, snapshot):
                    raise ArgumentError("adapter health persistence rejected a stale writer")
            else:
                self.persistence.write(snapshot)
            self._expected_snapshot_digest = snapshot.snapshot_digest
            return snapshot


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceFailoverPolicy:
    max_failovers: int = 0
    retry_policy: AutonomousEvidenceRetryPolicy = AutonomousEvidenceRetryPolicy()

    def __post_init__(self) -> None:
        _integer("evidence failover max_failovers", self.max_failovers, 0, MAX_AUTONOMOUS_EVIDENCE_FAILOVERS)
        if not isinstance(self.retry_policy, AutonomousEvidenceRetryPolicy):
            raise ArgumentError("evidence failover retry policy is malformed")

    def to_dict(self) -> dict[str, Any]:
        return {"schema": AUTONOMOUS_EVIDENCE_FAILOVER_POLICY_SCHEMA, "max_failovers": self.max_failovers, "retry_policy": self.retry_policy.to_dict(), "execution": "caller_controlled_reviewed_candidate_failover;no_fuzzy_selection", "retention": "metadata_only_candidate_identity_and_failure_class", "secret_material": "never_returned"}


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceFailoverEvent:
    domain: str
    candidate_id: str
    candidate_manifest_digest: str
    candidate_rank: int
    status: str
    failure_class: str | None
    retryable: bool
    failovers_used: int
    remaining_candidates: int

    def to_dict(self) -> dict[str, Any]:
        return {"schema": AUTONOMOUS_EVIDENCE_FAILOVER_EVENT_SCHEMA, "domain": self.domain, "candidate_id": self.candidate_id, "candidate_manifest_digest": self.candidate_manifest_digest, "candidate_rank": self.candidate_rank, "status": self.status, "failure_class": self.failure_class, "retryable": self.retryable, "failovers_used": self.failovers_used, "remaining_candidates": self.remaining_candidates, "retention": "metadata_only;candidate_identity_and_failure_class", "secret_material": "never_returned"}


class AutonomousEvidenceAdapterFailoverAcquirer:
    """Execute only the candidates in a verified selection plan under bounded failover."""

    def __init__(self, registry: AutonomousEvidenceAdapterRegistry, plan: AutonomousEvidenceAdapterSelectionPlan | Mapping[str, Any], *, policy: AutonomousEvidenceFailoverPolicy | None = None, provider_contracts: Any | None = None, source_boundary: Mapping[str, Any] | None = None, classify: Callable[[BaseException], AutonomousEvidenceRetryClassification | Mapping[str, Any]] | None = None, observe_failover: Callable[[AutonomousEvidenceFailoverEvent], Any] | None = None, observe_attempt: Callable[[AutonomousEvidenceRetryAttempt], Any] | None = None, clock: Callable[[], float] | None = None, sleep: Callable[[int], Any] | None = None) -> None:
        if not isinstance(registry, AutonomousEvidenceAdapterRegistry):
            raise ArgumentError("evidence failover requires a typed registry")
        typed_plan = plan if isinstance(plan, AutonomousEvidenceAdapterSelectionPlan) else AutonomousEvidenceAdapterSelectionPlan.from_dict(plan)
        typed_plan.verify(registry)
        if not typed_plan.complete:
            raise ArgumentError("evidence failover requires a complete selection plan")
        if policy is not None and not isinstance(policy, AutonomousEvidenceFailoverPolicy):
            raise ArgumentError("evidence failover policy is malformed")
        if provider_contracts is not None and not callable(getattr(provider_contracts, "create_acquirer_for_adapter", None)):
            raise ArgumentError("evidence failover provider contract registry is malformed")
        if source_boundary is not None:
            if not isinstance(source_boundary, Mapping):
                raise ArgumentError("evidence failover source boundary is malformed")
            if provider_contracts is None:
                raise ArgumentError("source-bound evidence failover requires provider contracts")
            if not callable(source_boundary.get("describe_source")):
                raise ArgumentError("source-bound evidence failover requires a policy and describe_source callback")
        for callback in (classify, observe_failover, observe_attempt, clock, sleep):
            if callback is not None and not callable(callback):
                raise ArgumentError("evidence failover callback is malformed")
        self.registry = registry
        self.plan = typed_plan
        self.policy = policy or AutonomousEvidenceFailoverPolicy()
        self.provider_contracts = provider_contracts
        self.source_boundary = dict(source_boundary) if source_boundary is not None else None
        self.classify = classify
        self.observe_failover = observe_failover
        self.observe_attempt = observe_attempt
        self.clock = clock or (lambda: time.monotonic() * 1000.0)
        self.sleep = sleep or (lambda delay_ms: time.sleep(delay_ms / 1000.0))

    def _candidate_order(self, row: AutonomousEvidenceAdapterSelectionRow) -> tuple[str, ...]:
        return tuple(item["adapter_id"] for item in sorted(({"adapter_id": adapter_id, "score": row.candidate_scores[index], "eligible": row.candidate_eligible[index]} for index, adapter_id in enumerate(row.candidate_ids) if row.candidate_eligible[index]), key=lambda item: (-item["score"], item["adapter_id"])))

    def _emit(self, event: AutonomousEvidenceFailoverEvent) -> None:
        if self.observe_failover is not None:
            self.observe_failover(event)

    def acquire(self, context: Mapping[str, Any]) -> Any:
        if not isinstance(context, Mapping):
            raise ArgumentError("evidence failover acquisition context is malformed")
        requirement = context.get("requirement")
        domain = requirement.get("domain") if isinstance(requirement, Mapping) else getattr(requirement, "domain", None)
        if domain not in self.plan.domains:
            raise ArgumentError("evidence failover context domain is outside the selection plan")
        self.plan.verify(self.registry)
        row = next(item for item in self.plan.rows if item.domain == domain)
        candidates = self._candidate_order(row)
        if not candidates:
            raise ArgumentError(f"evidence failover has no eligible candidates for {domain}")
        failovers_used = 0
        last_error: BaseException | None = None
        for index, adapter_id in enumerate(candidates):
            if index > self.policy.max_failovers:
                break
            manifest = self.registry.resolve(domain, adapter_id)
            if index > 0:
                self._emit(AutonomousEvidenceFailoverEvent(domain, adapter_id, manifest.manifest_digest, index, "fallback_started", None, True, failovers_used, len(candidates) - index))
            base = self.registry.create_acquirer({domain: adapter_id})
            if self.provider_contracts is not None:
                if self.source_boundary is None:
                    base = self.provider_contracts.create_acquirer_for_adapter(adapter_id, domain)
                else:
                    from .autonomous_evidence_source import create_autonomous_evidence_source_acquirer

                    contract = self.provider_contracts.contract_for_adapter(adapter_id, domain)
                    source_kind = self.source_boundary.get("source_kind")
                    if source_kind is None:
                        if len(contract.source_kinds) != 1:
                            raise ArgumentError(f"source-bound evidence failover requires source_kind for {contract.contract_id}")
                        source_kind = contract.source_kinds[0]
                    base = create_autonomous_evidence_source_acquirer(
                        self.provider_contracts,
                        adapter_id=adapter_id,
                        domain=domain,
                        source_kind=source_kind,
                        policy=self.source_boundary["policy"],
                        ledger=self.source_boundary.get("ledger"),
                        describe_source=self.source_boundary["describe_source"],
                    )
            retrying = create_autonomous_evidence_retrying_acquirer(base, policy=self.policy.retry_policy, classify=self.classify, observe=self.observe_attempt, clock=self.clock, sleep=self.sleep)
            try:
                value = retrying.acquire(context)
                self._emit(AutonomousEvidenceFailoverEvent(domain, adapter_id, manifest.manifest_digest, index, "candidate_succeeded", None, False, failovers_used, len(candidates) - index - 1))
                return value
            except Exception as error:
                last_error = error
                classification = self.classify(error) if self.classify is not None else classify_autonomous_evidence_acquisition_error(error)
                if isinstance(classification, Mapping):
                    classification = AutonomousEvidenceRetryClassification(classification.get("failure_class", classification.get("failureClass")), classification.get("retryable"))
                if not isinstance(classification, AutonomousEvidenceRetryClassification):
                    raise ArgumentError("evidence failover classifier returned malformed metadata")
                remaining = len(candidates) - index - 1
                self._emit(AutonomousEvidenceFailoverEvent(domain, adapter_id, manifest.manifest_digest, index, "candidate_failed", classification.failure_class, classification.retryable, failovers_used, remaining))
                if not self.policy.retry_policy.permits(classification) or remaining == 0 or failovers_used >= self.policy.max_failovers:
                    self._emit(AutonomousEvidenceFailoverEvent(domain, adapter_id, manifest.manifest_digest, index, "failover_exhausted", classification.failure_class, classification.retryable, failovers_used, remaining))
                    raise
                failovers_used += 1
        if last_error is not None:
            raise last_error
        raise ArgumentError("evidence failover exhausted unexpectedly")


def create_autonomous_evidence_adapter_failover_acquirer(registry: AutonomousEvidenceAdapterRegistry, plan: AutonomousEvidenceAdapterSelectionPlan | Mapping[str, Any], **options: Any) -> AutonomousEvidenceAdapterFailoverAcquirer:
    return AutonomousEvidenceAdapterFailoverAcquirer(registry, plan, **options)


class AutonomousEvidenceAdapterHealthController:
    """Join generic adapter selection to metadata-only acquisition/evaluator observations."""

    def __init__(self, store: InMemoryAutonomousEvidenceAdapterHealthStore, registry: AutonomousEvidenceAdapterRegistry) -> None:
        if not isinstance(store, InMemoryAutonomousEvidenceAdapterHealthStore) or not isinstance(registry, AutonomousEvidenceAdapterRegistry):
            raise ArgumentError("adapter health controller requires typed store and registry")
        self.store = store
        self.registry = registry
        self.selector = AutonomousEvidenceAdapterSelector(registry)

    def select_adaptive_for_domains(self, domains: Sequence[str], *, capability: str | None = None, min_score: float = 0, min_margin: float = 0, min_attempts: int = 3, failure_threshold: float = 0.75) -> AutonomousEvidenceAdapterSelectionPlan:
        manifests = {manifest.adapter_id: manifest.manifest_digest for manifest in self.registry.manifests()}
        rows: list[AutonomousEvidenceAdapterSelectionRow] = []
        signal_evidence: list[dict[str, Any]] = []
        for domain in domains:
            signals = self.store.selection_signals(domain=domain, manifest_digests=manifests, min_attempts=min_attempts, failure_threshold=failure_threshold)
            partial = self.selector.select_adaptive_for_domains((domain,), signals, capability=capability, min_score=min_score, min_margin=min_margin)
            rows.append(partial.rows[0])
            signal_evidence.append({"domain": domain, "signals": signals})
        return AutonomousEvidenceAdapterSelectionPlan(tuple(domains), capability, self.registry.registry_digest, tuple(rows), "weighted_evidence", content_digest(signal_evidence))

    def create_observed_acquirer_from_selection(self, plan: AutonomousEvidenceAdapterSelectionPlan | Mapping[str, Any], *, clock: Callable[[], float] | None = None, cost_units_by_adapter: Mapping[str, float] | None = None) -> Any:
        typed = plan if isinstance(plan, AutonomousEvidenceAdapterSelectionPlan) else AutonomousEvidenceAdapterSelectionPlan.from_dict(plan)
        typed.verify(self.registry)
        routes = {row.domain: row.adapter_id for row in typed.rows if row.status == "selected" and row.adapter_id is not None}
        if len(routes) != len(typed.rows):
            raise ArgumentError("adapter health selection is incomplete")
        return self.create_observed_acquirer(self.selector.create_acquirer_from_selection(typed), routes, clock=clock, cost_units_by_adapter=cost_units_by_adapter)

    def create_observed_acquirer(self, acquirer: Any, adapter_id_for_domain: Mapping[str, str], *, clock: Callable[[], float] | None = None, cost_units_by_adapter: Mapping[str, float] | None = None) -> Any:
        if not callable(getattr(acquirer, "acquire", None)) or not isinstance(adapter_id_for_domain, Mapping):
            raise ArgumentError("observed evidence adapter acquirer is malformed")
        costs = {} if cost_units_by_adapter is None else dict(cost_units_by_adapter)
        for adapter_id, cost in costs.items():
            _identifier("adapter health cost adapter_id", adapter_id)
            _finite("adapter health cost_units_by_adapter", cost, 0, 1_000_000)
        clock_fn = clock or (lambda: time.monotonic() * 1000.0)
        if not callable(clock_fn):
            raise ArgumentError("adapter health acquisition clock is malformed")
        controller = self

        class Observed:
            def acquire(self, context: Mapping[str, Any]) -> Any:
                if not isinstance(context, Mapping):
                    raise ArgumentError("adapter health acquisition context is malformed")
                requirement = context.get("requirement")
                domain = requirement.get("domain") if isinstance(requirement, Mapping) else getattr(requirement, "domain", None)
                adapter_id = adapter_id_for_domain.get(domain)
                if not isinstance(adapter_id, str):
                    raise ArgumentError(f"adapter health route is missing for {domain}")
                manifest = controller.registry.resolve(domain, adapter_id)
                started = _finite("adapter health acquisition clock", clock_fn(), 0, float("1.7976931348623157e308"))
                try:
                    value = acquirer.acquire(context)
                    finished = _finite("adapter health acquisition clock", clock_fn(), 0, float("1.7976931348623157e308"))
                    controller.store.record_acquisition(adapter_id=manifest.adapter_id, manifest_digest=manifest.manifest_digest, domain=domain, outcome="success", status="success", latency_ms=max(0, finished - started), cost_units=costs.get(adapter_id), evidence_digest=(context.get("request") or {}).get("source_digest") if isinstance(context.get("request"), Mapping) else None)
                    return value
                except Exception as error:
                    finished = _finite("adapter health acquisition clock", clock_fn(), 0, float("1.7976931348623157e308"))
                    controller.store.record_acquisition(adapter_id=manifest.adapter_id, manifest_digest=manifest.manifest_digest, domain=domain, outcome="failure", status="failure", latency_ms=max(0, finished - started), cost_units=costs.get(adapter_id), failure_class=error.__class__.__name__)
                    raise

        return Observed()

    def create_observed_evaluator(self, evaluator: Any, adapter_id_for_domain: Mapping[str, str]) -> Any:
        if not callable(getattr(evaluator, "evaluate", None)) or not isinstance(adapter_id_for_domain, Mapping):
            raise ArgumentError("observed evidence evaluator is malformed")
        evaluator_id_value = _identifier("adapter health evaluator_id", getattr(evaluator, "evaluator_id", "evaluator"))
        evaluator_version_value = _identifier("adapter health evaluator_version", getattr(evaluator, "evaluator_version", "1"))
        controller = self

        class ObservedEvaluator:
            evaluator_id = evaluator_id_value
            evaluator_version = evaluator_version_value

            def evaluate(self, input_value: Mapping[str, Any]) -> Any:
                requirement = input_value.get("requirement") if isinstance(input_value, Mapping) else None
                domain = requirement.get("domain") if isinstance(requirement, Mapping) else getattr(requirement, "domain", None)
                adapter_id = adapter_id_for_domain.get(domain)
                if not isinstance(adapter_id, str):
                    raise ArgumentError(f"adapter health route is missing for {domain}")
                manifest = controller.registry.resolve(domain, adapter_id)
                try:
                    decision = evaluator.evaluate(input_value)
                    if not isinstance(decision, Mapping):
                        raise ArgumentError("evidence evaluator decision must be a mapping")
                    score = _finite("adapter health evaluator score", decision.get("score"), 0, 1)
                    verdict = _identifier("adapter health evaluator verdict", decision.get("verdict"))
                    controller.store.record_evaluation(adapter_id=manifest.adapter_id, manifest_digest=manifest.manifest_digest, domain=domain, status=f"verdict_{verdict}", evaluator_reward=score * 2 - 1, evaluator_passed=verdict == "accepted", evaluator_id=evaluator_id_value, evaluator_version=evaluator_version_value, evidence_digest=decision.get("evidence_digest"))
                    return decision
                except Exception:
                    controller.store.record_evaluation(adapter_id=manifest.adapter_id, manifest_digest=manifest.manifest_digest, domain=domain, status="evaluation_failed", evaluator_reward=-1, evaluator_passed=False, evaluator_id=evaluator_id_value, evaluator_version=evaluator_version_value)
                    raise

        return ObservedEvaluator()


__all__ = [
    "AUTONOMOUS_EVIDENCE_ADAPTER_REGISTRY_SCHEMA", "AUTONOMOUS_EVIDENCE_ADAPTER_MANIFEST_SCHEMA", "AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SCHEMA", "AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_ROW_SCHEMA", "AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SCHEMA", "AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_OBSERVATION_SCHEMA", "AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA", "AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_RECEIPT_SCHEMA", "AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA", "AUTONOMOUS_EVIDENCE_FAILOVER_POLICY_SCHEMA", "AUTONOMOUS_EVIDENCE_FAILOVER_EVENT_SCHEMA",
    "MAX_AUTONOMOUS_EVIDENCE_ADAPTERS", "MAX_AUTONOMOUS_EVIDENCE_ADAPTER_DOMAINS", "MAX_AUTONOMOUS_EVIDENCE_ADAPTER_CAPABILITIES", "MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SOURCE_KINDS", "MAX_AUTONOMOUS_EVIDENCE_ADAPTER_REGISTRY_BYTES", "MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_CANDIDATES", "MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SIGNAL_BYTES", "MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS", "MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_QUERY_LIMIT", "MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_BYTES", "MAX_AUTONOMOUS_EVIDENCE_FAILOVERS", "AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_STRATEGIES",
    "AutonomousEvidenceAdapterManifest", "AutonomousEvidenceAdapterCoverage", "AutonomousEvidenceAdapterRegistration", "AutonomousEvidenceAdapterRegistry", "register_autonomous_evidence_adapters_for_all_domains", "AutonomousEvidenceAdapterSelectionSignal", "AutonomousEvidenceAdapterSelectionRow", "AutonomousEvidenceAdapterSelectionPlan", "AutonomousEvidenceAdapterSelector", "AutonomousEvidenceAdapterHealthObservation", "AutonomousEvidenceAdapterHealthEvent", "AutonomousEvidenceAdapterHealthReceipt", "AutonomousEvidenceAdapterHealthSnapshot", "validate_autonomous_evidence_adapter_health_snapshot", "InMemoryAutonomousEvidenceAdapterHealthStore", "AutonomousEvidenceAdapterHealthSnapshotTextStore", "TransactionalAutonomousEvidenceAdapterHealthSnapshotTextStore", "JsonAutonomousEvidenceAdapterHealthPersistence", "TransactionalJsonAutonomousEvidenceAdapterHealthPersistence", "AutonomousEvidenceAdapterHealthPersistenceCoordinator", "AutonomousEvidenceFailoverPolicy", "AutonomousEvidenceFailoverEvent", "AutonomousEvidenceAdapterFailoverAcquirer", "create_autonomous_evidence_adapter_failover_acquirer", "AutonomousEvidenceAdapterHealthController",
]
