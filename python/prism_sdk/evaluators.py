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

from .brain import BrainEvaluatorDecision, BrainOutcomeEvaluator, BrainRunError
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
        if not math.isfinite(total) or total <= 0:
            raise BrainRunError("domain evaluator signal weights must have a finite positive sum")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_EVALUATOR_SCHEMA,
            "domain": self.domain,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "required_signals": list(self.required_signals),
            "signal_weights": dict(self.signal_weights),
            "pass_threshold": self.pass_threshold,
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

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "DomainEvaluationEvidence":
        if not isinstance(value, Mapping):
            raise BrainRunError("domain evaluation evidence must be a mapping")
        allowed = {"schema", "domain", "capability", "risk_class", "signals", "references", "limitations", "retention"}
        if any(not isinstance(key, str) for key in value) or set(value).difference(allowed):
            raise BrainRunError("domain evaluation evidence contains unsupported fields")
        raw_signals = value.get("signals")
        if not isinstance(raw_signals, Mapping):
            raise BrainRunError("domain evaluation evidence signals must be a mapping")
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
        if normalized.domain != self.profile.domain:
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


class DomainEvaluatorRegistry:
    """Deterministic registry of domain evaluator adapters."""

    def __init__(self, adapters: Sequence[DomainEvaluatorAdapter] = ()) -> None:
        self._adapters: dict[str, DomainEvaluatorAdapter] = {}
        for adapter in adapters:
            self.register(adapter)

    def register(self, adapter: DomainEvaluatorAdapter) -> None:
        if not isinstance(adapter, DomainEvaluatorAdapter):
            raise BrainRunError("registry entries must be DomainEvaluatorAdapter values")
        domain = adapter.profile.domain
        if domain in self._adapters:
            raise BrainRunError(f"domain evaluator is already registered: {domain}")
        self._adapters[domain] = adapter

    def resolve(self, domain: str) -> DomainEvaluatorAdapter:
        _text("evaluator registry domain", domain)
        adapter = self._adapters.get(domain)
        if adapter is None:
            raise BrainRunError(f"no domain evaluator is registered for {domain!r}")
        return adapter

    def catalogue(self) -> list[dict[str, Any]]:
        return [self._adapters[key].catalogue_entry() for key in sorted(self._adapters)]

    @classmethod
    def with_builtin_profiles(cls) -> "DomainEvaluatorRegistry":
        registry = cls()
        for profile in builtin_domain_profiles():
            registry.register(DomainEvaluatorAdapter(profile))
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
        ),
        DomainEvaluatorProfile(
            domain="research",
            evaluator_id="domain-research-quality",
            evaluator_version="1",
            required_signals=("evidence_traceable", "uncertainty_reported", "claim_scope_respected"),
            signal_weights={"evidence_traceable": 2.0, "uncertainty_reported": 1.0, "claim_scope_respected": 2.0},
        ),
        DomainEvaluatorProfile(
            domain="operations",
            evaluator_id="domain-operations-quality",
            evaluator_version="1",
            required_signals=("safety_gate_passed", "approval_complete", "rollback_plan_present"),
            signal_weights={"safety_gate_passed": 3.0, "approval_complete": 2.0, "rollback_plan_present": 1.0},
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
        ),
    )


__all__ = [
    "DOMAIN_EVALUATOR_SCHEMA",
    "DomainEvaluationEvidence",
    "DomainEvaluatorAdapter",
    "DomainEvaluatorProfile",
    "DomainEvaluatorRegistry",
    "builtin_domain_profiles",
]
