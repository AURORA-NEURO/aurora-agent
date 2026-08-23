"""Bounded, provider-free intent extraction for autonomous task intake.

The intent contract is deliberately a classification projection, not a second source of
authority.  It makes the planner's interpretation observable before provider selection while
keeping task text transient and retaining only digests, bounded labels, and review signals.
The TypeScript SDK mirrors the same descriptor and canonical digest rules.
"""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .autonomous_task_lens import AutonomousDomainTaskLens, AUTONOMOUS_TASK_LENS_DOMAINS
from .errors import ArgumentError


AUTONOMOUS_TASK_INTENT_SCHEMA = "bioprism-autonomous-task-intent/0.1"
AUTONOMOUS_TASK_INTENT_VERSION = "0.1"
AUTONOMOUS_TASK_INTENT_ACTION_MODES = (
    "observe",
    "investigate",
    "analyze",
    "create",
    "modify",
    "compare",
    "plan",
    "coordinate",
    "evaluate",
    "synthesize",
)
AUTONOMOUS_TASK_INTENT_EFFECTS = ("none", "local_change", "external_effect")
AUTONOMOUS_TASK_INTENT_EVIDENCE_MODES = (
    "repository_and_test_evidence",
    "source_and_provenance_evidence",
    "schema_and_lineage_evidence",
    "method_and_reproduction_evidence",
    "grounding_and_safety_evidence",
    "modality_and_measurement_evidence",
    "telemetry_and_postcondition_evidence",
    "policy_and_audit_evidence",
    "handoff_and_dissent_evidence",
    "modality_and_transport_evidence",
    "specialist_and_synthesis_evidence",
    "holdout_and_replay_evidence",
)
MAX_AUTONOMOUS_TASK_INTENT_ITEMS = 8
MAX_AUTONOMOUS_TASK_INTENT_TEXT_BYTES = 512


_ACTION_CUES: Mapping[str, tuple[str, ...]] = {
    "observe": ("observe", "monitor", "inspect", "status", "check", "inventory"),
    "investigate": ("research", "find", "discover", "look up", "investigate", "review", "understand", "explain"),
    "analyze": ("analyze", "analyse", "assess", "measure", "quantify", "validate", "diagnose", "profile"),
    "create": ("create", "draft", "write", "generate", "design", "build", "implement", "develop"),
    "modify": ("fix", "debug", "refactor", "update", "change", "migrate", "patch", "remove", "delete"),
    "compare": ("compare", "contrast", "benchmark", "rank", "choose", "select", "versus", "vs"),
    "plan": ("plan", "schedule", "roadmap", "strategy", "rollout", "prepare", "runbook"),
    "coordinate": ("delegate", "coordinate", "assign", "handoff", "orchestrate", "manage", "approve"),
    "evaluate": ("evaluate", "test", "verify", "audit", "score", "grade", "replay", "red team"),
    "synthesize": ("synthesize", "synthesise", "combine", "integrate", "summarize", "summarise", "reconcile", "merge"),
}
_DEFAULT_ACTIONS = {
    "coding": "modify",
    "browser": "investigate",
    "data": "analyze",
    "science": "investigate",
    "biomedical": "investigate",
    "neuroscience": "analyze",
    "operations": "observe",
    "enterprise": "plan",
    "multi_agent": "coordinate",
    "multimodal": "analyze",
    "cross_domain": "synthesize",
    "evaluation": "evaluate",
}
_EVIDENCE_MODES = {
    "coding": "repository_and_test_evidence",
    "browser": "source_and_provenance_evidence",
    "data": "schema_and_lineage_evidence",
    "science": "method_and_reproduction_evidence",
    "biomedical": "grounding_and_safety_evidence",
    "neuroscience": "modality_and_measurement_evidence",
    "operations": "telemetry_and_postcondition_evidence",
    "enterprise": "policy_and_audit_evidence",
    "multi_agent": "handoff_and_dissent_evidence",
    "multimodal": "modality_and_transport_evidence",
    "cross_domain": "specialist_and_synthesis_evidence",
    "evaluation": "holdout_and_replay_evidence",
}
_EXTERNAL_EFFECT_CUES = (
    "deploy", "production", "publish", "send", "email", "purchase", "delete data",
    "restart service", "grant access", "execute command", "run command", "change live",
    "modify database", "provision", "roll back production",
)
_LOCAL_CHANGE_CUES = (
    "write code", "write a file", "implement", "patch", "refactor", "fix", "create a file",
    "update the repository", "change the code", "edit the document",
)
_CREDENTIAL_CUES = ("api key", "apikey", "token", "password", "secret", "credential", "private key")
_UNCERTAINTY_CUES = ("maybe", "possibly", "not sure", "unclear", "guess", "try to")


