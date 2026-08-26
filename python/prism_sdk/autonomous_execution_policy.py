"""Contextual execution-policy selection and evaluator-only online learning.

The autonomous stack has separate routers, task decisions, model selectors, evidence planners,
and learners.  This module supplies the missing joint policy boundary: it ranks caller-provided
execution candidates, applies hard safety/resource gates, and updates a value-only UCB state after
an explicit evaluator verdict.  It never receives task text, prompts, responses, tool arguments,
credentials, or effect payloads, and transport success is never a reward.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .errors import ArgumentError


AUTONOMOUS_EXECUTION_POLICY_SCHEMA = "bioprism-autonomous-execution-policy/0.1"
AUTONOMOUS_EXECUTION_POLICY_STATE_SCHEMA = "bioprism-autonomous-execution-policy-state/0.1"
AUTONOMOUS_EXECUTION_POLICY_SETTLEMENT_SCHEMA = "bioprism-autonomous-execution-policy-settlement/0.1"
AUTONOMOUS_EXECUTION_POLICY_PATHS = ("provider", "evidence_first", "workflow", "planning", "cross_domain", "tool_loop")
AUTONOMOUS_EXECUTION_POLICY_POSTURES = ("selected", "review_required", "refused")
AUTONOMOUS_EXECUTION_POLICY_DOMAINS = ("coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation")
AUTONOMOUS_EXECUTION_POLICY_MAX_CANDIDATES = 256
AUTONOMOUS_EXECUTION_POLICY_MAX_ARMS = 512
AUTONOMOUS_EXECUTION_POLICY_MAX_SETTLEMENTS = 4096
AUTONOMOUS_EXECUTION_POLICY_MAX_ITEMS = 32
AUTONOMOUS_EXECUTION_POLICY_MAX_BYTES = 8_000_000
_RETENTION = "value_only_policy_metadata;task_prompt_response_tool_and_credential_values_not_retained"
_STATE_RETENTION = "value_only_policy_state;task_prompt_response_tool_and_credential_values_not_retained"
_SECRET_MATERIAL = "never_returned"
_SETTLEMENT_RETENTION = "value_only_explicit_evaluator_credit;no_transport_reward"


def _fail(message: str) -> "NoReturn":
    raise ArgumentError(f"autonomous execution policy {message}")


def _text(name: str, value: Any, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or len(value) > maximum or "\x00" in value or any(ord(character) < 32 or ord(character) == 127 for character in value):
        _fail(f"{name} is outside its bound")
    return value.strip()


def _identifier(name: str, value: Any, maximum: int = 256) -> str:
    result = _text(name, value, maximum)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in result):
        _fail(f"{name} must be a bounded identifier")
    return result


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if allow_none and value is None:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _finite(name: str, value: Any, minimum: float, maximum: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not minimum <= float(value) <= maximum:
        _fail(f"{name} is outside its numeric bound")
    return float(value)


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} is outside its integer bound")
    return value


def _boolean(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        _fail(f"{name} must be boolean")
    return value


def _domains(name: str, value: Any) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)) or not 1 <= len(value) <= len(AUTONOMOUS_EXECUTION_POLICY_DOMAINS):
        _fail(f"{name} must contain 1..{len(AUTONOMOUS_EXECUTION_POLICY_DOMAINS)} domains")
    result = tuple(value)
    if any(not isinstance(item, str) or item not in AUTONOMOUS_EXECUTION_POLICY_DOMAINS for item in result):
        _fail(f"{name} contains an unsupported domain")
    if len(set(result)) != len(result):
        _fail(f"{name} contains duplicate domains")
    return result


def _items(name: str, value: Any) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)) or len(value) > AUTONOMOUS_EXECUTION_POLICY_MAX_ITEMS:
        _fail(f"{name} exceeds its item bound")
    result = tuple(_identifier(f"{name} item", item, 128) for item in value)
    if len(set(result)) != len(result):
        _fail(f"{name} contains duplicate items")
    return result


def _round(value: float) -> float:
    rounded = round(float(value), 12)
    return int(rounded) if rounded.is_integer() else rounded


@dataclass(frozen=True, slots=True)
class AutonomousExecutionPolicyCandidate:
    arm_id: str
    domain: str
    path: str
    capabilities: tuple[str, ...] = ()
    quality_prior: float = 0.5
    reliability: float = 0.5
    cost_units: float = 1.0
    latency_ms: float = 0.0
    risk: float = 0.5
    available: bool = True
    evidence_ready: bool = False
    structured_output: bool = False
    effects_supported: bool = False
    approval_required: bool = False
    provider: str | None = None
    model: str | None = None

    def __post_init__(self) -> None:
        _identifier("candidate arm_id", self.arm_id)
        if self.domain not in AUTONOMOUS_EXECUTION_POLICY_DOMAINS:
            _fail("candidate domain is unsupported")
        if self.path not in AUTONOMOUS_EXECUTION_POLICY_PATHS:
            _fail("candidate path is unsupported")
        _items("candidate capabilities", self.capabilities)
        _finite("candidate quality_prior", self.quality_prior, 0, 1)
        _finite("candidate reliability", self.reliability, 0, 1)
        _finite("candidate cost_units", self.cost_units, 0, 1_000_000)
        _finite("candidate latency_ms", self.latency_ms, 0, 86_400_000)
        _finite("candidate risk", self.risk, 0, 1)
        for name, value in (("available", self.available), ("evidence_ready", self.evidence_ready), ("structured_output", self.structured_output), ("effects_supported", self.effects_supported), ("approval_required", self.approval_required)):
            _boolean(f"candidate {name}", value)
        if self.provider is not None:
            _text("candidate provider", self.provider)
        if self.model is not None:
            _text("candidate model", self.model)

    def _descriptor(self) -> dict[str, Any]:
        return {"arm_id": self.arm_id, "domain": self.domain, "path": self.path, "capabilities": list(self.capabilities), "quality_prior": _round(self.quality_prior), "reliability": _round(self.reliability), "cost_units": _round(self.cost_units), "latency_ms": _round(self.latency_ms), "risk": _round(self.risk), "available": self.available, "evidence_ready": self.evidence_ready, "structured_output": self.structured_output, "effects_supported": self.effects_supported, "approval_required": self.approval_required, "provider": self.provider, "model": self.model}

    @property
    def candidate_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "candidate_digest": self.candidate_digest}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any] | "AutonomousExecutionPolicyCandidate") -> "AutonomousExecutionPolicyCandidate":
        if isinstance(value, cls):
            return value
        if not isinstance(value, Mapping):
            _fail("candidate must be a mapping")
        result = cls(arm_id=value.get("arm_id"), domain=value.get("domain"), path=value.get("path"), capabilities=tuple(value.get("capabilities", ())), quality_prior=value.get("quality_prior", 0.5), reliability=value.get("reliability", 0.5), cost_units=value.get("cost_units", 1.0), latency_ms=value.get("latency_ms", 0.0), risk=value.get("risk", 0.5), available=value.get("available", True), evidence_ready=value.get("evidence_ready", False), structured_output=value.get("structured_output", False), effects_supported=value.get("effects_supported", False), approval_required=value.get("approval_required", False), provider=value.get("provider"), model=value.get("model"))
        if value.get("candidate_digest") is not None and value.get("candidate_digest") != result.candidate_digest:
            _fail("candidate digest does not match metadata")
        return result


@dataclass(frozen=True, slots=True)
class AutonomousExecutionPolicyContext:
    requested_domains: tuple[str, ...]
    context_digest: str | None = None
    required_capabilities: tuple[str, ...] = ()
    preferred_capabilities: tuple[str, ...] = ()
    required_path: str | None = None
    evidence_required: bool = False
    structured_output_required: bool = False
    effects_requested: bool = False
    effects_approved: bool = False
    approval_granted: bool = False
    max_cost_units: float = 1_000_000
    max_latency_ms: float = 86_400_000
    max_risk: float = 1.0
    min_score: float = 0.0

    def __post_init__(self) -> None:
        _domains("requested_domains", self.requested_domains)
        _digest("context_digest", self.context_digest, allow_none=True)
        _items("required_capabilities", self.required_capabilities)
        _items("preferred_capabilities", self.preferred_capabilities)
        if self.required_path is not None and self.required_path not in AUTONOMOUS_EXECUTION_POLICY_PATHS:
            _fail("required_path is unsupported")
        for name, value in (("evidence_required", self.evidence_required), ("structured_output_required", self.structured_output_required), ("effects_requested", self.effects_requested), ("effects_approved", self.effects_approved), ("approval_granted", self.approval_granted)):
            _boolean(name, value)
        _finite("max_cost_units", self.max_cost_units, 0, 1_000_000)
        _finite("max_latency_ms", self.max_latency_ms, 0, 86_400_000)
        _finite("max_risk", self.max_risk, 0, 1)
        _finite("min_score", self.min_score, -2, 2)

    def to_dict(self) -> dict[str, Any]:
        return {"context_digest": self.context_digest, "requested_domains": list(self.requested_domains), "required_capabilities": list(self.required_capabilities), "preferred_capabilities": list(self.preferred_capabilities), "required_path": self.required_path, "evidence_required": self.evidence_required, "structured_output_required": self.structured_output_required, "effects_requested": self.effects_requested, "effects_approved": self.effects_approved, "approval_granted": self.approval_granted, "max_cost_units": _round(self.max_cost_units), "max_latency_ms": _round(self.max_latency_ms), "max_risk": _round(self.max_risk), "min_score": _round(self.min_score)}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any] | "AutonomousExecutionPolicyContext") -> "AutonomousExecutionPolicyContext":
        if isinstance(value, cls):
            return value
        if not isinstance(value, Mapping):
            _fail("context must be a mapping")
        return cls(requested_domains=tuple(value.get("requested_domains", ())), context_digest=value.get("context_digest"), required_capabilities=tuple(value.get("required_capabilities", ())), preferred_capabilities=tuple(value.get("preferred_capabilities", ())), required_path=value.get("required_path"), evidence_required=value.get("evidence_required", False), structured_output_required=value.get("structured_output_required", False), effects_requested=value.get("effects_requested", False), effects_approved=value.get("effects_approved", False), approval_granted=value.get("approval_granted", False), max_cost_units=value.get("max_cost_units", 1_000_000), max_latency_ms=value.get("max_latency_ms", 86_400_000), max_risk=value.get("max_risk", 1.0), min_score=value.get("min_score", 0.0))


@dataclass(frozen=True, slots=True)
class AutonomousExecutionPolicyArmState:
    arm_id: str
    pulls: int = 0
    failures: int = 0
    reward_sum: float = 0.0
    last_reward: float | None = None
    last_outcome_digest: str | None = None
    last_generation: int = 0

    def to_dict(self) -> dict[str, Any]:
        return {"arm_id": self.arm_id, "pulls": self.pulls, "failures": self.failures, "reward_sum": _round(self.reward_sum), "last_reward": None if self.last_reward is None else _round(self.last_reward), "last_outcome_digest": self.last_outcome_digest, "last_generation": self.last_generation}


@dataclass(frozen=True, slots=True)
class AutonomousExecutionPolicySettlementRecord:
    settlement_id: str
    arm_id: str
    outcome_digest: str
    reward: float
    passed: bool
    evaluator_id: str
    evaluator_version: str

    def to_dict(self) -> dict[str, Any]:
        return {"settlement_id": self.settlement_id, "arm_id": self.arm_id, "outcome_digest": self.outcome_digest, "reward": _round(self.reward), "passed": self.passed, "evaluator_id": self.evaluator_id, "evaluator_version": self.evaluator_version}


@dataclass(frozen=True, slots=True)
class AutonomousExecutionPolicyState:
    generation: int
    previous_state_digest: str | None
    arms: tuple[AutonomousExecutionPolicyArmState, ...]
    settlements: tuple[AutonomousExecutionPolicySettlementRecord, ...]

    def _descriptor(self) -> dict[str, Any]:
        return {"schema": AUTONOMOUS_EXECUTION_POLICY_STATE_SCHEMA, "generation": self.generation, "previous_state_digest": self.previous_state_digest, "arms": [arm.to_dict() for arm in sorted(self.arms, key=lambda item: item.arm_id)], "settlements": [item.to_dict() for item in self.settlements], "retention": _STATE_RETENTION, "secret_material": _SECRET_MATERIAL}

    @property
    def state_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "state_digest": self.state_digest}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any] | "AutonomousExecutionPolicyState") -> "AutonomousExecutionPolicyState":
        if isinstance(value, cls):
            return value
        if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_EXECUTION_POLICY_STATE_SCHEMA or value.get("retention") != _STATE_RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
            _fail("state markers are invalid")
        generation = _integer("state generation", value.get("generation"), 0, 2_147_483_647)
        previous_state_digest = _digest("state previous_state_digest", value.get("previous_state_digest"), allow_none=True)
        if (generation == 0 and previous_state_digest is not None) or (generation > 0 and previous_state_digest is None):
            _fail("state predecessor digest fence is malformed")
        raw_arms = value.get("arms")
        raw_settlements = value.get("settlements")
        if not isinstance(raw_arms, Sequence) or isinstance(raw_arms, (str, bytes)) or len(raw_arms) > AUTONOMOUS_EXECUTION_POLICY_MAX_ARMS:
            _fail("state arms exceed capacity")
        if not isinstance(raw_settlements, Sequence) or isinstance(raw_settlements, (str, bytes)) or len(raw_settlements) > AUTONOMOUS_EXECUTION_POLICY_MAX_SETTLEMENTS:
            _fail("state settlements exceed capacity")
        arms: list[AutonomousExecutionPolicyArmState] = []
        for raw in raw_arms:
            if not isinstance(raw, Mapping):
                _fail("state arm is malformed")
            arm_id = _identifier("state arm_id", raw.get("arm_id"))
            pulls = _integer("state arm pulls", raw.get("pulls"), 0, 2_147_483_647)
            failures = _integer("state arm failures", raw.get("failures"), 0, pulls)
            reward_sum = _finite("state arm reward_sum", raw.get("reward_sum"), 0, pulls)
            last_reward = None if raw.get("last_reward") is None else _finite("state arm last_reward", raw.get("last_reward"), 0, 1)
            last_outcome = _digest("state arm last_outcome_digest", raw.get("last_outcome_digest"), allow_none=True)
            last_generation = _integer("state arm last_generation", raw.get("last_generation"), 0, generation)
            arms.append(AutonomousExecutionPolicyArmState(arm_id, pulls, failures, reward_sum, last_reward, last_outcome, last_generation))
        if len({arm.arm_id for arm in arms}) != len(arms):
            _fail("state contains duplicate arms")
        settlements: list[AutonomousExecutionPolicySettlementRecord] = []
        for raw in raw_settlements:
            if not isinstance(raw, Mapping):
                _fail("state settlement is malformed")
            settlements.append(AutonomousExecutionPolicySettlementRecord(_identifier("state settlement_id", raw.get("settlement_id")), _identifier("state settlement arm_id", raw.get("arm_id")), _digest("state settlement outcome_digest", raw.get("outcome_digest")) or "", _finite("state settlement reward", raw.get("reward"), 0, 1), _boolean("state settlement passed", raw.get("passed")), _identifier("state settlement evaluator_id", raw.get("evaluator_id"), 128), _identifier("state settlement evaluator_version", raw.get("evaluator_version"), 128)))
        if len({item.settlement_id for item in settlements}) != len(settlements):
            _fail("state contains duplicate settlements")
        result = cls(generation, previous_state_digest, tuple(sorted(arms, key=lambda item: item.arm_id)), tuple(settlements))
        if value.get("state_digest") != result.state_digest:
            _fail("state digest does not match metadata")
        if len(canonical_json(result.to_dict()).encode("utf-8")) > AUTONOMOUS_EXECUTION_POLICY_MAX_BYTES:
            _fail("state exceeds its byte bound")
        return result


def _empty_state() -> AutonomousExecutionPolicyState:
    return AutonomousExecutionPolicyState(0, None, (), ())


@dataclass(frozen=True, slots=True)
class AutonomousExecutionPolicyRanking:
    arm_id: str
    domain: str
    path: str
    candidate_digest: str
    eligible: bool
    score: float | None
    exploitation: float | None
    exploration_bonus: float | None
    confidence: float | None
    mean_reward: float | None
    preferred_capability_match: float
    reasons: tuple[str, ...]
    review_reasons: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {"arm_id": self.arm_id, "domain": self.domain, "path": self.path, "candidate_digest": self.candidate_digest, "eligible": self.eligible, "score": None if self.score is None else _round(self.score), "exploitation": None if self.exploitation is None else _round(self.exploitation), "exploration_bonus": None if self.exploration_bonus is None else _round(self.exploration_bonus), "confidence": None if self.confidence is None else _round(self.confidence), "mean_reward": None if self.mean_reward is None else _round(self.mean_reward), "preferred_capability_match": _round(self.preferred_capability_match), "reasons": list(self.reasons), "review_reasons": list(self.review_reasons)}


@dataclass(frozen=True, slots=True)
class AutonomousExecutionPolicyDecision:
    context: AutonomousExecutionPolicyContext
    policy_generation: int
    total_pulls: int
    posture: str
    selected_arm_id: str | None
    selected_candidate: AutonomousExecutionPolicyCandidate | None
    rankings: tuple[AutonomousExecutionPolicyRanking, ...]
    review_reasons: tuple[str, ...]
    refusal_reasons: tuple[str, ...]

    def _descriptor(self) -> dict[str, Any]:
        return {"schema": AUTONOMOUS_EXECUTION_POLICY_SCHEMA, "context": self.context.to_dict(), "policy_generation": self.policy_generation, "total_pulls": self.total_pulls, "posture": self.posture, "selected_arm_id": self.selected_arm_id, "selected_candidate": None if self.selected_candidate is None else self.selected_candidate.to_dict(), "rankings": [row.to_dict() for row in self.rankings], "review_reasons": list(self.review_reasons), "refusal_reasons": list(self.refusal_reasons)}

    @property
    def decision_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "decision_digest": self.decision_digest, "authorization": "guidance_only;provider_source_tool_effect_and_credential_authority_remain_separate", "retention": _RETENTION, "secret_material": _SECRET_MATERIAL}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any] | "AutonomousExecutionPolicyDecision") -> "AutonomousExecutionPolicyDecision":
        if isinstance(value, cls):
            value = value.to_dict()
        if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_EXECUTION_POLICY_SCHEMA:
            _fail("decision is malformed")
        context = AutonomousExecutionPolicyContext.from_mapping(value.get("context"))
        selected_raw = value.get("selected_candidate")
        selected = None if selected_raw is None else AutonomousExecutionPolicyCandidate.from_mapping(selected_raw)
        raw_rankings = value.get("rankings")
        if not isinstance(raw_rankings, Sequence) or isinstance(raw_rankings, (str, bytes)) or len(raw_rankings) > AUTONOMOUS_EXECUTION_POLICY_MAX_CANDIDATES:
            _fail("decision rankings exceed capacity")
        rankings: list[AutonomousExecutionPolicyRanking] = []
        for raw in raw_rankings:
            if not isinstance(raw, Mapping):
                _fail("decision ranking is malformed")
            domain = raw.get("domain")
            path = raw.get("path")
            if domain not in AUTONOMOUS_EXECUTION_POLICY_DOMAINS or path not in AUTONOMOUS_EXECUTION_POLICY_PATHS:
                _fail("decision ranking domain or path is invalid")
            arm_id = _identifier("decision ranking arm_id", raw.get("arm_id"))
            rankings.append(AutonomousExecutionPolicyRanking(arm_id, domain, path, _digest("decision ranking candidate_digest", raw.get("candidate_digest")) or "", _boolean("decision ranking eligible", raw.get("eligible")), None if raw.get("score") is None else _finite("decision ranking score", raw.get("score"), -2, 2), None if raw.get("exploitation") is None else _finite("decision ranking exploitation", raw.get("exploitation"), -2, 2), None if raw.get("exploration_bonus") is None else _finite("decision ranking exploration_bonus", raw.get("exploration_bonus"), 0, 2), None if raw.get("confidence") is None else _finite("decision ranking confidence", raw.get("confidence"), 0, 1), None if raw.get("mean_reward") is None else _finite("decision ranking mean_reward", raw.get("mean_reward"), 0, 1), _finite("decision ranking preferred_capability_match", raw.get("preferred_capability_match"), 0, 1), _items("decision ranking reasons", raw.get("reasons", ())), _items("decision ranking review_reasons", raw.get("review_reasons", ()))))
        if len({row.arm_id for row in rankings}) != len(rankings):
            _fail("decision rankings contain duplicate arm IDs")
        result = cls(context, _integer("decision policy_generation", value.get("policy_generation"), 0, 2_147_483_647), _integer("decision total_pulls", value.get("total_pulls"), 0, 2_147_483_647), value.get("posture"), None if value.get("selected_arm_id") is None else _identifier("decision selected_arm_id", value.get("selected_arm_id")), selected, tuple(rankings), _items("decision review_reasons", value.get("review_reasons", ())), _items("decision refusal_reasons", value.get("refusal_reasons", ())))
        if result.posture not in AUTONOMOUS_EXECUTION_POLICY_POSTURES or (result.selected_arm_id is None) != (result.selected_candidate is None) or value.get("authorization") != "guidance_only;provider_source_tool_effect_and_credential_authority_remain_separate" or value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL or value.get("decision_digest") != result.decision_digest:
            _fail("decision metadata or digest is invalid")
        if (result.posture == "refused" and result.selected_arm_id is not None) or (result.posture != "refused" and result.selected_arm_id is None):
            _fail("decision posture and selection are inconsistent")
        if result.selected_candidate is not None and (result.selected_candidate.arm_id != result.selected_arm_id or not any(row.arm_id == result.selected_arm_id and row.candidate_digest == result.selected_candidate.candidate_digest for row in result.rankings)):
            _fail("decision selected candidate is not bound to its ranking")
        return result


@dataclass(frozen=True, slots=True)
class AutonomousExecutionPolicySettlement:
    settlement_id: str
    arm_id: str
    outcome_digest: str
    reward: float
    passed: bool
    evaluator_id: str
    evaluator_version: str
    previous_state_digest: str
    next_state_digest: str
    generation: int
    idempotent_replay: bool

    def to_dict(self) -> dict[str, Any]:
        return {"schema": AUTONOMOUS_EXECUTION_POLICY_SETTLEMENT_SCHEMA, "settlement_id": self.settlement_id, "arm_id": self.arm_id, "outcome_digest": self.outcome_digest, "reward": _round(self.reward), "passed": self.passed, "evaluator_id": self.evaluator_id, "evaluator_version": self.evaluator_version, "previous_state_digest": self.previous_state_digest, "next_state_digest": self.next_state_digest, "generation": self.generation, "idempotent_replay": self.idempotent_replay, "retention": _SETTLEMENT_RETENTION, "secret_material": _SECRET_MATERIAL}


class AutonomousExecutionPolicy:
    """Select one bounded execution arm and learn only from evaluator-owned credit."""

    def __init__(self, *, state: AutonomousExecutionPolicyState | Mapping[str, Any] | None = None, exploration: float = 0.35) -> None:
        self.exploration = _finite("exploration", exploration, 0, 2)
        self._state = _empty_state() if state is None else AutonomousExecutionPolicyState.from_mapping(state)

    @property
    def generation(self) -> int:
        return self._state.generation

    def snapshot(self) -> dict[str, Any]:
        return self._state.to_dict()

    def restore(self, value: AutonomousExecutionPolicyState | Mapping[str, Any]) -> None:
        next_state = AutonomousExecutionPolicyState.from_mapping(value)
        if next_state.generation < self._state.generation:
            _fail("state restore would roll back a newer generation")
        if next_state.generation == self._state.generation and next_state.state_digest != self._state.state_digest:
            _fail("state restore conflicts with the current generation")
        if next_state.generation == self._state.generation + 1 and next_state.previous_state_digest != self._state.state_digest:
            _fail("state restore predecessor digest does not match the current generation")
        self._state = next_state

    def select(self, context: AutonomousExecutionPolicyContext | Mapping[str, Any], candidates: Sequence[AutonomousExecutionPolicyCandidate | Mapping[str, Any]]) -> AutonomousExecutionPolicyDecision:
        selected_context = AutonomousExecutionPolicyContext.from_mapping(context)
        if not isinstance(candidates, Sequence) or isinstance(candidates, (str, bytes)) or not 1 <= len(candidates) <= AUTONOMOUS_EXECUTION_POLICY_MAX_CANDIDATES:
            _fail("candidates are outside their bound")
        normalized = tuple(AutonomousExecutionPolicyCandidate.from_mapping(item) for item in candidates)
        if len({item.arm_id for item in normalized}) != len(normalized):
            _fail("candidates contain duplicate arm_id values")
        by_arm = {arm.arm_id: arm for arm in self._state.arms}
        total_pulls = sum(arm.pulls for arm in self._state.arms)
        rows: list[tuple[AutonomousExecutionPolicyRanking, AutonomousExecutionPolicyCandidate]] = []
        for item in normalized:
            reasons: list[str] = []
            reviews: list[str] = []
            if item.domain not in selected_context.requested_domains:
                reasons.append("domain_not_requested")
            if not item.available:
                reasons.append("candidate_unavailable")
            if selected_context.required_path is not None and item.path != selected_context.required_path:
                reasons.append("path_not_requested")
            if any(capability not in item.capabilities for capability in selected_context.required_capabilities):
                reasons.append("required_capability_missing")
            if selected_context.evidence_required and not item.evidence_ready:
                reasons.append("evidence_not_ready")
            if selected_context.structured_output_required and not item.structured_output:
                reasons.append("structured_output_not_supported")
            if selected_context.effects_requested and not item.effects_supported:
                reasons.append("effects_not_supported")
            if item.cost_units > selected_context.max_cost_units:
                reasons.append("cost_budget_exceeded")
            if item.latency_ms > selected_context.max_latency_ms:
                reasons.append("latency_budget_exceeded")
            if item.risk > selected_context.max_risk:
                reasons.append("risk_budget_exceeded")
            if item.approval_required and not selected_context.approval_granted:
                reviews.append("candidate_approval_required")
            if selected_context.effects_requested and not selected_context.effects_approved:
                reviews.append("effect_approval_required")
            if reasons:
                rows.append((AutonomousExecutionPolicyRanking(item.arm_id, item.domain, item.path, item.candidate_digest, False, None, None, None, None, None, 0.0, tuple(reasons), tuple(reviews)), item))
                continue
            arm = by_arm.get(item.arm_id, AutonomousExecutionPolicyArmState(item.arm_id))
            prior_weight = 4.0
            mean_reward = (arm.reward_sum + item.quality_prior * prior_weight) / (arm.pulls + prior_weight)
            confidence = arm.pulls / (arm.pulls + prior_weight)
            exploration_bonus = self.exploration * math.sqrt(math.log(total_pulls + 2) / (arm.pulls + 1))
            preferred_match = 0.5 if not selected_context.preferred_capabilities else sum(capability in item.capabilities for capability in selected_context.preferred_capabilities) / len(selected_context.preferred_capabilities)
            cost_penalty = 0.0 if selected_context.max_cost_units == 0 else item.cost_units / selected_context.max_cost_units
            latency_penalty = 0.0 if selected_context.max_latency_ms == 0 else item.latency_ms / selected_context.max_latency_ms
            exploitation = 0.45 * mean_reward + 0.2 * item.quality_prior + 0.2 * item.reliability + 0.15 * preferred_match
            score = exploitation + exploration_bonus - 0.12 * item.risk - 0.08 * cost_penalty - 0.05 * latency_penalty
            rows.append((AutonomousExecutionPolicyRanking(item.arm_id, item.domain, item.path, item.candidate_digest, True, _round(score), _round(exploitation), _round(exploration_bonus), _round(confidence), _round(mean_reward), _round(preferred_match), (), tuple(reviews)), item))
        rows.sort(key=lambda pair: (-(pair[0].score if pair[0].score is not None else -math.inf), -(pair[0].exploitation if pair[0].exploitation is not None else -math.inf), -pair[1].reliability, pair[0].arm_id))
        winner = next((pair for pair in rows if pair[0].eligible and pair[0].score is not None and pair[0].score >= selected_context.min_score), None)
        refusal = () if winner is not None else tuple(dict.fromkeys(reason for row, _ in rows for reason in (row.reasons or ("all_candidates_below_score_floor",))))
        review = () if winner is None else tuple(dict.fromkeys(winner[0].review_reasons))
        posture = "refused" if winner is None else "review_required" if review else "selected"
        decision = AutonomousExecutionPolicyDecision(selected_context, self._state.generation, total_pulls, posture, None if winner is None else winner[1].arm_id, None if winner is None else winner[1], tuple(row for row, _ in rows), review, refusal)
        if len(canonical_json(decision.to_dict()).encode("utf-8")) > AUTONOMOUS_EXECUTION_POLICY_MAX_BYTES:
            _fail("decision exceeds its byte bound")
        return decision

    def settle(self, decision: AutonomousExecutionPolicyDecision | Mapping[str, Any], *, settlement_id: str, arm_id: str, decision_digest: str, outcome_digest: str, reward: float, passed: bool, evaluator_id: str, evaluator_version: str) -> AutonomousExecutionPolicySettlement:
        checked = AutonomousExecutionPolicyDecision.from_mapping(decision)
        if checked.selected_arm_id is None or checked.posture == "refused":
            _fail("cannot settle a refused decision")
        settlement_id = _identifier("settlement_id", settlement_id)
        arm_id = _identifier("settlement arm_id", arm_id)
        if arm_id != checked.selected_arm_id:
            _fail("settlement arm_id does not match the selected arm")
        outcome_digest = _digest("settlement outcome_digest", outcome_digest) or ""
        if _digest("settlement decision_digest", decision_digest) != checked.decision_digest:
            _fail("settlement decision_digest does not match the decision")
        reward = _finite("settlement reward", reward, 0, 1)
        passed = _boolean("settlement passed", passed)
        evaluator_id = _identifier("settlement evaluator_id", evaluator_id, 128)
        evaluator_version = _identifier("settlement evaluator_version", evaluator_version, 128)
        existing = next((item for item in self._state.settlements if item.settlement_id == settlement_id), None)
        if existing is not None:
            if (existing.arm_id, existing.outcome_digest, existing.reward, existing.passed, existing.evaluator_id, existing.evaluator_version) != (arm_id, outcome_digest, reward, passed, evaluator_id, evaluator_version):
                _fail("settlement_id was reused for different evaluator credit")
            return AutonomousExecutionPolicySettlement(settlement_id, arm_id, outcome_digest, reward, passed, evaluator_id, evaluator_version, self._state.state_digest, self._state.state_digest, self._state.generation, True)
        if self._state.generation >= 2_147_483_647 or len(self._state.settlements) >= AUTONOMOUS_EXECUTION_POLICY_MAX_SETTLEMENTS:
            _fail("settlement state capacity is exhausted")
        previous = self._state.state_digest
        arms = list(self._state.arms)
        index = next((position for position, item in enumerate(arms) if item.arm_id == arm_id), None)
        prior = arms[index] if index is not None else AutonomousExecutionPolicyArmState(arm_id)
        updated = AutonomousExecutionPolicyArmState(arm_id, prior.pulls + 1, prior.failures + (0 if passed else 1), _round(prior.reward_sum + reward), reward, outcome_digest, self._state.generation + 1)
        if index is None:
            if len(arms) >= AUTONOMOUS_EXECUTION_POLICY_MAX_ARMS:
                _fail("arm capacity is exhausted")
            arms.append(updated)
        else:
            arms[index] = updated
        settlements = self._state.settlements + (AutonomousExecutionPolicySettlementRecord(settlement_id, arm_id, outcome_digest, reward, passed, evaluator_id, evaluator_version),)
        self._state = AutonomousExecutionPolicyState(self._state.generation + 1, previous, tuple(sorted(arms, key=lambda item: item.arm_id)), settlements)
        return AutonomousExecutionPolicySettlement(settlement_id, arm_id, outcome_digest, reward, passed, evaluator_id, evaluator_version, previous, self._state.state_digest, self._state.generation, False)


def validate_autonomous_execution_policy_state(value: Mapping[str, Any] | AutonomousExecutionPolicyState) -> AutonomousExecutionPolicyState:
    return AutonomousExecutionPolicyState.from_mapping(value)


def validate_autonomous_execution_policy_decision(value: Mapping[str, Any] | AutonomousExecutionPolicyDecision) -> AutonomousExecutionPolicyDecision:
    return AutonomousExecutionPolicyDecision.from_mapping(value)


def select_autonomous_execution_policy(context: Mapping[str, Any] | AutonomousExecutionPolicyContext, candidates: Sequence[AutonomousExecutionPolicyCandidate | Mapping[str, Any]], *, state: Mapping[str, Any] | AutonomousExecutionPolicyState | None = None, exploration: float = 0.35) -> AutonomousExecutionPolicyDecision:
    return AutonomousExecutionPolicy(state=state, exploration=exploration).select(context, candidates)


# The lower-level execution journal already owns the shorter AutonomousExecutionPolicy name.
# Keep this joint selector namespaced so both contracts can safely be imported together.
AUTONOMOUS_JOINT_EXECUTION_POLICY_SCHEMA = AUTONOMOUS_EXECUTION_POLICY_SCHEMA
AUTONOMOUS_JOINT_EXECUTION_POLICY_STATE_SCHEMA = AUTONOMOUS_EXECUTION_POLICY_STATE_SCHEMA
AUTONOMOUS_JOINT_EXECUTION_POLICY_SETTLEMENT_SCHEMA = AUTONOMOUS_EXECUTION_POLICY_SETTLEMENT_SCHEMA
AUTONOMOUS_JOINT_EXECUTION_POLICY_PATHS = AUTONOMOUS_EXECUTION_POLICY_PATHS
AUTONOMOUS_JOINT_EXECUTION_POLICY_POSTURES = AUTONOMOUS_EXECUTION_POLICY_POSTURES
AUTONOMOUS_JOINT_EXECUTION_POLICY_DOMAINS = AUTONOMOUS_EXECUTION_POLICY_DOMAINS
AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_CANDIDATES = AUTONOMOUS_EXECUTION_POLICY_MAX_CANDIDATES
AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_ARMS = AUTONOMOUS_EXECUTION_POLICY_MAX_ARMS
AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_SETTLEMENTS = AUTONOMOUS_EXECUTION_POLICY_MAX_SETTLEMENTS
AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_ITEMS = AUTONOMOUS_EXECUTION_POLICY_MAX_ITEMS
AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_BYTES = AUTONOMOUS_EXECUTION_POLICY_MAX_BYTES
AutonomousJointExecutionPolicyCandidate = AutonomousExecutionPolicyCandidate
AutonomousJointExecutionPolicyContext = AutonomousExecutionPolicyContext
AutonomousJointExecutionPolicyArmState = AutonomousExecutionPolicyArmState
AutonomousJointExecutionPolicySettlementRecord = AutonomousExecutionPolicySettlementRecord
AutonomousJointExecutionPolicyState = AutonomousExecutionPolicyState
AutonomousJointExecutionPolicyRanking = AutonomousExecutionPolicyRanking
AutonomousJointExecutionPolicyDecision = AutonomousExecutionPolicyDecision
AutonomousJointExecutionPolicySettlement = AutonomousExecutionPolicySettlement
AutonomousJointExecutionPolicy = AutonomousExecutionPolicy
validate_autonomous_joint_execution_policy_state = validate_autonomous_execution_policy_state
validate_autonomous_joint_execution_policy_decision = validate_autonomous_execution_policy_decision
select_autonomous_joint_execution_policy = select_autonomous_execution_policy


__all__ = [
    "AUTONOMOUS_EXECUTION_POLICY_SCHEMA",
    "AUTONOMOUS_EXECUTION_POLICY_STATE_SCHEMA",
    "AUTONOMOUS_EXECUTION_POLICY_SETTLEMENT_SCHEMA",
    "AUTONOMOUS_EXECUTION_POLICY_PATHS",
    "AUTONOMOUS_EXECUTION_POLICY_POSTURES",
    "AUTONOMOUS_EXECUTION_POLICY_DOMAINS",
    "AUTONOMOUS_EXECUTION_POLICY_MAX_CANDIDATES",
    "AUTONOMOUS_EXECUTION_POLICY_MAX_ARMS",
    "AUTONOMOUS_EXECUTION_POLICY_MAX_SETTLEMENTS",
    "AUTONOMOUS_EXECUTION_POLICY_MAX_ITEMS",
    "AUTONOMOUS_EXECUTION_POLICY_MAX_BYTES",
    "AutonomousExecutionPolicyCandidate",
    "AutonomousExecutionPolicyContext",
    "AutonomousExecutionPolicyArmState",
    "AutonomousExecutionPolicySettlementRecord",
    "AutonomousExecutionPolicyState",
    "AutonomousExecutionPolicyRanking",
    "AutonomousExecutionPolicyDecision",
    "AutonomousExecutionPolicySettlement",
    "AutonomousExecutionPolicy",
    "validate_autonomous_execution_policy_state",
    "validate_autonomous_execution_policy_decision",
    "select_autonomous_execution_policy",
    "AUTONOMOUS_JOINT_EXECUTION_POLICY_SCHEMA",
    "AUTONOMOUS_JOINT_EXECUTION_POLICY_STATE_SCHEMA",
    "AUTONOMOUS_JOINT_EXECUTION_POLICY_SETTLEMENT_SCHEMA",
    "AUTONOMOUS_JOINT_EXECUTION_POLICY_PATHS",
    "AUTONOMOUS_JOINT_EXECUTION_POLICY_POSTURES",
    "AUTONOMOUS_JOINT_EXECUTION_POLICY_DOMAINS",
    "AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_CANDIDATES",
    "AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_ARMS",
    "AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_SETTLEMENTS",
    "AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_ITEMS",
    "AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_BYTES",
    "AutonomousJointExecutionPolicyCandidate",
    "AutonomousJointExecutionPolicyContext",
    "AutonomousJointExecutionPolicyArmState",
    "AutonomousJointExecutionPolicySettlementRecord",
    "AutonomousJointExecutionPolicyState",
    "AutonomousJointExecutionPolicyRanking",
    "AutonomousJointExecutionPolicyDecision",
    "AutonomousJointExecutionPolicySettlement",
    "AutonomousJointExecutionPolicy",
    "validate_autonomous_joint_execution_policy_state",
    "validate_autonomous_joint_execution_policy_decision",
    "select_autonomous_joint_execution_policy",
]
