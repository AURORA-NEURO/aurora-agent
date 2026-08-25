"""Provider-free capability routing inside each autonomous domain.

Domain routing identifies the discipline that should own a task.  Capability routing narrows
that decision to a reviewed workflow capability so prompt construction, model selection, and
tool coverage can use a more useful context than a domain's generic default.  The router is
deliberately lexical and abstaining: it never calls a provider, discovers a tool, accepts a
credential, or authorizes an effect.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
import re
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_CAPABILITY_ROUTE_SCHEMA = "bioprism-autonomous-capability-route/0.1"
AUTONOMOUS_CAPABILITY_ROUTE_SOURCE = "deterministic_capability_vocabulary"
AUTONOMOUS_CAPABILITY_ROUTE_REASONS = (
    "selected",
    "explicit_capability",
    "no_matching_capability",
    "insufficient_confidence",
    "insufficient_margin",
)
MAX_AUTONOMOUS_CAPABILITY_ROUTE_CANDIDATES = 32
MAX_AUTONOMOUS_CAPABILITY_ROUTE_MATCHED_TERMS = 16
_RETENTION = "task_text_transient_only; capability_scores_and_digests_only"
_AUTHORIZATION = "classification_only; no_provider_tool_or_effect_authority"


_VOCABULARY: dict[str, tuple[tuple[str, tuple[str, ...]], ...]] = {
    "coding": (
        ("review", ("review", "audit", "inspect", "release", "pull request", "pr")),
        ("debugging", ("debug", "bug", "failure", "failing", "error", "stack trace")),
        ("implementation", ("implement", "build", "code", "feature", "develop", "write code")),
        ("testing", ("test", "tests", "testing", "ci", "regression", "coverage")),
    ),
    "browser": (
        ("web_research", ("research", "search", "look up", "find sources", "web", "citation")),
        ("navigation", ("navigate", "browse", "open page", "click", "workspace", "route")),
        ("source_comparison", ("compare sources", "cross-source", "contrast", "fact check", "verify sources")),
    ),
    "data": (
        ("data_analysis", ("analyze data", "analysis", "aggregate", "statistics", "query", "visualize")),
        ("schema_validation", ("schema", "validate schema", "columns", "types", "data contract")),
        ("lineage", ("lineage", "provenance", "transform", "trace data", "data flow")),
        ("quality_control", ("quality", "missingness", "outlier", "duplicate", "quality control", "clean data")),
    ),
    "science": (
        ("literature", ("literature", "paper", "publication", "study", "references")),
        ("hypothesis", ("hypothesis", "hypotheses", "mechanism", "causal question")),
        ("experiment", ("experiment", "experimental design", "protocol", "assay")),
        ("statistics", ("statistics", "statistical", "p value", "confidence interval", "regression")),
        ("reproducibility", ("reproduce", "reproduction", "replicate", "replication", "reproducibility")),
    ),
    "biomedical": (
        ("biomedical_review", ("biomedical", "clinical evidence", "medical literature", "biomarker")),
        ("provenance", ("provenance", "reference", "population", "endpoint")),
        ("safety_boundary", ("safety", "risk", "ethics", "dual use", "medical boundary")),
        ("human_review", ("human review", "clinician", "clinical review", "subject", "informed consent")),
    ),
    "neuroscience": (
        ("neuroscience_analysis", ("neuroscience", "neural", "brain", "neural data")),
        ("signal_interpretation", ("signal", "neural signal", "spike", "eeg", "fmri", "interpret")),
        ("study_design", ("study design", "experiment design", "cohort", "trial")),
        ("reproducibility", ("reproduce", "replicate", "benchmark", "trace")),
    ),
    "operations": (
        ("observability", ("observe", "monitor", "telemetry", "metrics", "logs", "status")),
        ("incident_response", ("incident", "outage", "alert", "on call", "triage")),
        ("risk_review", ("risk", "readiness", "review change", "approval")),
        ("rollback", ("rollback", "roll back", "restore", "revert", "undo")),
        ("approval", ("approve", "authorization", "authorize", "change request")),
        ("runbook", ("runbook", "playbook", "procedure", "plan")),
    ),
    "enterprise": (
        ("workflow", ("workflow", "process", "business process", "coordinate")),
        ("governance", ("governance", "owner", "ownership", "policy")),
        ("compliance", ("compliance", "audit", "control", "regulation")),
        ("analytics", ("analytics", "dashboard", "kpi", "report")),
        ("coordination", ("coordinate", "stakeholder", "handoff", "meeting")),
    ),
    "multi_agent": (
        ("delegation", ("delegate", "delegation", "assign", "subtask")),
        ("coordination", ("coordinate", "orchestrate", "multi agent", "agents")),
        ("consensus", ("consensus", "vote", "agreement", "dissent")),
        ("conflict_resolution", ("conflict", "disagreement", "resolve conflict")),
        ("handoff", ("handoff", "handover", "transfer", "agent result")),
    ),
    "multimodal": (
        ("image", ("image", "photo", "visual", "vision")),
        ("audio", ("audio", "sound", "speech", "recording")),
        ("video", ("video", "frame", "temporal")),
        ("document", ("document", "pdf", "text extraction", "ocr")),
        ("cross_modal_alignment", ("align modalities", "cross modal", "multimodal", "fusion", "synchronize")),
    ),
    "cross_domain": (
        ("routing", ("route", "routing", "which domain", "assign domain")),
        ("synthesis", ("synthesize", "combine", "integrate", "summary")),
        ("evidence_alignment", ("evidence", "align evidence", "provenance", "compare findings")),
        ("workflow_composition", ("workflow", "compose workflow", "pipeline", "dependency")),
    ),
    "evaluation": (
        ("benchmarking", ("benchmark", "benchmarking", "compare models", "performance")),
        ("rubric", ("rubric", "criteria", "score", "grading")),
        ("replay", ("replay", "re-run", "reproduce trace", "deterministic")),
        ("failure_analysis", ("failure", "error analysis", "regression", "root cause")),
        ("reproducibility", ("reproducibility", "replicate", "exact replay", "repeat")),
    ),
}


def _bounded_text(name: str, value: Any, maximum: int = 32_000) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value


def _identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_.:-]{1,256}", value):
        raise ArgumentError(f"{name} is outside its identifier contract")
    return value


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _unit(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not 0 <= float(value) <= 1:
        raise ArgumentError(f"{name} must be within [0, 1]")
    return float(value)


def _canonical_number(value: float) -> int | float:
    """Match JavaScript/Rust canonical JSON for bounded routing probabilities."""

    return int(value) if value.is_integer() else value


def _normalize(value: str) -> str:
    return " ".join(re.sub(r"[^a-z0-9]+", " ", value.lower()).split())


def _matches(normalized: str, term: str) -> bool:
    needle = _normalize(term)
    return bool(needle) and f" {needle} " in f" {normalized} "


def _score(terms: Sequence[str], normalized: str) -> tuple[float, tuple[str, ...]]:
    matched = tuple(sorted(term for term in terms if _matches(normalized, term)))
    points = sum(2 if len(_normalize(term)) >= 6 or " " in term else 1 for term in matched)
    return round(min(1.0, points / 4.0), 12), matched


@dataclass(frozen=True, slots=True)
class AutonomousCapabilityRouteCandidate:
    domain: str
    capability: str
    score: float
    matched_terms: tuple[str, ...]

    def __post_init__(self) -> None:
        _identifier("capability route candidate domain", self.domain)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("capability route candidate domain is unsupported")
        _identifier("capability route candidate capability", self.capability)
        _unit("capability route candidate score", self.score)
        if not isinstance(self.matched_terms, Sequence) or isinstance(self.matched_terms, (str, bytes)) or len(self.matched_terms) > MAX_AUTONOMOUS_CAPABILITY_ROUTE_MATCHED_TERMS:
            raise ArgumentError("capability route candidate matched_terms exceed their bound")
        terms = tuple(_bounded_text("capability route matched term", term, maximum=256) for term in self.matched_terms)
        if len(set(terms)) != len(terms) or tuple(sorted(terms)) != terms:
            raise ArgumentError("capability route candidate matched_terms must be unique and sorted")
        object.__setattr__(self, "score", float(self.score))
        object.__setattr__(self, "matched_terms", terms)

    def to_dict(self) -> dict[str, Any]:
        return {"domain": self.domain, "capability": self.capability, "score": _canonical_number(self.score), "matched_terms": list(self.matched_terms)}


@dataclass(frozen=True, slots=True)
class AutonomousCapabilityRoute:
    task_digest: str
    domain: str
    candidates: tuple[AutonomousCapabilityRouteCandidate, ...]
    selected_capability: str | None
    confidence: float
    abstained: bool
    reason: str
    source: str = AUTONOMOUS_CAPABILITY_ROUTE_SOURCE

    def __post_init__(self) -> None:
        _digest("capability route task_digest", self.task_digest)
        _identifier("capability route domain", self.domain)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("capability route domain is unsupported")
        if not isinstance(self.candidates, Sequence) or isinstance(self.candidates, (str, bytes)) or len(self.candidates) > MAX_AUTONOMOUS_CAPABILITY_ROUTE_CANDIDATES:
            raise ArgumentError("capability route candidates exceed their bound")
        candidates = tuple(self.candidates)
        if any(not isinstance(candidate, AutonomousCapabilityRouteCandidate) for candidate in candidates):
            raise ArgumentError("capability route candidates are malformed")
        if any(candidate.domain != self.domain for candidate in candidates) or len({candidate.capability for candidate in candidates}) != len(candidates):
            raise ArgumentError("capability route candidate identity is invalid")
        if self.selected_capability is not None:
            _identifier("capability route selected_capability", self.selected_capability)
            if self.selected_capability not in {candidate.capability for candidate in candidates}:
                raise ArgumentError("selected capability is not present in candidates")
        _unit("capability route confidence", self.confidence)
        if not isinstance(self.abstained, bool) or self.reason not in AUTONOMOUS_CAPABILITY_ROUTE_REASONS or self.source != AUTONOMOUS_CAPABILITY_ROUTE_SOURCE:
            raise ArgumentError("capability route decision is invalid")
        if self.abstained and self.selected_capability is not None:
            raise ArgumentError("abstained capability route cannot select a capability")
        if not self.abstained and self.selected_capability is None:
            raise ArgumentError("selected capability route must select a capability")
        object.__setattr__(self, "candidates", candidates)
        object.__setattr__(self, "confidence", float(self.confidence))

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CAPABILITY_ROUTE_SCHEMA,
            "task_digest": self.task_digest,
            "domain": self.domain,
            "candidates": [candidate.to_dict() for candidate in self.candidates],
            "selected_capability": self.selected_capability,
            "confidence": _canonical_number(self.confidence),
            "abstained": self.abstained,
            "reason": self.reason,
            "source": self.source,
            "retention": _RETENTION,
            "authorization": _AUTHORIZATION,
            "secret_material": "never_returned",
        }

    @property
    def route_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "route_digest": self.route_digest}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousCapabilityRoute":
        if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_CAPABILITY_ROUTE_SCHEMA:
            raise ArgumentError("capability route schema is invalid")
        allowed = {"schema", "task_digest", "domain", "candidates", "selected_capability", "confidence", "abstained", "reason", "source", "route_digest", "retention", "authorization", "secret_material"}
        if set(value).difference(allowed):
            raise ArgumentError("capability route contains unsupported fields")
        if value.get("source") != AUTONOMOUS_CAPABILITY_ROUTE_SOURCE or value.get("retention") != _RETENTION or value.get("authorization") != _AUTHORIZATION or value.get("secret_material") != "never_returned":
            raise ArgumentError("capability route markers are invalid")
        raw_candidates = value.get("candidates")
        if not isinstance(raw_candidates, Sequence) or isinstance(raw_candidates, (str, bytes)):
            raise ArgumentError("capability route candidates are invalid")
        candidates = tuple(AutonomousCapabilityRouteCandidate(domain=row.get("domain"), capability=row.get("capability"), score=row.get("score"), matched_terms=tuple(row.get("matched_terms", ()))) for row in raw_candidates if isinstance(row, Mapping))
        if len(candidates) != len(raw_candidates):
            raise ArgumentError("capability route candidates are malformed")
        route = cls(task_digest=value.get("task_digest"), domain=value.get("domain"), candidates=candidates, selected_capability=value.get("selected_capability"), confidence=value.get("confidence"), abstained=value.get("abstained"), reason=value.get("reason"), source=value.get("source"))
        if value.get("route_digest") != route.route_digest:
            raise ArgumentError("capability route digest does not match its metadata")
        return route


def autonomous_capability_vocabulary(domain: str) -> tuple[str, ...]:
    _identifier("autonomous capability domain", domain)
    if domain not in _VOCABULARY:
        raise ArgumentError(f"unsupported autonomous capability domain: {domain}")
    return tuple(capability for capability, _ in _VOCABULARY[domain])


def route_autonomous_capability(
    task: str,
    domain: str,
    *,
    explicit_capability: str | None = None,
    min_confidence: float = 0.25,
    min_margin: float = 0.10,
) -> AutonomousCapabilityRoute:
    task_text = _bounded_text("autonomous capability route task", task)
    _identifier("autonomous capability route domain", domain)
    if domain not in _VOCABULARY:
        raise ArgumentError(f"unsupported autonomous capability domain: {domain}")
    min_confidence = _unit("autonomous capability route min_confidence", min_confidence)
    min_margin = _unit("autonomous capability route min_margin", min_margin)
    task_digest = content_digest({"task": task_text})
    if explicit_capability is not None:
        capability = _identifier("autonomous capability route explicit capability", explicit_capability)
        return AutonomousCapabilityRoute(task_digest, domain, (AutonomousCapabilityRouteCandidate(domain, capability, 1.0, ("caller_explicit_capability",)),), capability, 1.0, False, "explicit_capability")
    normalized = _normalize(task_text)
    candidates = []
    for capability, terms in _VOCABULARY[domain]:
        score, matched = _score(terms, normalized)
        if score > 0:
            candidates.append(AutonomousCapabilityRouteCandidate(domain, capability, score, matched))
    candidates.sort(key=lambda candidate: (-candidate.score, candidate.capability))
    candidates = tuple(candidates[:MAX_AUTONOMOUS_CAPABILITY_ROUTE_CANDIDATES])
    top = candidates[0] if candidates else None
    base_reason = "no_matching_capability" if top is None else "selected"
    if top is None:
        return AutonomousCapabilityRoute(task_digest, domain, candidates, None, 0.0, True, base_reason)
    if top.score < min_confidence:
        return AutonomousCapabilityRoute(task_digest, domain, candidates, None, top.score, True, "insufficient_confidence")
    if len(candidates) > 1 and top.score - candidates[1].score < min_margin:
        return AutonomousCapabilityRoute(task_digest, domain, candidates, None, top.score, True, "insufficient_margin")
    return AutonomousCapabilityRoute(task_digest, domain, candidates, top.capability, top.score, False, "selected")


def validate_autonomous_capability_route(task: str, value: Mapping[str, Any] | AutonomousCapabilityRoute) -> AutonomousCapabilityRoute:
    route = value if isinstance(value, AutonomousCapabilityRoute) else AutonomousCapabilityRoute.from_dict(value)
    task_digest = content_digest({"task": _bounded_text("autonomous capability route task", task)})
    if route.task_digest != task_digest:
        raise ArgumentError("autonomous capability route does not match the task digest")
    return route


__all__ = [
    "AUTONOMOUS_CAPABILITY_ROUTE_SCHEMA",
    "AUTONOMOUS_CAPABILITY_ROUTE_SOURCE",
    "AUTONOMOUS_CAPABILITY_ROUTE_REASONS",
    "MAX_AUTONOMOUS_CAPABILITY_ROUTE_CANDIDATES",
    "MAX_AUTONOMOUS_CAPABILITY_ROUTE_MATCHED_TERMS",
    "AutonomousCapabilityRouteCandidate",
    "AutonomousCapabilityRoute",
    "autonomous_capability_vocabulary",
    "route_autonomous_capability",
    "validate_autonomous_capability_route",
]