def _bounded_text(name: str, value: Any, *, maximum: int = MAX_AUTONOMOUS_TASK_INTENT_TEXT_BYTES) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its bound")
    return value


def _bounded_items(name: str, values: Sequence[str], *, maximum: int = MAX_AUTONOMOUS_TASK_INTENT_ITEMS) -> tuple[str, ...]:
    if not isinstance(values, Sequence) or isinstance(values, (str, bytes)) or len(values) > maximum:
        raise ArgumentError(f"{name} exceeds its item bound")
    result = tuple(_bounded_text(f"{name} item", value) for value in values)
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate items")
    return result


def _bounded_input_items(name: str, values: Sequence[str]) -> tuple[str, ...]:
    """Validate caller-owned task fields with the same contract as task intake."""

    if not isinstance(values, Sequence) or isinstance(values, (str, bytes)) or len(values) > 64:
        raise ArgumentError(f"{name} exceeds its item bound")
    result = tuple(_bounded_text(f"{name} item", value) for value in values)
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate items")
    return result


def _digest(value: Any, name: str) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _normalize(value: str) -> str:
    return " ".join(re.sub(r"[^a-z0-9]+", " ", value.lower()).split())


def _contains(normalized: str, phrase: str) -> bool:
    needle = _normalize(phrase)
    return bool(needle) and f" {needle} " in f" {normalized} "


def _unique(values: Sequence[str]) -> tuple[str, ...]:
    return tuple(dict.fromkeys(values))


