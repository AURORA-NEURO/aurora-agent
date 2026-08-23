"""Domain-neutral evaluator adapters for the autonomous brain.

The adapter layer gives engineering, research, operations, data, and biomedical applications one
bounded evaluator shape without pretending that a generic SDK can determine domain truth. Each
profile scores only caller-declared, normalized signals and returns a compact reward/approval
decision. Raw provider output, prompts, credentials, and source payloads remain outside this
module.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
import re
from typing import Any, Mapping, Sequence

from .brain import BrainOutcomeEvaluator, BrainRunError
from .memory import BrainMemoryError, _safe_value


MAX_DOMAIN_EVALUATOR_SIGNAL_COUNT = 64
MAX_DOMAIN_EVALUATOR_REFERENCE_COUNT = 64
MAX_DOMAIN_EVALUATOR_LIMITATION_COUNT = 32
MAX_DOMAIN_EVALUATOR_TEXT_BYTES = 256
DOMAIN_EVALUATOR_SCHEMA = "bioprism-brain-domain-evaluator/0.1"
_SAFE_SIGNAL = re.compile(r"^[A-Za-z][A-Za-z0-9_.-]{0,127}$")


def _text(name: str, value: Any, maximum: int = MAX_DOMAIN_EVALUATOR_TEXT_BYTES) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise BrainRunError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise BrainRunError(f"{name} exceeds its bounded size")
    return value


def _digest(value: Any) -> str:
    try:
        encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise BrainRunError("domain evaluator value must be JSON-safe") from error
    return hashlib.sha256(encoded).hexdigest()


@dataclass(frozen=True, slots=True)
class DomainEvaluatorProfile:
    """A caller-visible scoring policy over named bounded signals."""

    domain: str
    evaluator_id: str
    evaluator_version: str
    required_signals: tuple[str, ...]
    signal_weights: Mapping[str, float]
    pass_threshold: float = 1.0
    accepted_evidence_domains: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _text("domain evaluator domain", self.domain)
        _text("domain evaluator evaluator_id", self.evaluator_id)
        _text("domain evaluator evaluator_version", self.evaluator_version)
        if not isinstance(self.required_signals, Sequence) or isinstance(self.required_signals, (str, bytes)):
            raise BrainRunError("domain evaluator required_signals must be a sequence")
        if not self.required_signals or len(self.required_signals) > MAX_DOMAIN_EVALUATOR_SIGNAL_COUNT:
            raise BrainRunError("domain evaluator required_signals must contain 1..64 entries")
        seen: set[str] = set()
        for signal in self.required_signals:
            _text("domain evaluator required signal", signal, MAX_DOMAIN_EVALUATOR_TEXT_BYTES)
            if not _SAFE_SIGNAL.fullmatch(signal):
                raise BrainRunError(f"domain evaluator signal is not a safe identifier: {signal}")
            if signal in seen:
                raise BrainRunError(f"duplicate domain evaluator signal: {signal}")
            seen.add(signal)
        if not isinstance(self.signal_weights, Mapping) or not self.signal_weights:
            raise BrainRunError("domain evaluator signal_weights must be a non-empty mapping")
        total = 0.0
        for signal, weight in self.signal_weights.items():
            _text("domain evaluator weighted signal", signal)
            if not _SAFE_SIGNAL.fullmatch(signal):
                raise BrainRunError(f"domain evaluator weighted signal is not a safe identifier: {signal}")
            if not isinstance(weight, (int, float)) or isinstance(weight, bool) or not math.isfinite(float(weight)) or weight <= 0:
                raise BrainRunError("domain evaluator signal weights must be finite positive numbers")
            total += float(weight)
        if not any(signal in self.signal_weights for signal in self.required_signals):
            raise BrainRunError("domain evaluator must weight at least one required signal")
        if not isinstance(self.pass_threshold, (int, float)) or isinstance(self.pass_threshold, bool) or not 0 <= self.pass_threshold <= 1:
            raise BrainRunError("domain evaluator pass_threshold must be within [0, 1]")
        if not isinstance(self.accepted_evidence_domains, Sequence) or isinstance(
            self.accepted_evidence_domains, (str, bytes)
        ):
            raise BrainRunError("domain evaluator accepted_evidence_domains must be a sequence")
        accepted_domains: list[str] = []
        for accepted_domain in self.accepted_evidence_domains:
            _text("domain evaluator accepted evidence domain", accepted_domain)
            if accepted_domain in accepted_domains or accepted_domain == self.domain:
                raise BrainRunError("domain evaluator accepted evidence domains must be unique and distinct")
            accepted_domains.append(accepted_domain)
        if not math.isfinite(total) or total <= 0:
            raise BrainRunError("domain evaluator signal weights must have a finite positive sum")
        object.__setattr__(self, "accepted_evidence_domains", tuple(accepted_domains))

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_EVALUATOR_SCHEMA,
            "domain": self.domain,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "required_signals": list(self.required_signals),
            "signal_weights": dict(self.signal_weights),
            "pass_threshold": self.pass_threshold,
            "accepted_evidence_domains": list(self.accepted_evidence_domains),
            "execution": "caller_declared_signal_scoring_only",
        }


@dataclass(frozen=True, slots=True)
class DomainEvaluationEvidence:
    """Normalized, value-only evidence presented to a domain evaluator."""

    domain: str
    capability: str
    risk_class: str
    signals: Mapping[str, float]
    references: tuple[str, ...] = ()
    limitations: tuple[str, ...] = ()
    stage_plan_digest: str | None = None
    capability_contract_digests: tuple[str, ...] = ()
    selected_tool_names: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _text("domain evidence domain", self.domain)
        _text("domain evidence capability", self.capability)
        _text("domain evidence risk_class", self.risk_class)
        if not isinstance(self.signals, Mapping) or not self.signals:
            raise BrainRunError("domain evidence signals must be a non-empty mapping")
        if len(self.signals) > MAX_DOMAIN_EVALUATOR_SIGNAL_COUNT:
            raise BrainRunError("domain evidence signals exceed the bounded count")
        for signal, value in self.signals.items():
            _text("domain evidence signal", signal)
            if not _SAFE_SIGNAL.fullmatch(signal):
                raise BrainRunError(f"domain evidence signal is not a safe identifier: {signal}")
            if not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(float(value)) or not 0 <= value <= 1:
                raise BrainRunError("domain evidence signal values must be finite numbers within [0, 1]")
        for name, values, maximum in (
            ("references", self.references, MAX_DOMAIN_EVALUATOR_REFERENCE_COUNT),
            ("limitations", self.limitations, MAX_DOMAIN_EVALUATOR_LIMITATION_COUNT),
        ):
            if not isinstance(values, Sequence) or isinstance(values, (str, bytes)) or len(values) > maximum:
                raise BrainRunError(f"domain evidence {name} has an invalid bounded sequence")
            for value in values:
                _text(f"domain evidence {name} entry", value)
                if name == "references" and (
                    len(value) != 64 or any(character not in "0123456789abcdef" for character in value)
                ):
                    raise BrainRunError("domain evidence references must be lowercase SHA-256 digests")
        if self.stage_plan_digest is not None:
            if len(self.stage_plan_digest) != 64 or any(
                character not in "0123456789abcdef" for character in self.stage_plan_digest
            ):
                raise BrainRunError("domain evidence stage_plan_digest must be a lowercase SHA-256 digest")
        if not isinstance(self.capability_contract_digests, Sequence) or isinstance(
            self.capability_contract_digests, (str, bytes)
        ) or len(self.capability_contract_digests) > MAX_DOMAIN_EVALUATOR_SIGNAL_COUNT:
            raise BrainRunError("domain evidence capability_contract_digests are outside their bound")
        for digest in self.capability_contract_digests:
            if not isinstance(digest, str) or len(digest) != 64 or any(
                character not in "0123456789abcdef" for character in digest
            ):
                raise BrainRunError("domain evidence capability contract digests must be lowercase SHA-256 digests")
        if not isinstance(self.selected_tool_names, Sequence) or isinstance(
            self.selected_tool_names, (str, bytes)
        ) or len(self.selected_tool_names) > MAX_DOMAIN_EVALUATOR_SIGNAL_COUNT:
            raise BrainRunError("domain evidence selected_tool_names are outside their bound")
        for name in self.selected_tool_names:
            _text("domain evidence selected tool name", name)

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "DomainEvaluationEvidence":
        if not isinstance(value, Mapping):
            raise BrainRunError("domain evaluation evidence must be a mapping")
        allowed = {
            "schema",
            "domain",
            "capability",
            "risk_class",
            "signals",
            "references",
            "limitations",
            "retention",
            # Workflow evaluators add these bounded routing fields. Domain adapters deliberately
            # ignore them after validating the value-only evidence projection.
            "workflow_id",
            "workflow_digest",
            "stage_id",
            "required_signals",
            "stage_plan_digest",
            "capability_contract_digests",
            "selected_tool_names",
        }
        if any(not isinstance(key, str) for key in value) or set(value).difference(allowed):
            raise BrainRunError("domain evaluation evidence contains unsupported fields")
        raw_signals = value.get("signals")
        if not isinstance(raw_signals, Mapping):
            raise BrainRunError("domain evaluation evidence signals must be a mapping")
        workflow_id = value.get("workflow_id")
        if workflow_id is not None:
            _text("domain evaluation workflow_id", workflow_id)
        workflow_digest = value.get("workflow_digest")
        if workflow_digest is not None:
            _text("domain evaluation workflow_digest", workflow_digest)
            if len(workflow_digest) != 64 or any(character not in "0123456789abcdef" for character in workflow_digest):
                raise BrainRunError("domain evaluation workflow_digest must be a lowercase SHA-256 digest")
        stage_id = value.get("stage_id")
        if stage_id is not None:
            _text("domain evaluation stage_id", stage_id)
        required_signals = value.get("required_signals")
        if required_signals is not None:
            if not isinstance(required_signals, Sequence) or isinstance(required_signals, (str, bytes)):
                raise BrainRunError("domain evaluation required_signals must be a sequence")
            if len(required_signals) > MAX_DOMAIN_EVALUATOR_SIGNAL_COUNT:
                raise BrainRunError("domain evaluation required_signals exceed the bounded count")
            for signal in required_signals:
                _text("domain evaluation required signal", signal)
                if not _SAFE_SIGNAL.fullmatch(signal):
                    raise BrainRunError(f"domain evaluation required signal is not a safe identifier: {signal}")
        stage_plan_digest = value.get("stage_plan_digest")
        if stage_plan_digest is not None:
            if not isinstance(stage_plan_digest, str) or len(stage_plan_digest) != 64 or any(
                character not in "0123456789abcdef" for character in stage_plan_digest
            ):
                raise BrainRunError("domain evaluation stage_plan_digest must be a lowercase SHA-256 digest")
        contract_digests = value.get("capability_contract_digests", ())
        if not isinstance(contract_digests, Sequence) or isinstance(contract_digests, (str, bytes)):
            raise BrainRunError("domain evaluation capability_contract_digests must be a sequence")
        if len(contract_digests) > MAX_DOMAIN_EVALUATOR_SIGNAL_COUNT:
            raise BrainRunError("domain evaluation capability_contract_digests exceed the bounded count")
        for digest in contract_digests:
            if not isinstance(digest, str) or len(digest) != 64 or any(
                character not in "0123456789abcdef" for character in digest
            ):
                raise BrainRunError("domain evaluation capability contract digests must be lowercase SHA-256 digests")
        selected_tool_names = value.get("selected_tool_names", ())
        if not isinstance(selected_tool_names, Sequence) or isinstance(selected_tool_names, (str, bytes)):
            raise BrainRunError("domain evaluation selected_tool_names must be a sequence")
        if len(selected_tool_names) > MAX_DOMAIN_EVALUATOR_SIGNAL_COUNT:
            raise BrainRunError("domain evaluation selected_tool_names exceed the bounded count")
        for name in selected_tool_names:
            _text("domain evaluation selected tool name", name)
        normalized_signals: dict[str, float] = {}
        for signal, raw in raw_signals.items():
            if isinstance(raw, bool):
                normalized_signals[signal] = 1.0 if raw else 0.0
            elif isinstance(raw, (int, float)):
                normalized_signals[signal] = float(raw)
            else:
                raise BrainRunError("domain evaluation evidence signals must be booleans or numbers")
        references = value.get("references", ())
        limitations = value.get("limitations", ())
        if not isinstance(references, Sequence) or isinstance(references, (str, bytes)):
            raise BrainRunError("domain evaluation evidence references must be a sequence")
        if not isinstance(limitations, Sequence) or isinstance(limitations, (str, bytes)):
            raise BrainRunError("domain evaluation evidence limitations must be a sequence")
        try:
            safe = _safe_value(
                {
                    "domain": value.get("domain"),
                    "capability": value.get("capability"),
                    "risk_class": value.get("risk_class"),
                    "signals": normalized_signals,
                    "references": list(references),
                    "limitations": list(limitations),
                    "stage_plan_digest": stage_plan_digest,
                    "capability_contract_digests": list(contract_digests),
                    "selected_tool_names": list(selected_tool_names),
                }
            )
        except BrainMemoryError as error:
            raise BrainRunError("domain evaluation evidence contains forbidden content") from error
        return cls(
            domain=safe["domain"],
            capability=safe["capability"],
            risk_class=safe["risk_class"],
            signals=safe["signals"],
            references=tuple(safe["references"]),
            limitations=tuple(safe["limitations"]),
            stage_plan_digest=safe.get("stage_plan_digest"),
            capability_contract_digests=tuple(safe.get("capability_contract_digests", ())),
            selected_tool_names=tuple(safe.get("selected_tool_names", ())),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_EVALUATOR_SCHEMA,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "signals": dict(self.signals),
            "references": list(self.references),
            "limitations": list(self.limitations),
            "stage_plan_digest": self.stage_plan_digest,
            "capability_contract_digests": list(self.capability_contract_digests),
            "selected_tool_names": list(self.selected_tool_names),
            "retention": "value_only_digests_and_signal_scores",
        }


class DomainEvaluatorAdapter(BrainOutcomeEvaluator):
    """Turn a normalized domain profile into a :class:`BrainOutcomeEvaluator`."""

    def __init__(self, profile: DomainEvaluatorProfile) -> None:
        if not isinstance(profile, DomainEvaluatorProfile):
            raise BrainRunError("profile must be a DomainEvaluatorProfile")
        self.profile = profile
        super().__init__(self._evaluate, evaluator_id=profile.evaluator_id, evaluator_version=profile.evaluator_version)

    def normalize_evidence(self, evidence: Mapping[str, Any]) -> DomainEvaluationEvidence:
        normalized = DomainEvaluationEvidence.from_mapping(evidence)
        accepted_domains = {self.profile.domain, *self.profile.accepted_evidence_domains}
        if normalized.domain not in accepted_domains:
            raise BrainRunError(
                f"domain evaluator {self.profile.domain!r} cannot evaluate {normalized.domain!r} evidence"
            )
        return normalized

    def _evaluate(self, evaluation_input: Mapping[str, Any]) -> dict[str, Any]:
        raw_evidence = evaluation_input.get("evidence")
        if not isinstance(raw_evidence, Mapping):
            return {
                "reward": 0.0,
                "passed": False,
                "failed": True,
                "failure_class": "missing_domain_evidence",
                "replan_requested": True,
                "replan_instruction": f"Collect bounded {self.profile.domain} evaluation signals.",
            }
        evidence = self.normalize_evidence(raw_evidence)
        weighted_total = 0.0
        observed_weight = 0.0
        missing: list[str] = []
        below_threshold: list[str] = []
        for signal in self.profile.required_signals:
            value = evidence.signals.get(signal)
            if value is None:
                missing.append(signal)
                continue
            if value < self.profile.pass_threshold:
                below_threshold.append(signal)
        for signal, weight in self.profile.signal_weights.items():
            value = evidence.signals.get(signal)
            if value is None:
                continue
            weighted_total += float(value) * float(weight)
            observed_weight += float(weight)
        reward = 0.0 if observed_weight == 0 else weighted_total / observed_weight
        failed = bool(missing or below_threshold or reward < self.profile.pass_threshold)
        gaps = [*missing, *below_threshold]
        instruction = None
        if failed:
            detail = ", ".join(dict.fromkeys(gaps)) or "the weighted quality threshold"
            instruction = f"Address bounded {self.profile.domain} evaluation gaps: {detail}."
        return {
            "reward": reward,
            "passed": not failed,
            "failed": failed,
            "failure_class": None if not failed else "domain_evidence_gate",
            "feedback_digest": _digest(evidence.to_dict()),
            "replan_requested": failed,
            "replan_instruction": instruction,
        }

    def catalogue_entry(self) -> dict[str, Any]:
        return self.profile.to_dict()


class CompositeDomainEvaluator(BrainOutcomeEvaluator):
    """Route value-only decisions to domain-specific evaluators under one stable identity.

    Cross-domain trajectories must expose one evaluator identity to the learning ledger, but the
    quality rubric for a coding child should not silently become the rubric for a biomedical or
    operations child. This adapter keeps one outer identity for trajectory settlement and routes
    each value-only input using the reviewed selection context's domain. It never sees provider
    text, credentials, or raw tool envelopes.
    """

    def __init__(
        self,
        evaluators: Mapping[str, BrainOutcomeEvaluator],
        *,
        evaluator_id: str = "composite-domain-quality",
        evaluator_version: str = "1",
    ) -> None:
        if not isinstance(evaluators, Mapping) or not evaluators:
            raise BrainRunError("composite domain evaluators must be a non-empty mapping")
        normalized: dict[str, BrainOutcomeEvaluator] = {}
        for domain, evaluator in evaluators.items():
            if not isinstance(domain, str) or not domain.strip() or len(domain.encode("utf-8")) > MAX_DOMAIN_EVALUATOR_TEXT_BYTES:
                raise BrainRunError("composite domain evaluator keys must be bounded domain names")
            if not isinstance(evaluator, BrainOutcomeEvaluator):
                raise BrainRunError("composite domain evaluator values must be BrainOutcomeEvaluator instances")
            if domain in normalized:
                raise BrainRunError(f"duplicate composite domain evaluator: {domain}")
            normalized[domain] = evaluator
        self.evaluators = normalized
        super().__init__(self._evaluate, evaluator_id=evaluator_id, evaluator_version=evaluator_version)

    def _resolve_domain(self, evaluation_input: Mapping[str, Any]) -> str:
        context = evaluation_input.get("context")
        domain = context.get("domain") if isinstance(context, Mapping) else None
        evidence = evaluation_input.get("evidence")
        evidence_domain = evidence.get("domain") if isinstance(evidence, Mapping) else None
        if domain is None:
            domain = evidence_domain
        if not isinstance(domain, str) or not domain.strip():
            raise BrainRunError("composite domain evaluation requires an explicit domain context")
        if evidence_domain is not None and not isinstance(evidence_domain, str):
            raise BrainRunError("composite domain evidence domain must be a string when supplied")
        if domain not in self.evaluators:
            raise BrainRunError(f"no composite evaluator is registered for {domain!r}")
        return domain

    def _evaluate(self, evaluation_input: Mapping[str, Any]) -> dict[str, Any]:
        try:
            domain = self._resolve_domain(evaluation_input)
        except BrainRunError:
            return {
                "reward": 0.0,
                "passed": False,
                "failed": True,
                "failure_class": "unmapped_domain_evaluator",
                "replan_requested": True,
                "replan_instruction": "Provide an explicit reviewed evaluator for the routed domain.",
            }
        decision = self.evaluators[domain].assess_value_only_input(evaluation_input)
        return {
            "reward": decision.reward,
            "passed": decision.passed,
            "failed": decision.failed,
            "feedback_digest": decision.feedback_digest,
            "failure_class": decision.failure_class,
            "evidence_digest": decision.evidence_digest,
            "replan_requested": decision.replan_requested,
            "replan_instruction": decision.replan_instruction,
        }

    def catalogue_entry(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_EVALUATOR_SCHEMA,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "domains": [
                {
                    "domain": domain,
                    "evaluator_id": evaluator.evaluator_id,
                    "evaluator_version": evaluator.evaluator_version,
                }
                for domain, evaluator in sorted(self.evaluators.items())
            ],
            "execution": "value_only_domain_routing",
            "retention": "evaluator_id_version_and_domain_keys_only",
        }

    @classmethod
    def from_registry(
        cls,
        registry: "DomainEvaluatorRegistry",
        *,
        domains: Sequence[str],
        evaluator_id: str = "composite-domain-quality",
        evaluator_version: str = "1",
    ) -> "CompositeDomainEvaluator":
        if not isinstance(registry, DomainEvaluatorRegistry):
            raise BrainRunError("composite evaluator registry must be a DomainEvaluatorRegistry")
        if not isinstance(domains, Sequence) or isinstance(domains, (str, bytes)) or not domains:
            raise BrainRunError("composite evaluator domains must be a non-empty sequence")
        selected: dict[str, BrainOutcomeEvaluator] = {}
        for domain in domains:
            if not isinstance(domain, str) or not domain.strip():
                raise BrainRunError("composite evaluator domains must contain non-empty strings")
            selected[domain] = registry.resolve_for_autonomous_domain(domain)
        return cls(selected, evaluator_id=evaluator_id, evaluator_version=evaluator_version)


class DomainEvaluatorRegistry:
    """Deterministic registry of domain evaluator adapters."""

    def __init__(self, adapters: Sequence[DomainEvaluatorAdapter] = ()) -> None:
        self._adapters: dict[str, DomainEvaluatorAdapter] = {}
        self._autonomous_adapters: dict[str, DomainEvaluatorAdapter] = {}
        for adapter in adapters:
            self.register(adapter)

    def register(self, adapter: DomainEvaluatorAdapter) -> None:
        if not isinstance(adapter, DomainEvaluatorAdapter):
            raise BrainRunError("registry entries must be DomainEvaluatorAdapter values")
        domain = adapter.profile.domain
        if domain in self._adapters:
            raise BrainRunError(f"domain evaluator is already registered: {domain}")
        self._adapters[domain] = adapter

    def register_autonomous(self, adapter: DomainEvaluatorAdapter) -> None:
        """Register an exact-domain adapter without shadowing a canonical evaluator key."""

        if not isinstance(adapter, DomainEvaluatorAdapter):
            raise BrainRunError("autonomous registry entries must be DomainEvaluatorAdapter values")
        domain = adapter.profile.domain
        if domain in self._autonomous_adapters:
            raise BrainRunError(f"autonomous domain evaluator is already registered: {domain}")
        self._autonomous_adapters[domain] = adapter

    def resolve(self, domain: str) -> DomainEvaluatorAdapter:
        _text("evaluator registry domain", domain)
        adapter = self._adapters.get(domain)
        if adapter is None:
            adapter = self._autonomous_adapters.get(domain)
        if adapter is None:
            raise BrainRunError(f"no domain evaluator is registered for {domain!r}")
        return adapter

    def resolve_for_autonomous_domain(
        self,
        domain: str,
        *,
        fallback_domain: str | None = None,
    ) -> DomainEvaluatorAdapter:
        """Prefer an exact autonomous-domain evaluator, with an explicit legacy fallback."""

        _text("autonomous evaluator domain", domain)
        exact = self._autonomous_adapters.get(domain) or self._adapters.get(domain)
        if exact is not None:
            return exact
        if fallback_domain is not None:
            return self.resolve(fallback_domain)
        raise BrainRunError(f"no evaluator is registered for autonomous domain {domain!r}")

    def catalogue(self) -> list[dict[str, Any]]:
        adapters = [*self._adapters.values(), *self._autonomous_adapters.values()]
        return [
            adapter.catalogue_entry()
            for adapter in sorted(adapters, key=lambda item: (item.profile.domain, item.profile.evaluator_id))
        ]

    def resolve_for_replay(
        self,
        domain: str,
        *,
        evaluator_id: str,
        evaluator_version: str,
    ) -> DomainEvaluatorAdapter:
        """Select the canonical or exact adapter that matches a replay case identity."""

        primary = self.resolve(domain)
        if primary.evaluator_id == evaluator_id and primary.evaluator_version == evaluator_version:
            return primary
        exact = self._autonomous_adapters.get(domain)
        if exact is not None and exact.evaluator_id == evaluator_id and exact.evaluator_version == evaluator_version:
            return exact
        return primary

    @classmethod
    def with_builtin_profiles(cls) -> "DomainEvaluatorRegistry":
        registry = cls()
        for profile in builtin_domain_profiles():
            registry.register(DomainEvaluatorAdapter(profile))
        return registry

    @classmethod
    def with_builtin_autonomous_profiles(cls) -> "DomainEvaluatorRegistry":
        """Return specialized profiles for all autonomous domains plus non-overlapping legacy profiles."""

        registry = cls.with_builtin_profiles()
        for profile in builtin_autonomous_domain_evaluator_profiles():
            registry.register_autonomous(DomainEvaluatorAdapter(profile))
        return registry


def builtin_domain_profiles() -> tuple[DomainEvaluatorProfile, ...]:
    """Return conservative profiles spanning the major catalogued application domains."""

    return (
        DomainEvaluatorProfile(
            domain="engineering",
            evaluator_id="domain-engineering-quality",
            evaluator_version="1",
            required_signals=("schema_valid", "tests_passed", "evidence_complete"),
            signal_weights={"schema_valid": 1.0, "tests_passed": 2.0, "evidence_complete": 1.0},
            accepted_evidence_domains=("coding", "multi_agent", "evaluation"),
        ),
        DomainEvaluatorProfile(
            domain="research",
            evaluator_id="domain-research-quality",
            evaluator_version="1",
            required_signals=("evidence_traceable", "uncertainty_reported", "claim_scope_respected"),
            signal_weights={"evidence_traceable": 2.0, "uncertainty_reported": 1.0, "claim_scope_respected": 2.0},
            accepted_evidence_domains=("browser", "science", "multimodal", "cross_domain"),
        ),
        DomainEvaluatorProfile(
            domain="operations",
            evaluator_id="domain-operations-quality",
            evaluator_version="1",
            required_signals=("safety_gate_passed", "approval_complete", "rollback_plan_present"),
            signal_weights={"safety_gate_passed": 3.0, "approval_complete": 2.0, "rollback_plan_present": 1.0},
            accepted_evidence_domains=("enterprise",),
        ),
        DomainEvaluatorProfile(
            domain="data",
            evaluator_id="domain-data-quality",
            evaluator_version="1",
            required_signals=("schema_valid", "lineage_complete", "quality_gate_passed"),
            signal_weights={"schema_valid": 1.0, "lineage_complete": 2.0, "quality_gate_passed": 2.0},
        ),
        DomainEvaluatorProfile(
            domain="biomedical",
            evaluator_id="domain-biomedical-boundary",
            evaluator_version="1",
            required_signals=("boundary_compliant", "provenance_complete", "human_review_ready"),
            signal_weights={"boundary_compliant": 3.0, "provenance_complete": 2.0, "human_review_ready": 2.0},
            accepted_evidence_domains=("neuroscience",),
        ),
    )


def builtin_autonomous_domain_evaluator_profiles() -> tuple[DomainEvaluatorProfile, ...]:
    """Return specialized, conservative evaluator contracts for all twelve autonomous domains."""

    return (
        DomainEvaluatorProfile(
            domain="coding",
            evaluator_id="autonomous-coding-quality",
            evaluator_version="1",
            required_signals=("schema_valid", "tests_passed", "evidence_complete"),
            signal_weights={"schema_valid": 1.0, "tests_passed": 2.0, "evidence_complete": 1.0},
            accepted_evidence_domains=("engineering",),
        ),
        DomainEvaluatorProfile(
            domain="browser",
            evaluator_id="autonomous-browser-quality",
            evaluator_version="1",
            required_signals=("evidence_traceable", "source_comparison", "freshness_reported", "claim_scope_respected"),
            signal_weights={"evidence_traceable": 2.0, "source_comparison": 1.0, "freshness_reported": 1.0, "claim_scope_respected": 2.0},
            accepted_evidence_domains=("research",),
        ),
        DomainEvaluatorProfile(
            domain="data",
            evaluator_id="autonomous-data-quality",
            evaluator_version="1",
            required_signals=("schema_valid", "lineage_complete", "quality_gate_passed"),
            signal_weights={"schema_valid": 1.0, "lineage_complete": 2.0, "quality_gate_passed": 2.0},
        ),
        DomainEvaluatorProfile(
            domain="science",
            evaluator_id="autonomous-science-quality",
            evaluator_version="1",
            required_signals=("evidence_traceable", "uncertainty_reported", "claim_scope_respected", "reproducible"),
            signal_weights={"evidence_traceable": 2.0, "uncertainty_reported": 1.0, "claim_scope_respected": 2.0, "reproducible": 1.0},
            accepted_evidence_domains=("research",),
        ),
        DomainEvaluatorProfile(
            domain="biomedical",
            evaluator_id="autonomous-biomedical-boundary",
            evaluator_version="1",
            required_signals=("boundary_compliant", "provenance_complete", "human_review_ready"),
            signal_weights={"boundary_compliant": 3.0, "provenance_complete": 2.0, "human_review_ready": 2.0},
        ),
        DomainEvaluatorProfile(
            domain="neuroscience",
            evaluator_id="autonomous-neuroscience-quality",
            evaluator_version="1",
            required_signals=("signal_quality_reported", "preprocessing_traceable", "claim_scope_respected", "reproducible"),
            signal_weights={"signal_quality_reported": 2.0, "preprocessing_traceable": 2.0, "claim_scope_respected": 2.0, "reproducible": 1.0},
            accepted_evidence_domains=("biomedical",),
        ),
        DomainEvaluatorProfile(
            domain="operations",
            evaluator_id="autonomous-operations-quality",
            evaluator_version="1",
            required_signals=("safety_gate_passed", "approval_complete", "rollback_plan_present"),
            signal_weights={"safety_gate_passed": 3.0, "approval_complete": 2.0, "rollback_plan_present": 2.0, "observability_ready": 1.0},
            accepted_evidence_domains=(),
        ),
        DomainEvaluatorProfile(
            domain="enterprise",
            evaluator_id="autonomous-enterprise-quality",
            evaluator_version="1",
            required_signals=("ownership_complete", "policy_aligned", "approval_complete", "decision_traceable"),
            signal_weights={"ownership_complete": 2.0, "policy_aligned": 2.0, "approval_complete": 2.0, "decision_traceable": 1.0},
            accepted_evidence_domains=("operations",),
        ),
        DomainEvaluatorProfile(
            domain="multi_agent",
            evaluator_id="autonomous-multi-agent-quality",
            evaluator_version="1",
            required_signals=("contract_complete", "attribution_complete", "conflict_resolved", "approval_complete"),
            signal_weights={"contract_complete": 2.0, "attribution_complete": 2.0, "conflict_resolved": 2.0, "approval_complete": 1.0},
            accepted_evidence_domains=("engineering",),
        ),
        DomainEvaluatorProfile(
            domain="multimodal",
            evaluator_id="autonomous-multimodal-quality",
            evaluator_version="1",
            required_signals=("modality_inventory_complete", "alignment_valid", "uncertainty_reported", "claim_scope_respected"),
            signal_weights={"modality_inventory_complete": 2.0, "alignment_valid": 2.0, "uncertainty_reported": 1.0, "claim_scope_respected": 2.0},
            accepted_evidence_domains=("research",),
        ),
        DomainEvaluatorProfile(
            domain="cross_domain",
            evaluator_id="autonomous-cross-domain-quality",
            evaluator_version="1",
            required_signals=("route_traceable", "evidence_aligned", "attribution_complete", "uncertainty_reported"),
            signal_weights={"route_traceable": 1.0, "evidence_aligned": 2.0, "attribution_complete": 2.0, "uncertainty_reported": 1.0},
            accepted_evidence_domains=("research",),
        ),
        DomainEvaluatorProfile(
            domain="evaluation",
            evaluator_id="autonomous-evaluation-quality",
            evaluator_version="1",
            required_signals=("rubric_frozen", "replay_reproducible", "evaluator_independent", "evidence_complete"),
            signal_weights={"rubric_frozen": 2.0, "replay_reproducible": 2.0, "evaluator_independent": 2.0, "evidence_complete": 1.0},
            accepted_evidence_domains=("engineering",),
        ),
    )


__all__ = [
    "DOMAIN_EVALUATOR_SCHEMA",
    "DomainEvaluationEvidence",
    "DomainEvaluatorAdapter",
    "CompositeDomainEvaluator",
    "DomainEvaluatorProfile",
    "DomainEvaluatorRegistry",
    "builtin_domain_profiles",
    "builtin_autonomous_domain_evaluator_profiles",
]
