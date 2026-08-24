"""Adaptive registry, health, selection, and failover for LLM evidence adapters.

``autonomous_evidence_llm_adapter`` provides one provider-backed source.  This module turns a
collection of those sources into an explicit autonomous decision surface:

* the registry freezes adapter manifests and exposes a digest-bound candidate set;
* the health ledger stores only bounded acquisition/evaluator observations and produces selection
  signals without retaining prompts, responses, credentials, or evidence values;
* the selector emits a reviewable per-domain plan and verifies it before dispatch; and
* the failover acquirer retries only classified transient provider failures and only within an
  explicit fallback budget.

The result is deliberately not a fuzzy "try anything" router.  Every candidate, score, eligibility
decision, manifest digest, failure class, and fallback transition is inspectable.  A changed
registry or a stale selection plan fails closed before a provider call.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import threading
import time
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence_llm_adapter import (
    AutonomousLLMEvidenceAdapter,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError
from .llm_runtime import CredentialError, ProviderError


AUTONOMOUS_LLM_EVIDENCE_ADAPTER_REGISTRY_SCHEMA = "bioprism-python-autonomous-llm-evidence-adapter-registry/0.1"
AUTONOMOUS_LLM_EVIDENCE_ADAPTER_MANIFEST_SCHEMA = "bioprism-python-autonomous-llm-evidence-adapter-manifest/0.1"
AUTONOMOUS_LLM_EVIDENCE_ADAPTER_SELECTION_SCHEMA = "bioprism-python-autonomous-llm-evidence-adapter-selection/0.1"
AUTONOMOUS_LLM_EVIDENCE_ADAPTER_SELECTION_ROW_SCHEMA = "bioprism-python-autonomous-llm-evidence-adapter-selection-row/0.1"
AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_SCHEMA = "bioprism-python-autonomous-llm-evidence-adapter-health/0.1"
AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_OBSERVATION_SCHEMA = "bioprism-python-autonomous-llm-evidence-adapter-health-observation/0.1"
AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA = "bioprism-python-autonomous-llm-evidence-adapter-health-event/0.1"
AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-llm-evidence-adapter-health-snapshot/0.1"
AUTONOMOUS_LLM_EVIDENCE_FAILOVER_POLICY_SCHEMA = "bioprism-python-autonomous-llm-evidence-failover-policy/0.1"
AUTONOMOUS_LLM_EVIDENCE_FAILOVER_EVENT_SCHEMA = "bioprism-python-autonomous-llm-evidence-failover-event/0.1"
MAX_AUTONOMOUS_LLM_EVIDENCE_ADAPTERS = 256
MAX_AUTONOMOUS_LLM_EVIDENCE_SELECTION_CANDIDATES = 256
MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_EVENTS = 16_384
MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_SNAPSHOT_BYTES = 512_000
MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_QUERY_LIMIT = 512
MAX_AUTONOMOUS_LLM_EVIDENCE_FAILOVERS = 7

_SELECTION_STRATEGIES = frozenset({"lexicographic_adapter_id", "weighted_evidence"})
_HEALTH_OUTCOMES = frozenset({"success", "failure", "unknown"})
_HEALTH_KINDS = frozenset({"acquisition", "evaluation"})
_RETENTION = "metadata_only;raw_evidence_credentials_prompts_and_provider_payloads_never_persisted"


def _text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value.strip()


def _identifier(name: str, value: Any, maximum: int = 256) -> str:
    result = _text(name, value, maximum)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+- /" for character in result):
        raise ArgumentError(f"{name} contains unsupported identifier characters")
    return result


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _optional_digest(name: str, value: Any) -> str | None:
    return None if value is None else _digest(name, value)


def _finite(name: str, value: Any, minimum: float, maximum: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ArgumentError(f"{name} must be a finite number between {minimum} and {maximum}")
    result = float(value)
    if result != result or result in {float("inf"), float("-inf")} or not minimum <= result <= maximum:
        raise ArgumentError(f"{name} must be a finite number between {minimum} and {maximum}")
    return result


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise ArgumentError(f"{name} must be an integer between {minimum} and {maximum}")
    return value


def _domains(value: Any) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or not 1 <= len(value) <= len(AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError(f"LLM evidence domains must contain 1..{len(AUTONOMOUS_DOMAIN_NAMES)} entries")
    normalized = tuple(_identifier("LLM evidence domain", item) for item in value)
    if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in normalized):
        raise ArgumentError("LLM evidence domains contain an unsupported domain")
    if len(set(normalized)) != len(normalized):
        raise ArgumentError("LLM evidence domains must not contain duplicates")
    return normalized


def _safe_metadata(value: Any, name: str, *, depth: int = 0) -> None:
    if depth > 16:
        raise ArgumentError(f"{name} is too deeply nested")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip() or "\x00" in key:
                raise ArgumentError(f"{name} contains an invalid key")
            normalized = "".join(character for character in key.lower() if character.isalnum())
            if normalized in {
                "apikey", "authorization", "bearer", "credential", "credentials", "password",
                "secret", "token", "privatekey", "prompt", "messages", "response", "raw",
                "rawvalue", "payload", "arguments", "output", "task", "content", "body",
                "headers", "input",
            }:
                raise ArgumentError(f"{name} contains transient or secret-shaped fields")
            _safe_metadata(child, f"{name}.{key}", depth=depth + 1)
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        if len(value) > 512:
            raise ArgumentError(f"{name} contains too many entries")
        for index, child in enumerate(value):
            _safe_metadata(child, f"{name}[{index}]", depth=depth + 1)
    elif isinstance(value, float) and (value != value or value in {float("inf"), float("-inf")}):
        raise ArgumentError(f"{name} contains a non-finite number")


def _json_bytes(value: Any, name: str, maximum: int) -> None:
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be canonical JSON") from error
    if len(encoded) > maximum:
        raise ArgumentError(f"{name} exceeds its bounded size")


@dataclass(frozen=True, slots=True)
class AutonomousLLMEvidenceAdapterManifest:
    """Frozen metadata used to bind a selection plan to an adapter implementation."""

    adapter_id: str
    version: str
    domain: str
    provider: str
    capabilities: tuple[str, ...]
    source_kinds: tuple[str, ...]
    manifest_digest: str

    @classmethod
    def from_adapter(cls, adapter: AutonomousLLMEvidenceAdapter) -> "AutonomousLLMEvidenceAdapterManifest":
        if not isinstance(adapter, AutonomousLLMEvidenceAdapter):
            raise ArgumentError("LLM evidence adapter manifest requires a typed adapter")
        payload = {
            "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_MANIFEST_SCHEMA,
            "adapter_id": adapter.adapter_id,
            "version": adapter.version,
            "domain": adapter.domain,
            "provider": adapter.provider,
            "capabilities": list(adapter.capabilities),
            "source_kinds": list(adapter.source_kinds),
            "configuration": adapter.to_dict(),
            "retention": "metadata_only_adapter_identity_and_configuration",
            "secret_material": "never_returned",
        }
        return cls(
            adapter_id=adapter.adapter_id,
            version=adapter.version,
            domain=adapter.domain,
            provider=adapter.provider,
            capabilities=adapter.capabilities,
            source_kinds=adapter.source_kinds,
            manifest_digest=content_digest(payload),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_MANIFEST_SCHEMA,
            "adapter_id": self.adapter_id,
            "version": self.version,
            "domain": self.domain,
            "provider": self.provider,
            "capabilities": list(self.capabilities),
            "source_kinds": list(self.source_kinds),
            "manifest_digest": self.manifest_digest,
            "retention": "metadata_only_adapter_identity_and_configuration",
            "secret_material": "never_returned",
        }


class AutonomousLLMEvidenceAdapterRegistry:
    """Explicit adapter registry whose digest is the root of every selection plan."""

    def __init__(self, adapters: Sequence[AutonomousLLMEvidenceAdapter] = ()) -> None:
        if isinstance(adapters, (str, bytes)) or not isinstance(adapters, Sequence):
            raise ArgumentError("LLM evidence adapter registry adapters must be a sequence")
        if len(adapters) > MAX_AUTONOMOUS_LLM_EVIDENCE_ADAPTERS:
            raise ArgumentError("LLM evidence adapter registry exceeds its adapter bound")
        self._adapters: dict[tuple[str, str], AutonomousLLMEvidenceAdapter] = {}
        for adapter in adapters:
            self.register(adapter)

    def register(self, adapter: AutonomousLLMEvidenceAdapter, *, replace: bool = False) -> AutonomousLLMEvidenceAdapterManifest:
        if not isinstance(adapter, AutonomousLLMEvidenceAdapter):
            raise ArgumentError("LLM evidence adapter registry accepts only typed adapters")
        if not isinstance(replace, bool):
            raise ArgumentError("LLM evidence adapter registry replace must be boolean")
        key = (adapter.domain, adapter.adapter_id)
        conflicting_domain = next(
            (registered.domain for (registered_domain, registered_id), registered in self._adapters.items()
             if registered_id == adapter.adapter_id and registered_domain != adapter.domain),
            None,
        )
        if conflicting_domain is not None:
            raise ArgumentError(
                f"LLM evidence adapter id is already scoped to another domain: {adapter.adapter_id}/{conflicting_domain}"
            )
        if key in self._adapters and not replace:
            raise ArgumentError(f"LLM evidence adapter is already registered: {adapter.domain}/{adapter.adapter_id}")
        if key not in self._adapters and len(self._adapters) >= MAX_AUTONOMOUS_LLM_EVIDENCE_ADAPTERS:
            raise ArgumentError("LLM evidence adapter registry capacity is exhausted")
        self._adapters[key] = adapter
        return self.manifest_for(adapter.domain, adapter.adapter_id)

    def resolve(self, domain: str, adapter_id: str) -> AutonomousLLMEvidenceAdapter:
        key = (_identifier("LLM evidence adapter domain", domain), _identifier("LLM evidence adapter id", adapter_id))
        adapter = self._adapters.get(key)
        if adapter is None:
            raise ArgumentError(f"LLM evidence adapter is unavailable: {key[0]}/{key[1]}")
        return adapter

    def manifest_for(self, domain: str, adapter_id: str) -> AutonomousLLMEvidenceAdapterManifest:
        return AutonomousLLMEvidenceAdapterManifest.from_adapter(self.resolve(domain, adapter_id))

    def manifests(self) -> tuple[AutonomousLLMEvidenceAdapterManifest, ...]:
        return tuple(
            AutonomousLLMEvidenceAdapterManifest.from_adapter(self._adapters[key])
            for key in sorted(self._adapters)
        )

    @property
    def registry_digest(self) -> str:
        return content_digest(
            {
                "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_REGISTRY_SCHEMA,
                "manifests": [manifest.to_dict() for manifest in self.manifests()],
                "retention": "metadata_only_adapter_manifests",
                "secret_material": "never_returned",
            }
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_REGISTRY_SCHEMA,
            "manifests": [manifest.to_dict() for manifest in self.manifests()],
            "registry_digest": self.registry_digest,
            "retention": "metadata_only_adapter_manifests",
            "secret_material": "never_returned",
        }

    def candidates(self, domain: str, capability: str | None = None) -> tuple[AutonomousLLMEvidenceAdapterManifest, ...]:
        normalized_domain = _identifier("LLM evidence candidate domain", domain)
        normalized_capability = None if capability is None else _identifier("LLM evidence candidate capability", capability)
        candidates = tuple(
            manifest
            for manifest in self.manifests()
            if manifest.domain == normalized_domain
            and (normalized_capability is None or normalized_capability in manifest.capabilities)
        )
        if len(candidates) > MAX_AUTONOMOUS_LLM_EVIDENCE_SELECTION_CANDIDATES:
            raise ArgumentError("LLM evidence candidate set exceeds its bound")
        return candidates

    def verify_selection(self, plan: "AutonomousLLMEvidenceAdapterSelectionPlan") -> None:
        if not isinstance(plan, AutonomousLLMEvidenceAdapterSelectionPlan):
            raise ArgumentError("LLM evidence selection plan is malformed")
        if plan.registry_digest != self.registry_digest:
            raise ArgumentError("LLM evidence selection plan registry digest is stale")
        for row in plan.rows:
            current = self.candidates(row.domain, plan.capability)
            current_ids = tuple(manifest.adapter_id for manifest in current)
            current_digests = tuple(manifest.manifest_digest for manifest in current)
            if current_ids != row.candidate_ids or current_digests != row.candidate_manifest_digests:
                raise ArgumentError("LLM evidence selection plan candidate manifests changed")
            if row.status == "selected":
                if row.adapter_id is None or row.manifest_digest is None:
                    raise ArgumentError("selected LLM evidence row is incomplete")
                selected = self.manifest_for(row.domain, row.adapter_id)
                if selected.manifest_digest != row.manifest_digest:
                    raise ArgumentError("LLM evidence selected adapter manifest changed")


@dataclass(frozen=True, slots=True)
class AutonomousLLMEvidenceAdapterSelectionRow:
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
            raise ArgumentError("LLM evidence selection row domain or status is invalid")
        if self.adapter_id is not None:
            _identifier("LLM evidence selection row adapter_id", self.adapter_id)
        _optional_digest("LLM evidence selection row manifest_digest", self.manifest_digest)
        if len(self.candidate_ids) != len(self.candidate_manifest_digests) or len(self.candidate_ids) != len(self.candidate_scores) or len(self.candidate_ids) != len(self.candidate_eligible):
            raise ArgumentError("LLM evidence selection row candidate arrays must align")
        if len(self.candidate_ids) > MAX_AUTONOMOUS_LLM_EVIDENCE_SELECTION_CANDIDATES:
            raise ArgumentError("LLM evidence selection row exceeds its candidate bound")
        if len(set(self.candidate_ids)) != len(self.candidate_ids):
            raise ArgumentError("LLM evidence selection row contains duplicate candidates")
        for index, candidate_id in enumerate(self.candidate_ids):
            _identifier(f"LLM evidence selection row candidate {index}", candidate_id)
            _digest(f"LLM evidence selection row candidate digest {index}", self.candidate_manifest_digests[index])
            _finite(f"LLM evidence selection row candidate score {index}", self.candidate_scores[index], 0, 1)
            if not isinstance(self.candidate_eligible[index], bool):
                raise ArgumentError("LLM evidence selection row candidate eligibility must be boolean")
        if self.status == "selected" and self.adapter_id not in self.candidate_ids:
            raise ArgumentError("selected LLM evidence row adapter must be a candidate")
        _identifier("LLM evidence selection row reason", self.reason)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_SELECTION_ROW_SCHEMA,
            "domain": self.domain,
            "status": self.status,
            "adapter_id": self.adapter_id,
            "manifest_digest": self.manifest_digest,
            "candidate_ids": list(self.candidate_ids),
            "candidate_manifest_digests": list(self.candidate_manifest_digests),
            "candidate_scores": list(self.candidate_scores),
            "candidate_eligible": list(self.candidate_eligible),
            "reason": self.reason,
            "retention": "metadata_only_manifest_and_health_evidence",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousLLMEvidenceAdapterSelectionPlan:
    domains: tuple[str, ...]
    capability: str | None
    registry_digest: str
    rows: tuple[AutonomousLLMEvidenceAdapterSelectionRow, ...]
    strategy: str
    signal_digest: str | None = None

    def __post_init__(self) -> None:
        domains = _domains(self.domains)
        if tuple(row.domain for row in self.rows) != domains:
            raise ArgumentError("LLM evidence selection rows must align with domains")
        if self.capability is not None:
            _identifier("LLM evidence selection capability", self.capability)
        _digest("LLM evidence selection registry_digest", self.registry_digest)
        if self.strategy not in _SELECTION_STRATEGIES:
            raise ArgumentError("LLM evidence selection strategy is invalid")
        _optional_digest("LLM evidence selection signal_digest", self.signal_digest)
        object.__setattr__(self, "domains", domains)

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_SELECTION_SCHEMA,
            "domains": list(self.domains),
            "capability": self.capability,
            "registry_digest": self.registry_digest,
            "rows": [row.to_dict() for row in self.rows],
            "strategy": self.strategy,
            "signal_digest": self.signal_digest,
        }

    @property
    def plan_digest(self) -> str:
        return content_digest(self._payload())

    @property
    def complete(self) -> bool:
        return all(row.status == "selected" for row in self.rows)

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "plan_digest": self.plan_digest,
            "complete": self.complete,
            "retention": "metadata_only_manifest_and_health_evidence",
            "execution": "selection_only;source_dispatch_and_provider_invocation_remain_separate_approvals",
            "secret_material": "never_returned",
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousLLMEvidenceAdapterSelectionPlan":
        if not isinstance(value, Mapping):
            raise ArgumentError("LLM evidence selection plan must be a mapping")
        allowed = {"schema", "domains", "capability", "registry_digest", "rows", "strategy", "signal_digest", "plan_digest", "complete", "retention", "execution", "secret_material"}
        if set(value) != allowed or value.get("schema") != AUTONOMOUS_LLM_EVIDENCE_ADAPTER_SELECTION_SCHEMA:
            raise ArgumentError("LLM evidence selection plan contains unsupported or missing fields")
        raw_rows = value.get("rows")
        if not isinstance(raw_rows, Sequence) or isinstance(raw_rows, (str, bytes)):
            raise ArgumentError("LLM evidence selection plan rows must be a sequence")
        rows = tuple(
            AutonomousLLMEvidenceAdapterSelectionRow(
                domain=_identifier("LLM evidence selection row domain", raw.get("domain")),
                status=_text("LLM evidence selection row status", raw.get("status"), 32),
                adapter_id=None if raw.get("adapter_id") is None else _identifier("LLM evidence selection row adapter_id", raw.get("adapter_id")),
                manifest_digest=_optional_digest("LLM evidence selection row manifest_digest", raw.get("manifest_digest")),
                candidate_ids=tuple(_identifier("LLM evidence selection candidate id", item) for item in raw.get("candidate_ids", ())),
                candidate_manifest_digests=tuple(_digest("LLM evidence selection candidate digest", item) for item in raw.get("candidate_manifest_digests", ())),
                candidate_scores=tuple(_finite("LLM evidence selection candidate score", item, 0, 1) for item in raw.get("candidate_scores", ())),
                candidate_eligible=tuple(item for item in raw.get("candidate_eligible", ())),
                reason=_identifier("LLM evidence selection row reason", raw.get("reason")),
            )
            for raw in raw_rows
            if isinstance(raw, Mapping)
        )
        if len(rows) != len(raw_rows):
            raise ArgumentError("LLM evidence selection plan contains a malformed row")
        plan = cls(
            domains=tuple(value.get("domains", ())),
            capability=None if value.get("capability") is None else _identifier("LLM evidence selection capability", value.get("capability")),
            registry_digest=_digest("LLM evidence selection registry_digest", value.get("registry_digest")),
            rows=rows,
            strategy=_text("LLM evidence selection strategy", value.get("strategy"), 64),
            signal_digest=_optional_digest("LLM evidence selection signal_digest", value.get("signal_digest")),
        )
        if value.get("plan_digest") != plan.plan_digest or value.get("complete") != plan.complete:
            raise ArgumentError("LLM evidence selection plan digest or completeness is invalid")
        if value.get("retention") != "metadata_only_manifest_and_health_evidence" or value.get("secret_material") != "never_returned":
            raise ArgumentError("LLM evidence selection plan retention contract is invalid")
        if canonical_json(value) != canonical_json(plan.to_dict()):
            raise ArgumentError("LLM evidence selection plan is not canonical")
        return plan


class AutonomousLLMEvidenceAdapterSelector:
    """Deterministic selector over one registry snapshot."""

    def __init__(self, registry: AutonomousLLMEvidenceAdapterRegistry) -> None:
        if not isinstance(registry, AutonomousLLMEvidenceAdapterRegistry):
            raise ArgumentError("LLM evidence adapter selector requires a typed registry")
        self.registry = registry

    def select_for_domains(
        self,
        domains: Sequence[str],
        *,
        capability: str | None = None,
        strategy: str = "lexicographic_adapter_id",
        selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
        min_score: float = 0.0,
        min_margin: float = 0.0,
    ) -> AutonomousLLMEvidenceAdapterSelectionPlan:
        requested = _domains(domains)
        if strategy not in _SELECTION_STRATEGIES:
            raise ArgumentError("LLM evidence selector strategy is invalid")
        if strategy == "lexicographic_adapter_id" and selection_signals is not None:
            raise ArgumentError("lexicographic LLM evidence selection cannot consume signals")
        if strategy == "weighted_evidence" and selection_signals is None:
            raise ArgumentError("weighted LLM evidence selection requires explicit signals")
        min_score = _finite("LLM evidence selection min_score", min_score, 0, 1)
        min_margin = _finite("LLM evidence selection min_margin", min_margin, 0, 1)
        signals: dict[str, dict[str, Any]] = {}
        if selection_signals is not None:
            if not isinstance(selection_signals, Mapping) or len(selection_signals) > MAX_AUTONOMOUS_LLM_EVIDENCE_SELECTION_CANDIDATES:
                raise ArgumentError("LLM evidence selection signals are outside their bound")
            known = {manifest.adapter_id for manifest in self.registry.manifests()}
            for adapter_id, raw in selection_signals.items():
                normalized_id = _identifier("LLM evidence selection signal adapter_id", adapter_id)
                if normalized_id not in known or not isinstance(raw, Mapping):
                    raise ArgumentError("LLM evidence selection signal names an unknown or malformed adapter")
                _safe_metadata(raw, "LLM evidence selection signal")
                eligible = raw.get("eligible", True)
                if not isinstance(eligible, bool):
                    raise ArgumentError("LLM evidence selection signal eligible must be boolean")
                score = _finite("LLM evidence selection signal score", raw.get("score", 0.5), 0, 1)
                signals[normalized_id] = {**dict(raw), "eligible": eligible, "score": score}
        signal_digest = None if strategy != "weighted_evidence" else content_digest({"signals": [{"adapter_id": key, **signals[key]} for key in sorted(signals)]})
        rows: list[AutonomousLLMEvidenceAdapterSelectionRow] = []
        for domain in requested:
            candidates = self.registry.candidates(domain, capability)
            descriptors = [
                signals.get(manifest.adapter_id, {"eligible": True, "score": 0.5})
                if strategy == "weighted_evidence"
                else {"eligible": True, "score": 0.0}
                for manifest in candidates
            ]
            eligible = tuple(bool(descriptor["eligible"]) for descriptor in descriptors)
            scores = tuple(float(descriptor["score"]) for descriptor in descriptors)
            ranked_indexes = sorted(
                (index for index, is_eligible in enumerate(eligible) if is_eligible),
                key=lambda index: (-scores[index], candidates[index].adapter_id),
            )
            top_index = ranked_indexes[0] if ranked_indexes else None
            top_score = scores[top_index] if top_index is not None else 0.0
            second_score = scores[ranked_indexes[1]] if len(ranked_indexes) > 1 else 0.0
            margin = top_score - second_score if top_index is not None else 0.0
            reason = (
                "no_matching_adapter" if not candidates else
                "no_eligible_adapter" if top_index is None else
                "selection_below_min_score" if top_score < min_score else
                "insufficient_selection_margin" if margin < min_margin else
                strategy
            )
            selected = candidates[top_index] if reason == strategy and top_index is not None else None
            rows.append(
                AutonomousLLMEvidenceAdapterSelectionRow(
                    domain=domain,
                    status="selected" if selected is not None else "missing",
                    adapter_id=None if selected is None else selected.adapter_id,
                    manifest_digest=None if selected is None else selected.manifest_digest,
                    candidate_ids=tuple(manifest.adapter_id for manifest in candidates),
                    candidate_manifest_digests=tuple(manifest.manifest_digest for manifest in candidates),
                    candidate_scores=scores,
                    candidate_eligible=eligible,
                    reason=reason,
                )
            )
        return AutonomousLLMEvidenceAdapterSelectionPlan(
            domains=requested,
            capability=None if capability is None else _identifier("LLM evidence selection capability", capability),
            registry_digest=self.registry.registry_digest,
            rows=tuple(rows),
            strategy=strategy,
            signal_digest=signal_digest,
        )

    def select_adaptive_for_domains(
        self,
        domains: Sequence[str],
        selection_signals: Mapping[str, Mapping[str, Any]],
        *,
        capability: str | None = None,
        min_score: float = 0.0,
        min_margin: float = 0.0,
    ) -> AutonomousLLMEvidenceAdapterSelectionPlan:
        return self.select_for_domains(
            domains,
            capability=capability,
            strategy="weighted_evidence",
            selection_signals=selection_signals,
            min_score=min_score,
            min_margin=min_margin,
        )

    def create_router_from_selection(self, plan: AutonomousLLMEvidenceAdapterSelectionPlan | Mapping[str, Any]) -> Any:
        typed = plan if isinstance(plan, AutonomousLLMEvidenceAdapterSelectionPlan) else AutonomousLLMEvidenceAdapterSelectionPlan.from_dict(plan)
        self.registry.verify_selection(typed)
        if not typed.complete:
            raise ArgumentError("LLM evidence selection is incomplete")
        adapters = {row.domain: self.registry.resolve(row.domain, row.adapter_id or "") for row in typed.rows}
        from .autonomous_evidence_llm_adapter import AutonomousLLMEvidenceAdapterRouter

        return AutonomousLLMEvidenceAdapterRouter(adapters, require_all_domains=len(adapters) == len(AUTONOMOUS_DOMAIN_NAMES))


@dataclass(frozen=True, slots=True)
class AutonomousLLMEvidenceAdapterHealthObservation:
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
        _identifier("LLM evidence health adapter_id", self.adapter_id)
        _digest("LLM evidence health manifest_digest", self.manifest_digest)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES or self.observation_kind not in _HEALTH_KINDS or self.outcome not in _HEALTH_OUTCOMES:
            raise ArgumentError("LLM evidence health domain, kind, or outcome is invalid")
        _identifier("LLM evidence health status", self.status)
        object.__setattr__(self, "latency_ms", _finite("LLM evidence health latency_ms", self.latency_ms, 0, 86_400_000))
        if self.cost_units is not None:
            object.__setattr__(self, "cost_units", _finite("LLM evidence health cost_units", self.cost_units, 0, 1_000_000))
        if self.failure_class is not None:
            _identifier("LLM evidence health failure_class", self.failure_class)
        if self.evaluator_reward is not None:
            object.__setattr__(self, "evaluator_reward", _finite("LLM evidence health evaluator_reward", self.evaluator_reward, -1, 1))
        if self.evaluator_passed is not None and not isinstance(self.evaluator_passed, bool):
            raise ArgumentError("LLM evidence health evaluator_passed must be boolean or None")
        for name, value in (("evaluator_id", self.evaluator_id), ("evaluator_version", self.evaluator_version)):
            if value is not None:
                _identifier(f"LLM evidence health {name}", value)
        _optional_digest("LLM evidence health evidence_digest", self.evidence_digest)
        if self.observation_kind == "evaluation":
            if self.outcome != "unknown" or self.evaluator_reward is None:
                raise ArgumentError("LLM evidence evaluation observations require unknown outcome and explicit reward")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_OBSERVATION_SCHEMA,
            "adapter_id": self.adapter_id,
            "manifest_digest": self.manifest_digest,
            "domain": self.domain,
            "observation_kind": self.observation_kind,
            "outcome": self.outcome,
            "status": self.status,
            "latency_ms": self.latency_ms,
            "cost_units": self.cost_units,
            "failure_class": self.failure_class,
            "evaluator_reward": self.evaluator_reward,
            "evaluator_passed": self.evaluator_passed,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "evidence_digest": self.evidence_digest,
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }


def _observation_from_dict(value: Mapping[str, Any]) -> AutonomousLLMEvidenceAdapterHealthObservation:
    if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_OBSERVATION_SCHEMA:
        raise ArgumentError("LLM evidence health observation schema is invalid")
    allowed = {"schema", "adapter_id", "manifest_digest", "domain", "observation_kind", "outcome", "status", "latency_ms", "cost_units", "failure_class", "evaluator_reward", "evaluator_passed", "evaluator_id", "evaluator_version", "evidence_digest", "retention", "secret_material"}
    if set(value) != allowed or value.get("retention") != _RETENTION or value.get("secret_material") != "never_returned":
        raise ArgumentError("LLM evidence health observation contains unsupported fields")
    observation = AutonomousLLMEvidenceAdapterHealthObservation(
        adapter_id=_identifier("LLM evidence health adapter_id", value.get("adapter_id")),
        manifest_digest=_digest("LLM evidence health manifest_digest", value.get("manifest_digest")),
        domain=_identifier("LLM evidence health domain", value.get("domain")),
        observation_kind=_text("LLM evidence health observation_kind", value.get("observation_kind"), 32),
        outcome=_text("LLM evidence health outcome", value.get("outcome"), 32),
        status=_identifier("LLM evidence health status", value.get("status")),
        latency_ms=_finite("LLM evidence health latency_ms", value.get("latency_ms"), 0, 86_400_000),
        cost_units=None if value.get("cost_units") is None else _finite("LLM evidence health cost_units", value.get("cost_units"), 0, 1_000_000),
        failure_class=None if value.get("failure_class") is None else _identifier("LLM evidence health failure_class", value.get("failure_class")),
        evaluator_reward=None if value.get("evaluator_reward") is None else _finite("LLM evidence health evaluator_reward", value.get("evaluator_reward"), -1, 1),
        evaluator_passed=value.get("evaluator_passed"),
        evaluator_id=None if value.get("evaluator_id") is None else _identifier("LLM evidence health evaluator_id", value.get("evaluator_id")),
        evaluator_version=None if value.get("evaluator_version") is None else _identifier("LLM evidence health evaluator_version", value.get("evaluator_version")),
        evidence_digest=_optional_digest("LLM evidence health evidence_digest", value.get("evidence_digest")),
    )
    if canonical_json(value) != canonical_json(observation.to_dict()):
        raise ArgumentError("LLM evidence health observation is not canonical")
    return observation


@dataclass(frozen=True, slots=True)
class AutonomousLLMEvidenceAdapterHealthEvent:
    sequence: int
    previous_digest: str
    observation: AutonomousLLMEvidenceAdapterHealthObservation
    created_at: float
    event_digest: str

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA,
            "sequence": self.sequence,
            "previous_digest": self.previous_digest,
            "observation": self.observation.to_dict(),
            "created_at": self.created_at,
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }

    def __post_init__(self) -> None:
        _integer("LLM evidence health event sequence", self.sequence, 1, MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_EVENTS)
        if self.sequence == 1:
            if self.previous_digest != "":
                raise ArgumentError("first LLM evidence health event must have an empty previous digest")
        else:
            _digest("LLM evidence health event previous_digest", self.previous_digest)
        _finite("LLM evidence health event created_at", self.created_at, 0, 9_000_000_000_000_000)
        _digest("LLM evidence health event event_digest", self.event_digest)
        if content_digest(self._payload()) != self.event_digest:
            raise ArgumentError("LLM evidence health event digest is invalid")

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "event_digest": self.event_digest}


class InMemoryAutonomousLLMEvidenceAdapterHealthStore:
    """Hash-chained, metadata-only adapter health ledger."""

    def __init__(self, *, max_events: int = MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_EVENTS, clock: Callable[[], float] = time.time) -> None:
        self.max_events = _integer("LLM evidence health max_events", max_events, 1, MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_EVENTS)
        if not callable(clock):
            raise ArgumentError("LLM evidence health clock must be callable")
        self._clock = clock
        self._events: list[AutonomousLLMEvidenceAdapterHealthEvent] = []
        self._lock = threading.RLock()

    def record(self, observation: AutonomousLLMEvidenceAdapterHealthObservation | Mapping[str, Any]) -> dict[str, Any]:
        normalized = observation if isinstance(observation, AutonomousLLMEvidenceAdapterHealthObservation) else _observation_from_dict(observation)
        if not isinstance(normalized, AutonomousLLMEvidenceAdapterHealthObservation):
            raise ArgumentError("LLM evidence health observation is malformed")
        with self._lock:
            if len(self._events) >= self.max_events:
                raise ArgumentError("LLM evidence health event capacity is exhausted")
            previous = self._events[-1].event_digest if self._events else ""
            descriptor = {
                "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA,
                "sequence": len(self._events) + 1,
                "previous_digest": previous,
                "observation": normalized.to_dict(),
                "created_at": _finite("LLM evidence health clock", self._clock(), 0, 9_000_000_000_000_000),
                "retention": _RETENTION,
                "secret_material": "never_returned",
            }
            event = AutonomousLLMEvidenceAdapterHealthEvent(
                sequence=descriptor["sequence"],
                previous_digest=previous,
                observation=normalized,
                created_at=descriptor["created_at"],
                event_digest=content_digest(descriptor),
            )
            self._events.append(event)
            return {
                "schema": "bioprism-python-autonomous-llm-evidence-adapter-health-receipt/0.1",
                "sequence": event.sequence,
                "event_digest": event.event_digest,
                "adapter_id": normalized.adapter_id,
                "manifest_digest": normalized.manifest_digest,
                "domain": normalized.domain,
                "observation_kind": normalized.observation_kind,
                "retention": _RETENTION,
                "secret_material": "never_returned",
            }

    def record_acquisition(
        self,
        *,
        adapter_id: str,
        manifest_digest: str,
        domain: str,
        outcome: str,
        status: str,
        latency_ms: float,
        failure_class: str | None = None,
        cost_units: float | None = None,
        evidence_digest: str | None = None,
    ) -> dict[str, Any]:
        return self.record(
            AutonomousLLMEvidenceAdapterHealthObservation(
                adapter_id=adapter_id,
                manifest_digest=manifest_digest,
                domain=domain,
                observation_kind="acquisition",
                outcome=outcome,
                status=status,
                latency_ms=latency_ms,
                cost_units=cost_units,
                failure_class=failure_class,
                evidence_digest=evidence_digest,
            )
        )

    def record_evaluation(
        self,
        *,
        adapter_id: str,
        manifest_digest: str,
        domain: str,
        status: str,
        evaluator_reward: float,
        evaluator_passed: bool,
        evaluator_id: str | None = None,
        evaluator_version: str | None = None,
        evidence_digest: str | None = None,
    ) -> dict[str, Any]:
        return self.record(
            AutonomousLLMEvidenceAdapterHealthObservation(
                adapter_id=adapter_id,
                manifest_digest=manifest_digest,
                domain=domain,
                observation_kind="evaluation",
                outcome="unknown",
                status=status,
                latency_ms=0,
                evaluator_reward=evaluator_reward,
                evaluator_passed=evaluator_passed,
                evaluator_id=evaluator_id,
                evaluator_version=evaluator_version,
                evidence_digest=evidence_digest,
            )
        )

    def events(self) -> tuple[AutonomousLLMEvidenceAdapterHealthEvent, ...]:
        with self._lock:
            return tuple(self._events)

    def _aggregate(self, *, adapter_id: str | None = None, manifest_digest: str | None = None, domain: str | None = None) -> list[dict[str, Any]]:
        grouped: dict[tuple[str, str, str], dict[str, Any]] = {}
        for event in self._events:
            observation = event.observation
            if adapter_id is not None and observation.adapter_id != adapter_id:
                continue
            if manifest_digest is not None and observation.manifest_digest != manifest_digest:
                continue
            if domain is not None and observation.domain != domain:
                continue
            key = (observation.adapter_id, observation.manifest_digest, observation.domain)
            row = grouped.setdefault(
                key,
                {
                    "adapter_id": observation.adapter_id,
                    "manifest_digest": observation.manifest_digest,
                    "domain": observation.domain,
                    "attempts": 0,
                    "successes": 0,
                    "failures": 0,
                    "unknown": 0,
                    "total_latency_ms": 0.0,
                    "quality_observations": 0,
                    "reward_total": 0.0,
                    "quality_passed": 0,
                    "consecutive_failures": 0,
                    "last_status": None,
                    "last_outcome": None,
                    "last_sequence": 0,
                },
            )
            row["last_status"] = observation.status
            row["last_outcome"] = observation.outcome
            row["last_sequence"] = event.sequence
            if observation.observation_kind == "acquisition":
                row["attempts"] += 1
                row["total_latency_ms"] += observation.latency_ms
                if observation.outcome == "success":
                    row["successes"] += 1
                    row["consecutive_failures"] = 0
                elif observation.outcome == "failure":
                    row["failures"] += 1
                    row["consecutive_failures"] += 1
                else:
                    row["unknown"] += 1
            else:
                row["quality_observations"] += 1
                row["reward_total"] += float(observation.evaluator_reward or 0)
                row["quality_passed"] += int(observation.evaluator_passed is True)
        return list(grouped.values())

    def health(
        self,
        *,
        adapter_id: str | None = None,
        manifest_digest: str | None = None,
        domain: str | None = None,
        min_attempts: int = 3,
        failure_threshold: float = 0.75,
        limit: int = MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_QUERY_LIMIT,
    ) -> list[dict[str, Any]]:
        if adapter_id is not None:
            _identifier("LLM evidence health query adapter_id", adapter_id)
        if manifest_digest is not None:
            _digest("LLM evidence health query manifest_digest", manifest_digest)
        if domain is not None:
            domain = _identifier("LLM evidence health query domain", domain)
            if domain not in AUTONOMOUS_DOMAIN_NAMES:
                raise ArgumentError("LLM evidence health query domain is unsupported")
        min_attempts = _integer("LLM evidence health min_attempts", min_attempts, 1, MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_EVENTS)
        failure_threshold = _finite("LLM evidence health failure_threshold", failure_threshold, 0, 1)
        limit = _integer("LLM evidence health limit", limit, 1, MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_QUERY_LIMIT)
        result: list[dict[str, Any]] = []
        with self._lock:
            for row in sorted(self._aggregate(adapter_id=adapter_id, manifest_digest=manifest_digest, domain=domain), key=lambda item: (item["domain"], item["adapter_id"]))[:limit]:
                attempts = row["attempts"]
                failures = row["failures"]
                quality = row["quality_observations"]
                result.append(
                    {
                        "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_SCHEMA,
                        "adapter_id": row["adapter_id"],
                        "manifest_digest": row["manifest_digest"],
                        "domain": row["domain"],
                        "attempts": attempts,
                        "successes": row["successes"],
                        "failures": failures,
                        "unknown": row["unknown"],
                        "success_rate": row["successes"] / attempts if attempts else 0.0,
                        "failure_rate": failures / attempts if attempts else 0.0,
                        "mean_latency_ms": row["total_latency_ms"] / attempts if attempts else 0.0,
                        "quality_observations": quality,
                        "evaluator_reward_mean": row["reward_total"] / quality if quality else None,
                        "evaluator_pass_rate": row["quality_passed"] / quality if quality else None,
                        "consecutive_failures": row["consecutive_failures"],
                        "last_status": row["last_status"],
                        "last_outcome": row["last_outcome"],
                        "last_sequence": row["last_sequence"],
                        "circuit": "open" if attempts >= min_attempts and failures / attempts >= failure_threshold else "closed",
                        "retention": "aggregated_metadata_only",
                        "secret_material": "never_returned",
                    }
                )
        return result

    def selection_signals(
        self,
        *,
        manifest_digests: Mapping[str, str] | None = None,
        min_attempts: int = 3,
        failure_threshold: float = 0.75,
    ) -> dict[str, dict[str, Any]]:
        if manifest_digests is not None:
            if not isinstance(manifest_digests, Mapping):
                raise ArgumentError("LLM evidence health manifest_digests must be a mapping")
            for adapter_id, digest in manifest_digests.items():
                _identifier("LLM evidence health manifest adapter_id", adapter_id)
                _digest("LLM evidence health manifest digest", digest)
        health_rows = self.health(min_attempts=min_attempts, failure_threshold=failure_threshold, limit=MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_QUERY_LIMIT)
        by_id = {row["adapter_id"]: row for row in health_rows}
        selected_ids = set(manifest_digests or by_id)
        result: dict[str, dict[str, Any]] = {}
        for adapter_id in sorted(selected_ids):
            row = by_id.get(adapter_id)
            if row is None:
                result[adapter_id] = {"eligible": True, "score": 0.5, "attempts": 0, "success_rate": 0.0, "exploration": True}
                continue
            success_rate = float(row["success_rate"])
            quality = 0.5 if row["evaluator_reward_mean"] is None else (float(row["evaluator_reward_mean"]) + 1) / 2
            latency_score = 1 / (1 + float(row["mean_latency_ms"]) / 1000)
            score = max(0.0, min(1.0, 0.55 * success_rate + 0.30 * quality + 0.15 * latency_score))
            result[adapter_id] = {
                "eligible": row["circuit"] == "closed",
                "score": score,
                "attempts": row["attempts"],
                "success_rate": success_rate,
                "failure_rate": row["failure_rate"],
                "quality_mean": row["evaluator_reward_mean"],
                "mean_latency_ms": row["mean_latency_ms"],
                "circuit": row["circuit"],
            }
        return result

    def snapshot(self) -> dict[str, Any]:
        with self._lock:
            events = [event.to_dict() for event in self._events]
            payload = {
                "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA,
                "sequence": len(events),
                "head_digest": self._events[-1].event_digest if self._events else "",
                "events": events,
                "retention": _RETENTION,
                "secret_material": "never_returned",
            }
            _json_bytes(payload, "LLM evidence health snapshot", MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_SNAPSHOT_BYTES)
            return {**payload, "snapshot_digest": content_digest(payload)}

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        if not isinstance(snapshot, Mapping) or snapshot.get("schema") != AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA:
            raise ArgumentError("LLM evidence health snapshot schema is invalid")
        allowed = {"schema", "sequence", "head_digest", "events", "retention", "secret_material", "snapshot_digest"}
        if set(snapshot) != allowed or snapshot.get("retention") != _RETENTION or snapshot.get("secret_material") != "never_returned":
            raise ArgumentError("LLM evidence health snapshot contains unsupported fields")
        raw_events = snapshot.get("events")
        if not isinstance(raw_events, Sequence) or isinstance(raw_events, (str, bytes)) or len(raw_events) > self.max_events:
            raise ArgumentError("LLM evidence health snapshot events are outside their bound")
        unsigned = dict(snapshot)
        supplied = _digest("LLM evidence health snapshot snapshot_digest", unsigned.pop("snapshot_digest", None))
        if supplied != content_digest(unsigned):
            raise ArgumentError("LLM evidence health snapshot digest is invalid")
        restored: list[AutonomousLLMEvidenceAdapterHealthEvent] = []
        for index, raw in enumerate(raw_events, start=1):
            if not isinstance(raw, Mapping):
                raise ArgumentError("LLM evidence health snapshot event is malformed")
            observation = _observation_from_dict(raw.get("observation", {}))
            event = AutonomousLLMEvidenceAdapterHealthEvent(
                sequence=_integer("LLM evidence health event sequence", raw.get("sequence"), 1, self.max_events),
                previous_digest=raw.get("previous_digest"),
                observation=observation,
                created_at=_finite("LLM evidence health event created_at", raw.get("created_at"), 0, 9_000_000_000_000_000),
                event_digest=_digest("LLM evidence health event event_digest", raw.get("event_digest")),
            )
            if event.sequence != index or event.previous_digest != (restored[-1].event_digest if restored else ""):
                raise ArgumentError("LLM evidence health snapshot chain is not contiguous")
            if set(raw) != {"schema", "sequence", "previous_digest", "observation", "created_at", "retention", "secret_material", "event_digest"} or raw.get("schema") != AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA or raw.get("retention") != _RETENTION or raw.get("secret_material") != "never_returned":
                raise ArgumentError("LLM evidence health snapshot event retention is invalid")
            restored.append(event)
        if snapshot.get("sequence") != len(restored) or snapshot.get("head_digest") != (restored[-1].event_digest if restored else ""):
            raise ArgumentError("LLM evidence health snapshot head is invalid")
        with self._lock:
            self._events = restored

    def verify_integrity(self) -> dict[str, Any]:
        snapshot = self.snapshot()
        return {"verified": True, "events": snapshot["sequence"], "head_digest": snapshot["head_digest"]}


class AutonomousLLMEvidenceAdapterHealthSnapshotTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class TransactionalAutonomousLLMEvidenceAdapterHealthSnapshotTextStore(AutonomousLLMEvidenceAdapterHealthSnapshotTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class JsonAutonomousLLMEvidenceAdapterHealthPersistence:
    """Canonical text persistence for the adapter health snapshot."""

    def __init__(self, store: AutonomousLLMEvidenceAdapterHealthSnapshotTextStore, *, max_bytes: int = MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_SNAPSHOT_BYTES) -> None:
        if not callable(getattr(store, "read", None)) or not callable(getattr(store, "write", None)):
            raise ArgumentError("LLM evidence health persistence store is malformed")
        self.store = store
        self.max_bytes = _integer("LLM evidence health persistence max_bytes", max_bytes, 1, MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_SNAPSHOT_BYTES)

    def read(self) -> dict[str, Any] | None:
        value = self.store.read()
        if value is None:
            return None
        if not isinstance(value, str) or len(value.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("LLM evidence health persistence value is outside its bound")
        try:
            parsed = json.loads(value)
        except (TypeError, ValueError) as error:
            raise ArgumentError("LLM evidence health persistence value is invalid JSON") from error
        if not isinstance(parsed, Mapping):
            raise ArgumentError("LLM evidence health persistence value must be a mapping")
        _json_bytes(parsed, "LLM evidence health persistence value", self.max_bytes)
        return dict(parsed)

    def write(self, snapshot: Mapping[str, Any]) -> None:
        encoded = canonical_json(snapshot)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("LLM evidence health persistence snapshot exceeds its bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousLLMEvidenceAdapterHealthPersistence(JsonAutonomousLLMEvidenceAdapterHealthPersistence):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        if expected_snapshot_digest is not None:
            _digest("LLM evidence health expected snapshot digest", expected_snapshot_digest)
        encoded = canonical_json(snapshot)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("LLM evidence health persistence snapshot exceeds its bound")
        callback = getattr(self.store, "write_if_unchanged", None)
        if not callable(callback):
            raise ArgumentError("LLM evidence health store does not support compare-and-swap")
        result = callback(expected_snapshot_digest, encoded)
        if not isinstance(result, bool):
            raise ArgumentError("LLM evidence health compare-and-swap returned a non-boolean")
        return result


class AutonomousLLMEvidenceAdapterHealthPersistenceCoordinator:
    """Restore/flush coordinator with optional compare-and-swap fencing."""

    def __init__(self, health_store: InMemoryAutonomousLLMEvidenceAdapterHealthStore, persistence: Any) -> None:
        if not isinstance(health_store, InMemoryAutonomousLLMEvidenceAdapterHealthStore):
            raise ArgumentError("LLM evidence health coordinator requires a typed health store")
        if not callable(getattr(persistence, "read", None)) or not callable(getattr(persistence, "write", None)):
            raise ArgumentError("LLM evidence health coordinator persistence is malformed")
        self.health_store = health_store
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None

    def restore(self) -> dict[str, Any]:
        snapshot = self.persistence.read()
        if snapshot is not None:
            self.health_store.restore(snapshot)
            self._expected_snapshot_digest = snapshot.get("snapshot_digest")
        else:
            self._expected_snapshot_digest = None
        return self.health_store.verify_integrity()

    def flush(self) -> dict[str, Any]:
        snapshot = self.health_store.snapshot()
        write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
        if callable(write_if_unchanged):
            if not write_if_unchanged(self._expected_snapshot_digest, snapshot):
                raise ArgumentError("LLM evidence health persistence compare-and-swap conflict")
        else:
            self.persistence.write(snapshot)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot


@dataclass(frozen=True, slots=True)
class AutonomousLLMEvidenceFailoverPolicy:
    max_failovers: int = 0

    def __post_init__(self) -> None:
        _integer("LLM evidence failover max_failovers", self.max_failovers, 0, MAX_AUTONOMOUS_LLM_EVIDENCE_FAILOVERS)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_FAILOVER_POLICY_SCHEMA,
            "max_failovers": self.max_failovers,
            "execution": "caller_controlled_reviewed_candidate_failover;no_fuzzy_selection",
            "retention": "metadata_only_candidate_identity_and_failure_class",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousLLMEvidenceFailoverEvent:
    domain: str
    candidate_id: str
    candidate_manifest_digest: str
    candidate_rank: int
    status: str
    failure_class: str | None
    retryable: bool
    failovers_used: int
    remaining_candidates: int

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES or self.status not in {"candidate_failed", "fallback_started", "candidate_succeeded", "failover_exhausted"}:
            raise ArgumentError("LLM evidence failover event domain or status is invalid")
        _identifier("LLM evidence failover candidate_id", self.candidate_id)
        _digest("LLM evidence failover candidate_manifest_digest", self.candidate_manifest_digest)
        _integer("LLM evidence failover candidate_rank", self.candidate_rank, 1, MAX_AUTONOMOUS_LLM_EVIDENCE_SELECTION_CANDIDATES)
        if self.failure_class is not None:
            _identifier("LLM evidence failover failure_class", self.failure_class)
        if not isinstance(self.retryable, bool):
            raise ArgumentError("LLM evidence failover retryable must be boolean")
        _integer("LLM evidence failover failovers_used", self.failovers_used, 0, MAX_AUTONOMOUS_LLM_EVIDENCE_FAILOVERS)
        _integer("LLM evidence failover remaining_candidates", self.remaining_candidates, 0, MAX_AUTONOMOUS_LLM_EVIDENCE_SELECTION_CANDIDATES)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_FAILOVER_EVENT_SCHEMA,
            "domain": self.domain,
            "candidate_id": self.candidate_id,
            "candidate_manifest_digest": self.candidate_manifest_digest,
            "candidate_rank": self.candidate_rank,
            "status": self.status,
            "failure_class": self.failure_class,
            "retryable": self.retryable,
            "failovers_used": self.failovers_used,
            "remaining_candidates": self.remaining_candidates,
            "retention": "metadata_only;candidate_identity_and_failure_class",
            "secret_material": "never_returned",
        }


def _failure_class(error: BaseException) -> tuple[str, bool]:
    if isinstance(error, CredentialError):
        return "credential_error", False
    if isinstance(error, ArgumentError):
        return "invalid_request", False
    if isinstance(error, ProviderError):
        if error.circuit_open:
            return "circuit_open", True
        return ("provider_retryable" if error.retryable else "provider_error", bool(error.retryable))
    return "adapter_error", False


def _context_key(context: Mapping[str, Any]) -> str:
    request = context.get("request")
    requirement = context.get("requirement")
    if not isinstance(request, Mapping):
        raise ArgumentError("LLM evidence failover context request is malformed")
    return content_digest(
        {
            "plan_digest": context.get("plan_digest"),
            "requirement_id": getattr(requirement, "requirement_id", requirement.get("requirement_id") if isinstance(requirement, Mapping) else None),
            "source_id": request.get("source_id"),
            "request_id": request.get("request_id"),
        }
    )


class AutonomousLLMEvidenceAdapterFailoverAcquirer:
    """Execute a verified selection plan with explicit transient-failure fallback."""

    def __init__(
        self,
        registry: AutonomousLLMEvidenceAdapterRegistry,
        plan: AutonomousLLMEvidenceAdapterSelectionPlan | Mapping[str, Any],
        *,
        policy: AutonomousLLMEvidenceFailoverPolicy | None = None,
        health_store: InMemoryAutonomousLLMEvidenceAdapterHealthStore | None = None,
        observe_failover: Callable[[AutonomousLLMEvidenceFailoverEvent], Any] | None = None,
        provider_contracts: Any | None = None,
    ) -> None:
        if not isinstance(registry, AutonomousLLMEvidenceAdapterRegistry):
            raise ArgumentError("LLM evidence failover requires a typed registry")
        typed_plan = plan if isinstance(plan, AutonomousLLMEvidenceAdapterSelectionPlan) else AutonomousLLMEvidenceAdapterSelectionPlan.from_dict(plan)
        registry.verify_selection(typed_plan)
        if not isinstance(policy, (AutonomousLLMEvidenceFailoverPolicy, type(None))):
            raise ArgumentError("LLM evidence failover policy is malformed")
        if health_store is not None and not isinstance(health_store, InMemoryAutonomousLLMEvidenceAdapterHealthStore):
            raise ArgumentError("LLM evidence failover health_store is malformed")
        if observe_failover is not None and not callable(observe_failover):
            raise ArgumentError("LLM evidence failover observer is malformed")
        if provider_contracts is not None:
            from .autonomous_evidence_provider_contract import AutonomousEvidenceProviderContractRegistry
            if not isinstance(provider_contracts, AutonomousEvidenceProviderContractRegistry):
                raise ArgumentError("LLM evidence failover provider_contracts is malformed")
            provider_contracts.verify()
        self.registry = registry
        self.plan = typed_plan
        self.policy = policy or AutonomousLLMEvidenceFailoverPolicy()
        self.health_store = health_store
        self.observe_failover = observe_failover
        self.provider_contracts = provider_contracts
        self._selected: dict[str, str] = {}
        self._lock = threading.RLock()

    def _candidate_order(self, row: AutonomousLLMEvidenceAdapterSelectionRow) -> tuple[str, ...]:
        indexes = [index for index, eligible in enumerate(row.candidate_eligible) if eligible]
        indexes.sort(key=lambda index: (-row.candidate_scores[index], row.candidate_ids[index]))
        return tuple(row.candidate_ids[index] for index in indexes)

    def _emit(self, event: AutonomousLLMEvidenceFailoverEvent) -> None:
        if self.observe_failover is not None:
            self.observe_failover(event)

    def acquire(self, context: Mapping[str, Any]) -> Any:
        if not isinstance(context, Mapping):
            raise ArgumentError("LLM evidence failover context must be a mapping")
        requirement = context.get("requirement")
        domain = getattr(requirement, "domain", requirement.get("domain") if isinstance(requirement, Mapping) else None)
        row = next((candidate for candidate in self.plan.rows if candidate.domain == domain), None)
        if row is None:
            raise ArgumentError(f"LLM evidence failover plan does not cover domain: {domain}")
        candidates = self._candidate_order(row)
        if not candidates:
            raise ArgumentError(f"LLM evidence failover selection has no eligible candidates for {domain}")
        context_key = _context_key(context)
        last_error: Exception | None = None
        for candidate_index, adapter_id in enumerate(candidates[: self.policy.max_failovers + 1]):
            adapter = self.registry.resolve(domain, adapter_id)
            manifest = self.registry.manifest_for(domain, adapter_id)
            started = time.monotonic()
            try:
                if self.provider_contracts is None:
                    value = adapter.acquire(context)
                else:
                    value = self.provider_contracts.create_acquirer_for_adapter(adapter_id, domain).acquire(context)
                if self.health_store is not None:
                    self.health_store.record_acquisition(
                        adapter_id=adapter_id,
                        manifest_digest=manifest.manifest_digest,
                        domain=domain,
                        outcome="success",
                        status="observed",
                        latency_ms=max(0.0, (time.monotonic() - started) * 1000),
                    )
                with self._lock:
                    self._selected[context_key] = adapter_id
                self._emit(
                    AutonomousLLMEvidenceFailoverEvent(
                        domain=domain,
                        candidate_id=adapter_id,
                        candidate_manifest_digest=manifest.manifest_digest,
                        candidate_rank=candidate_index + 1,
                        status="candidate_succeeded",
                        failure_class=None,
                        retryable=False,
                        failovers_used=candidate_index,
                        remaining_candidates=max(0, len(candidates) - candidate_index - 1),
                    )
                )
                return value
            except Exception as error:
                last_error = error
                failure_class, retryable = _failure_class(error)
                remaining = max(0, len(candidates) - candidate_index - 1)
                can_failover = retryable and candidate_index < self.policy.max_failovers and remaining > 0
                if self.health_store is not None:
                    self.health_store.record_acquisition(
                        adapter_id=adapter_id,
                        manifest_digest=manifest.manifest_digest,
                        domain=domain,
                        outcome="failure",
                        status="failed",
                        latency_ms=max(0.0, (time.monotonic() - started) * 1000),
                        failure_class=failure_class,
                    )
                self._emit(
                    AutonomousLLMEvidenceFailoverEvent(
                        domain=domain,
                        candidate_id=adapter_id,
                        candidate_manifest_digest=manifest.manifest_digest,
                        candidate_rank=candidate_index + 1,
                        status="fallback_started" if can_failover else "failover_exhausted" if retryable else "candidate_failed",
                        failure_class=failure_class,
                        retryable=retryable,
                        failovers_used=candidate_index,
                        remaining_candidates=remaining,
                    )
                )
                if not can_failover:
                    raise
        raise last_error or ArgumentError("LLM evidence failover exhausted unexpectedly")

    def project(self, value: Any, context: Mapping[str, Any]) -> Sequence[Mapping[str, Any]]:
        key = _context_key(context)
        with self._lock:
            selected_id = self._selected.get(key)
        requirement = context.get("requirement")
        domain = getattr(requirement, "domain", requirement.get("domain") if isinstance(requirement, Mapping) else None)
        row = next((candidate for candidate in self.plan.rows if candidate.domain == domain), None)
        if row is None:
            raise ArgumentError(f"LLM evidence failover plan does not cover domain: {domain}")
        selected_id = selected_id or (self._candidate_order(row)[0] if self._candidate_order(row) else None)
        if selected_id is None:
            return ()
        return self.registry.resolve(domain, selected_id).project_value(value, context)

    def record_evaluation(
        self,
        context: Mapping[str, Any],
        *,
        status: str,
        evaluator_reward: float,
        evaluator_passed: bool,
        evaluator_id: str | None = None,
        evaluator_version: str | None = None,
        evidence_digest: str | None = None,
    ) -> dict[str, Any]:
        """Credit the adapter selected for one transient evidence request.

        The caller supplies the evaluator's explicit reward; this method never infers quality
        from transport success.  Only the selected adapter identity and evaluator metadata enter
        the health ledger.
        """

        if self.health_store is None:
            raise ArgumentError("LLM evidence failover has no health store for evaluation credit")
        selected_id = self._selected.get(_context_key(context))
        if selected_id is None:
            raise ArgumentError("LLM evidence failover has no selected adapter for evaluation credit")
        requirement = context.get("requirement")
        domain = getattr(requirement, "domain", requirement.get("domain") if isinstance(requirement, Mapping) else None)
        manifest = self.registry.manifest_for(domain, selected_id)
        return self.health_store.record_evaluation(
            adapter_id=selected_id,
            manifest_digest=manifest.manifest_digest,
            domain=domain,
            status=status,
            evaluator_reward=evaluator_reward,
            evaluator_passed=evaluator_passed,
            evaluator_id=evaluator_id,
            evaluator_version=evaluator_version,
            evidence_digest=evidence_digest,
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_FAILOVER_POLICY_SCHEMA,
            "selection_plan_digest": self.plan.plan_digest,
            "registry_digest": self.plan.registry_digest,
            "provider_contract_registry_digest": None if self.provider_contracts is None else self.provider_contracts.registry_digest,
            "provider_contracts_enabled": self.provider_contracts is not None,
            "max_failovers": self.policy.max_failovers,
            "domains": list(self.plan.domains),
            "health_recording": self.health_store is not None,
            "execution": "caller_controlled_reviewed_candidate_failover;no_fuzzy_selection",
            "retention": "metadata_only_candidate_identity_and_failure_class",
            "secret_material": "never_returned",
        }


def create_autonomous_llm_evidence_adapter_failover_acquirer(
    registry: AutonomousLLMEvidenceAdapterRegistry,
    plan: AutonomousLLMEvidenceAdapterSelectionPlan | Mapping[str, Any],
    *,
    policy: AutonomousLLMEvidenceFailoverPolicy | None = None,
    health_store: InMemoryAutonomousLLMEvidenceAdapterHealthStore | None = None,
    observe_failover: Callable[[AutonomousLLMEvidenceFailoverEvent], Any] | None = None,
    provider_contracts: Any | None = None,
) -> AutonomousLLMEvidenceAdapterFailoverAcquirer:
    return AutonomousLLMEvidenceAdapterFailoverAcquirer(
        registry,
        plan,
        policy=policy,
        health_store=health_store,
        observe_failover=observe_failover,
        provider_contracts=provider_contracts,
    )


__all__ = [
    "AUTONOMOUS_LLM_EVIDENCE_ADAPTER_REGISTRY_SCHEMA",
    "AUTONOMOUS_LLM_EVIDENCE_ADAPTER_MANIFEST_SCHEMA",
    "AUTONOMOUS_LLM_EVIDENCE_ADAPTER_SELECTION_SCHEMA",
    "AUTONOMOUS_LLM_EVIDENCE_ADAPTER_SELECTION_ROW_SCHEMA",
    "AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_SCHEMA",
    "AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_OBSERVATION_SCHEMA",
    "AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA",
    "AUTONOMOUS_LLM_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA",
    "AUTONOMOUS_LLM_EVIDENCE_FAILOVER_POLICY_SCHEMA",
    "AUTONOMOUS_LLM_EVIDENCE_FAILOVER_EVENT_SCHEMA",
    "MAX_AUTONOMOUS_LLM_EVIDENCE_ADAPTERS",
    "MAX_AUTONOMOUS_LLM_EVIDENCE_SELECTION_CANDIDATES",
    "MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_EVENTS",
    "MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_SNAPSHOT_BYTES",
    "MAX_AUTONOMOUS_LLM_EVIDENCE_HEALTH_QUERY_LIMIT",
    "MAX_AUTONOMOUS_LLM_EVIDENCE_FAILOVERS",
    "AutonomousLLMEvidenceAdapterManifest",
    "AutonomousLLMEvidenceAdapterRegistry",
    "AutonomousLLMEvidenceAdapterSelectionRow",
    "AutonomousLLMEvidenceAdapterSelectionPlan",
    "AutonomousLLMEvidenceAdapterSelector",
    "AutonomousLLMEvidenceAdapterHealthObservation",
    "AutonomousLLMEvidenceAdapterHealthEvent",
    "InMemoryAutonomousLLMEvidenceAdapterHealthStore",
    "AutonomousLLMEvidenceAdapterHealthSnapshotTextStore",
    "TransactionalAutonomousLLMEvidenceAdapterHealthSnapshotTextStore",
    "JsonAutonomousLLMEvidenceAdapterHealthPersistence",
    "TransactionalJsonAutonomousLLMEvidenceAdapterHealthPersistence",
    "AutonomousLLMEvidenceAdapterHealthPersistenceCoordinator",
    "AutonomousLLMEvidenceFailoverPolicy",
    "AutonomousLLMEvidenceFailoverEvent",
    "AutonomousLLMEvidenceAdapterFailoverAcquirer",
    "create_autonomous_llm_evidence_adapter_failover_acquirer",
]