@dataclass(frozen=True, slots=True)
class AutonomousTaskIntent:
    """A bounded interpretation of a task before provider selection or execution."""

    domain: str
    capability: str
    risk_class: str
    workflow_id: str
    task_digest: str
    lens_digest: str
    intent_id: str
    action_mode: str
    alternative_action_modes: tuple[str, ...]
    requested_effect: str
    evidence_mode: str
    ambiguity_flags: tuple[str, ...]
    planning_signals: tuple[str, ...]
    success_signals: tuple[str, ...]
    risk_signals: tuple[str, ...]
    requested_output_count: int
    desired_outputs_digest: str | None
    constraints_digest: str | None
    intent_version: str = AUTONOMOUS_TASK_INTENT_VERSION

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_TASK_INTENT_DOMAINS:
            raise ArgumentError(f"unsupported task-intent domain: {self.domain}")
        for name, value in (("capability", self.capability), ("risk_class", self.risk_class), ("workflow_id", self.workflow_id), ("intent_id", self.intent_id)):
            _bounded_text(f"task intent {name}", value, maximum=256)
        if self.intent_version != AUTONOMOUS_TASK_INTENT_VERSION:
            raise ArgumentError("unsupported task-intent version")
        _digest(self.task_digest, "task intent task_digest")
        _digest(self.lens_digest, "task intent lens_digest")
        if self.action_mode not in AUTONOMOUS_TASK_INTENT_ACTION_MODES:
            raise ArgumentError("task intent action_mode is unsupported")
        if self.requested_effect not in AUTONOMOUS_TASK_INTENT_EFFECTS:
            raise ArgumentError("task intent requested_effect is unsupported")
        if self.evidence_mode not in AUTONOMOUS_TASK_INTENT_EVIDENCE_MODES:
            raise ArgumentError("task intent evidence_mode is unsupported")
        for name, values in (("alternative_action_modes", self.alternative_action_modes), ("ambiguity_flags", self.ambiguity_flags), ("planning_signals", self.planning_signals), ("success_signals", self.success_signals), ("risk_signals", self.risk_signals)):
            object.__setattr__(self, name, _bounded_items(f"task intent {name}", values))
        if not isinstance(self.requested_output_count, int) or isinstance(self.requested_output_count, bool) or not 0 <= self.requested_output_count <= 64:
            raise ArgumentError("task intent requested_output_count is outside its bounds")
        for name, value in (("desired_outputs_digest", self.desired_outputs_digest), ("constraints_digest", self.constraints_digest)):
            if value is not None:
                _digest(value, f"task intent {name}")

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_TASK_INTENT_SCHEMA,
            "intent_version": self.intent_version,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "workflow_id": self.workflow_id,
            "task_digest": self.task_digest,
            "lens_digest": self.lens_digest,
            "intent_id": self.intent_id,
            "action_mode": self.action_mode,
            "alternative_action_modes": list(self.alternative_action_modes),
            "requested_effect": self.requested_effect,
            "evidence_mode": self.evidence_mode,
            "ambiguity_flags": list(self.ambiguity_flags),
            "planning_signals": list(self.planning_signals),
            "success_signals": list(self.success_signals),
            "risk_signals": list(self.risk_signals),
            "requested_output_count": self.requested_output_count,
            "desired_outputs_digest": self.desired_outputs_digest,
            "constraints_digest": self.constraints_digest,
        }

    @property
    def intent_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._descriptor(),
            "intent_digest": self.intent_digest,
            "retention": "value_only_intent_metadata;task_text_not_retained",
            "authorization": "classification_only;no_provider_tool_or_effect_authority",
            "secret_material": "never_returned",
        }

    def prompt_contract(self, *, compact: bool = False) -> dict[str, Any]:
        result = {
            "schema": AUTONOMOUS_TASK_INTENT_SCHEMA,
            "intent_id": self.intent_id,
            "intent_digest": self.intent_digest,
            "task_digest": self.task_digest,
            "lens_digest": self.lens_digest,
            "action_mode": self.action_mode,
            "requested_effect": self.requested_effect,
            "evidence_mode": self.evidence_mode,
            "ambiguity_flags": list(self.ambiguity_flags),
            "authority": "classification_only;no_provider_tool_or_effect_authority",
        }
        if not compact:
            result.update(
                {
                    "alternative_action_modes": list(self.alternative_action_modes),
                    "planning_signals": list(self.planning_signals),
                    "success_signals": list(self.success_signals),
                    "risk_signals": list(self.risk_signals),
                    "requested_output_count": self.requested_output_count,
                    "desired_outputs_digest": self.desired_outputs_digest,
                    "constraints_digest": self.constraints_digest,
                }
            )
        result["secret_material"] = "never_returned"
        return result


AUTONOMOUS_TASK_INTENT_DOMAINS = AUTONOMOUS_TASK_LENS_DOMAINS


def infer_autonomous_task_intent(
    *,
    task: str,
    task_digest: str,
    domain: str,
    capability: str,
    risk_class: str,
    workflow_id: str,
    lens: AutonomousDomainTaskLens,
    constraints: Sequence[str] = (),
    desired_outputs: Sequence[str] = (),
) -> AutonomousTaskIntent:
    """Classify a task with reviewed lexical cues and no provider call."""

    task_text = _bounded_text("task intent task", task, maximum=16_000)
    _digest(task_digest, "task intent task_digest")
    if task_digest != content_digest({"task": task_text}):
        raise ArgumentError("task intent task_digest does not match task text")
    if domain not in AUTONOMOUS_TASK_INTENT_DOMAINS or lens.domain != domain:
        raise ArgumentError("task intent domain and lens must agree")
    _bounded_text("task intent capability", capability, maximum=256)
    _bounded_text("task intent risk_class", risk_class, maximum=256)
    _bounded_text("task intent workflow_id", workflow_id, maximum=256)
    constraints = _bounded_input_items("task intent constraints", constraints)
    desired_outputs = _bounded_input_items("task intent desired_outputs", desired_outputs)
    normalized = _normalize(task_text)
    scores = {
        mode: sum(1 for cue in cues if _contains(normalized, cue))
        for mode, cues in _ACTION_CUES.items()
    }
    ranked = sorted(scores, key=lambda mode: (-scores[mode], AUTONOMOUS_TASK_INTENT_ACTION_MODES.index(mode)))
    active = [mode for mode in ranked if scores[mode] > 0]
    action_mode = active[0] if active else _DEFAULT_ACTIONS[domain]
    alternatives = tuple(active[1:5])
    ambiguity: list[str] = []
    if not active:
        ambiguity.append("missing_action_signal")
    elif len(active) > 1 and scores[active[0]] == scores[active[1]]:
        ambiguity.append("competing_action_modes")
    if not desired_outputs:
        ambiguity.append("no_explicit_output_contract")
    if any(_contains(normalized, cue) for cue in _UNCERTAINTY_CUES):
        ambiguity.append("uncertainty_language")
    if any(_contains(normalized, cue) for cue in _EXTERNAL_EFFECT_CUES):
        requested_effect = "external_effect"
        ambiguity.append("effect_requires_explicit_approval")
    elif any(_contains(normalized, cue) for cue in _LOCAL_CHANGE_CUES):
        requested_effect = "local_change"
    else:
        requested_effect = "none"
    risk_signals: list[str] = []
    if risk_class != "read_only":
        risk_signals.append("domain_policy_review")
    if requested_effect == "external_effect":
        risk_signals.append("external_effect_language")
    if any(_contains(normalized, cue) for cue in _CREDENTIAL_CUES):
        risk_signals.append("credential_or_secret_language")
    domain_risk_signals = {
        "biomedical": "human_review_boundary",
        "operations": "rollback_required",
        "enterprise": "governance_boundary",
        "multi_agent": "single_effect_authority",
        "cross_domain": "source_domain_ownership",
        "evaluation": "independent_evaluator",
    }
    if domain in domain_risk_signals:
        risk_signals.append(domain_risk_signals[domain])
    if not desired_outputs:
        risk_signals.append("output_contract_missing")
    success_signals = ["workflow_completion_contract", *lens.evaluator_signals]
    if desired_outputs:
        success_signals.append("caller_outputs_requested")
    constraints_digest = None if not constraints else content_digest(list(constraints))
    desired_outputs_digest = None if not desired_outputs else content_digest(list(desired_outputs))
    intent = AutonomousTaskIntent(
        domain=domain,
        capability=capability,
        risk_class=risk_class,
        workflow_id=workflow_id,
        task_digest=task_digest,
        lens_digest=lens.lens_digest,
        intent_id=f"{domain}:{workflow_id}:{action_mode}",
        action_mode=action_mode,
        alternative_action_modes=alternatives,
        requested_effect=requested_effect,
        evidence_mode=_EVIDENCE_MODES[domain],
        ambiguity_flags=_unique(ambiguity),
        planning_signals=_unique((f"action:{action_mode}", *lens.planning_dimensions)),
        success_signals=_unique(success_signals),
        risk_signals=_unique(risk_signals),
        requested_output_count=len(desired_outputs),
        desired_outputs_digest=desired_outputs_digest,
        constraints_digest=constraints_digest,
    )
    return intent


__all__ = [
    "AUTONOMOUS_TASK_INTENT_SCHEMA",
    "AUTONOMOUS_TASK_INTENT_VERSION",
    "AUTONOMOUS_TASK_INTENT_DOMAINS",
    "AUTONOMOUS_TASK_INTENT_ACTION_MODES",
    "AUTONOMOUS_TASK_INTENT_EFFECTS",
    "AUTONOMOUS_TASK_INTENT_EVIDENCE_MODES",
    "MAX_AUTONOMOUS_TASK_INTENT_ITEMS",
    "AutonomousTaskIntent",
    "infer_autonomous_task_intent",
]
