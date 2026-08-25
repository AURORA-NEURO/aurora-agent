"""Evaluator-driven prompt-variant selection for the autonomous brain.

Prompt registries already make renderer identity explicit, but deterministic selection alone
cannot improve a prompt implementation from observed task quality.  This module adds a small,
caller-owned UCB policy that selects among registry candidates and learns only from explicit
evaluator rewards.  It deliberately stores manifest identity, arm counts, reward values, and
settlement digests; task text, rendered messages, provider payloads, credentials, and evaluator
feedback remain outside the durable projection.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .autonomous_prompt_registry import (
    AutonomousPromptRegistry,
    AutonomousPromptSelectionPlan,
    AutonomousPromptSelectionRow,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_PROMPT_LEARNING_SCHEMA = "bioprism-python-autonomous-prompt-learning/0.1"
AUTONOMOUS_PROMPT_ADAPTIVE_SELECTION_SCHEMA = "bioprism-python-autonomous-prompt-adaptive-selection/0.1"
AUTONOMOUS_PROMPT_LEARNING_SETTLEMENT_SCHEMA = "bioprism-python-autonomous-prompt-learning-settlement/0.1"
AUTONOMOUS_PROMPT_LEARNING_POLICY = "ucb1_explicit_evaluator_v1"
AUTONOMOUS_PROMPT_LEARNING_RETENTION = "value_only_prompt_manifest_arms_and_settlement_digests"
MAX_AUTONOMOUS_PROMPT_LEARNING_ARMS = 4_096
MAX_AUTONOMOUS_PROMPT_LEARNING_SETTLEMENTS = 4_096
MAX_AUTONOMOUS_PROMPT_LEARNING_EXPLORATION = 2.0


def _text(name: str, value: Any, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value.strip()


def _digest(name: str, value: Any) -> str:
    result = _text(name, value, 64)
    if len(result) != 64 or any(character not in "0123456789abcdef" for character in result):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return result


def _finite(name: str, value: Any, minimum: float, maximum: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not minimum <= float(value) <= maximum:
        raise ArgumentError(f"{name} is outside its numeric bounds")
    return float(value)


def _json_number(value: float) -> int | float:
    """Match JSON.stringify's representation of integral floating-point values."""

    return 0 if value == 0.0 else value


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise ArgumentError(f"{name} is outside its integer bounds")
    return value


def _capabilities(value: Sequence[str]) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or len(value) > 64:
        raise ArgumentError("prompt learning required capabilities are outside their bounds")
    result = tuple(_text("prompt learning capability", item, 256) for item in value)
    if len(set(result)) != len(result):
        raise ArgumentError("prompt learning required capabilities contain duplicates")
    return result


def _settlement_record(value: Mapping[str, Any]) -> dict[str, Any]:
    """Normalize the bounded, value-only replay ledger representation."""

    if not isinstance(value, Mapping):
        raise ArgumentError("prompt learning settlement must be a mapping")
    required = {
        "settlement_key",
        "arm_id",
        "selection_digest",
        "evaluator_id",
        "evaluator_version",
        "reward",
        "passed",
        "outcome_digest",
    }
    if set(value) != required:
        raise ArgumentError("prompt learning settlement fields are invalid")
    passed = value.get("passed")
    if not isinstance(passed, bool):
        raise ArgumentError("prompt learning settlement passed must be boolean")
    return {
        "settlement_key": _digest("prompt learning settlement_key", value.get("settlement_key")),
        "arm_id": _digest("prompt learning settlement arm_id", value.get("arm_id")),
        "selection_digest": _digest("prompt learning settlement selection_digest", value.get("selection_digest")),
        "evaluator_id": _text("prompt learning settlement evaluator_id", value.get("evaluator_id")),
        "evaluator_version": _text("prompt learning settlement evaluator_version", value.get("evaluator_version"), 128),
        "reward": _json_number(_finite("prompt learning settlement reward", value.get("reward"), -1.0, 1.0)),
        "passed": passed,
        "outcome_digest": _digest("prompt learning settlement outcome_digest", value.get("outcome_digest")),
    }


def prompt_learning_arm_id(
    *,
    domain: str,
    stage: str,
    required_capabilities: Sequence[str],
    prompt_id: str,
    version: str,
    manifest_digest: str,
) -> str:
    """Compute a stable, registry-bound identity for one prompt arm."""

    domain = _text("prompt learning arm domain", domain)
    if domain not in AUTONOMOUS_DOMAIN_NAMES:
        raise ArgumentError("prompt learning arm domain is unsupported")
    descriptor = {
        "domain": domain,
        "stage": _text("prompt learning arm stage", stage),
        "required_capabilities": list(_capabilities(required_capabilities)),
        "prompt_id": _text("prompt learning arm prompt_id", prompt_id),
        "version": _text("prompt learning arm version", version, 128),
        "manifest_digest": _digest("prompt learning arm manifest_digest", manifest_digest),
    }
    return content_digest(descriptor)


@dataclass(frozen=True, slots=True)
class AutonomousPromptLearningArm:
    domain: str
    stage: str
    required_capabilities: tuple[str, ...]
    prompt_id: str
    version: str
    manifest_digest: str
    pulls: int = 0
    failures: int = 0
    reward_sum: float = 0.0

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("prompt learning arm domain is unsupported")
        object.__setattr__(self, "domain", _text("prompt learning arm domain", self.domain))
        object.__setattr__(self, "stage", _text("prompt learning arm stage", self.stage))
        object.__setattr__(self, "required_capabilities", _capabilities(self.required_capabilities))
        object.__setattr__(self, "prompt_id", _text("prompt learning arm prompt_id", self.prompt_id))
        object.__setattr__(self, "version", _text("prompt learning arm version", self.version, 128))
        object.__setattr__(self, "manifest_digest", _digest("prompt learning arm manifest_digest", self.manifest_digest))
        object.__setattr__(self, "pulls", _integer("prompt learning arm pulls", self.pulls, 0, 2_147_483_647))
        object.__setattr__(self, "failures", _integer("prompt learning arm failures", self.failures, 0, 2_147_483_647))
        if self.failures > self.pulls:
            raise ArgumentError("prompt learning arm failures exceed pulls")
        object.__setattr__(self, "reward_sum", _finite("prompt learning arm reward_sum", self.reward_sum, -self.pulls, self.pulls))

    @property
    def arm_id(self) -> str:
        return prompt_learning_arm_id(
            domain=self.domain,
            stage=self.stage,
            required_capabilities=self.required_capabilities,
            prompt_id=self.prompt_id,
            version=self.version,
            manifest_digest=self.manifest_digest,
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "arm_id": self.arm_id,
            "domain": self.domain,
            "stage": self.stage,
            "required_capabilities": list(self.required_capabilities),
            "prompt_id": self.prompt_id,
            "version": self.version,
            "manifest_digest": self.manifest_digest,
            "pulls": self.pulls,
            "failures": self.failures,
            "reward_sum": _json_number(self.reward_sum),
        }


@dataclass(frozen=True, slots=True)
class AutonomousPromptLearningState:
    registry_digest: str
    generation: int = 0
    arms: tuple[AutonomousPromptLearningArm, ...] = ()
    settlements: tuple[Mapping[str, Any], ...] = ()

    def __post_init__(self) -> None:
        object.__setattr__(self, "registry_digest", _digest("prompt learning registry_digest", self.registry_digest))
        object.__setattr__(self, "generation", _integer("prompt learning generation", self.generation, 0, 2_147_483_647))
        if not isinstance(self.arms, Sequence) or len(self.arms) > MAX_AUTONOMOUS_PROMPT_LEARNING_ARMS or any(not isinstance(arm, AutonomousPromptLearningArm) for arm in self.arms):
            raise ArgumentError("prompt learning arms are outside their bounds")
        arm_ids = [arm.arm_id for arm in self.arms]
        if len(set(arm_ids)) != len(arm_ids):
            raise ArgumentError("prompt learning state contains duplicate arms")
        if not isinstance(self.settlements, Sequence) or isinstance(self.settlements, (str, bytes, bytearray)) or len(self.settlements) > MAX_AUTONOMOUS_PROMPT_LEARNING_SETTLEMENTS:
            raise ArgumentError("prompt learning settlements are outside their bounds")
        object.__setattr__(self, "settlements", tuple(_settlement_record(item) for item in self.settlements))

    @classmethod
    def empty(cls, registry_digest: str) -> "AutonomousPromptLearningState":
        return cls(registry_digest=_digest("prompt learning registry_digest", registry_digest))

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_PROMPT_LEARNING_SCHEMA,
            "registry_digest": self.registry_digest,
            "generation": self.generation,
            "arms": [arm.to_dict() for arm in sorted(self.arms, key=lambda item: item.arm_id)],
            "settlements": [dict(item) for item in self.settlements],
            "retention": AUTONOMOUS_PROMPT_LEARNING_RETENTION,
            "secret_material": "never_returned",
        }

    @property
    def state_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "state_digest": self.state_digest}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousPromptLearningState":
        if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_PROMPT_LEARNING_SCHEMA:
            raise ArgumentError("prompt learning state schema is invalid")
        raw_arms = value.get("arms")
        if not isinstance(raw_arms, Sequence) or isinstance(raw_arms, (str, bytes)):
            raise ArgumentError("prompt learning state arms are malformed")
        arms = tuple(
            AutonomousPromptLearningArm(
                domain=item.get("domain"),
                stage=item.get("stage"),
                required_capabilities=tuple(item.get("required_capabilities", ())),
                prompt_id=item.get("prompt_id"),
                version=item.get("version"),
                manifest_digest=item.get("manifest_digest"),
                pulls=item.get("pulls", 0),
                failures=item.get("failures", 0),
                reward_sum=item.get("reward_sum", 0.0),
            )
            for item in raw_arms
            if isinstance(item, Mapping)
        )
        if len(arms) != len(raw_arms):
            raise ArgumentError("prompt learning state contains malformed arms")
        raw_settlements = value.get("settlements", ())
        if not isinstance(raw_settlements, Sequence) or isinstance(raw_settlements, (str, bytes)):
            raise ArgumentError("prompt learning state settlements are malformed")
        state = cls(
            registry_digest=value.get("registry_digest"),
            generation=value.get("generation", 0),
            arms=arms,
            settlements=tuple(dict(item) for item in raw_settlements if isinstance(item, Mapping)),
        )
        if len(state.settlements) != len(raw_settlements):
            raise ArgumentError("prompt learning state contains malformed settlements")
        if value.get("state_digest") is not None and value.get("state_digest") != state.state_digest:
            raise ArgumentError("prompt learning state digest does not match its contents")
        return state


@dataclass(frozen=True, slots=True)
class AutonomousPromptAdaptiveSelection:
    registry_digest: str
    generation: int
    plan: AutonomousPromptSelectionPlan
    arm_ids: tuple[str, ...]
    exploration: float

    def __post_init__(self) -> None:
        object.__setattr__(self, "registry_digest", _digest("adaptive prompt selection registry_digest", self.registry_digest))
        object.__setattr__(self, "generation", _integer("adaptive prompt selection generation", self.generation, 0, 2_147_483_647))
        if not isinstance(self.plan, AutonomousPromptSelectionPlan) or self.plan.registry_digest != self.registry_digest:
            raise ArgumentError("adaptive prompt selection plan is not bound to its registry")
        if not isinstance(self.arm_ids, Sequence) or isinstance(self.arm_ids, (str, bytes, bytearray)) or len(self.arm_ids) != len(self.plan.rows):
            raise ArgumentError("adaptive prompt selection arm ids are malformed")
        object.__setattr__(self, "arm_ids", tuple(_digest("adaptive prompt selection arm_id", arm_id) for arm_id in self.arm_ids))
        object.__setattr__(self, "exploration", _finite("adaptive prompt selection exploration", self.exploration, 0.0, MAX_AUTONOMOUS_PROMPT_LEARNING_EXPLORATION))

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_PROMPT_ADAPTIVE_SELECTION_SCHEMA,
            "registry_digest": self.registry_digest,
            "generation": self.generation,
            "plan_digest": self.plan.plan_digest,
            "arm_ids": list(self.arm_ids),
            "exploration": self.exploration,
            "selection_policy": AUTONOMOUS_PROMPT_LEARNING_POLICY,
        }

    @property
    def selection_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._descriptor(),
            "selection_digest": self.selection_digest,
            "plan": self.plan.to_dict(),
            "retention": "selection_metadata_only;rendered_messages_transient",
            "secret_material": "never_returned",
        }


def _state(value: AutonomousPromptLearningState | Mapping[str, Any] | None, registry: AutonomousPromptRegistry) -> AutonomousPromptLearningState:
    state = AutonomousPromptLearningState.empty(registry.registry_digest) if value is None else value if isinstance(value, AutonomousPromptLearningState) else AutonomousPromptLearningState.from_dict(value)
    if state.registry_digest != registry.registry_digest:
        raise ArgumentError("prompt learning state is stale for the current registry")
    for arm in state.arms:
        template = registry.template_for(arm.prompt_id)
        manifest = template.manifest
        if (
            manifest.domain != arm.domain
            or manifest.version != arm.version
            or manifest.manifest_digest != arm.manifest_digest
            or (arm.stage not in manifest.stages and "*" not in manifest.stages)
            or not set(arm.required_capabilities).issubset(manifest.capabilities)
        ):
            raise ArgumentError("prompt learning arm is stale for the current registry")
    return state


def select_adaptive_autonomous_prompts(
    registry: AutonomousPromptRegistry,
    requests: Sequence[Mapping[str, Any]],
    *,
    state: AutonomousPromptLearningState | Mapping[str, Any] | None = None,
    exploration: float = 0.35,
) -> AutonomousPromptAdaptiveSelection:
    """Select a verified prompt plan using deterministic UCB1 exploration."""

    if not isinstance(registry, AutonomousPromptRegistry):
        raise ArgumentError("adaptive prompt selection requires an AutonomousPromptRegistry")
    if not isinstance(requests, Sequence) or isinstance(requests, (str, bytes, bytearray)) or not 1 <= len(requests) <= 128:
        raise ArgumentError("adaptive prompt selection requests are outside their bounds")
    state_value = _state(state, registry)
    exploration = _finite("adaptive prompt selection exploration", exploration, 0.0, MAX_AUTONOMOUS_PROMPT_LEARNING_EXPLORATION)
    by_arm = {arm.arm_id: arm for arm in state_value.arms}
    rows: list[AutonomousPromptSelectionRow] = []
    selected_arm_ids: list[str] = []
    for index, request in enumerate(requests):
        if not isinstance(request, Mapping):
            raise ArgumentError(f"adaptive prompt selection request {index} is malformed")
        domain = _text(f"adaptive prompt selection request {index} domain", request.get("domain"))
        stage = _text(f"adaptive prompt selection request {index} stage", request.get("stage", request.get("stage_id")))
        required = _capabilities(tuple(request.get("required_capabilities", ())))
        candidates = registry.candidates(domain, stage, required)
        if not candidates:
            raise ArgumentError(f"no prompt template satisfies {domain}/{stage}")
        arms = []
        for template in candidates:
            manifest = template.manifest
            candidate_arm = AutonomousPromptLearningArm(domain, stage, required, manifest.prompt_id, manifest.version, manifest.manifest_digest)
            arms.append((template, candidate_arm))
        unpulled = [item for item in arms if item[1].arm_id not in by_arm or by_arm[item[1].arm_id].pulls == 0]
        if unpulled:
            selected_template, selected_arm = unpulled[0]
        else:
            total_pulls = max(1, sum(by_arm[item[1].arm_id].pulls for item in arms))
            selected_template, selected_arm = max(
                arms,
                key=lambda item: (
                    by_arm[item[1].arm_id].reward_sum / by_arm[item[1].arm_id].pulls
                    + exploration * math.sqrt(math.log(total_pulls + 1) / by_arm[item[1].arm_id].pulls),
                    -candidates.index(item[0]),
                ),
            )
        manifest = selected_template.manifest
        selected_arm_ids.append(selected_arm.arm_id)
        rows.append(
            AutonomousPromptSelectionRow(
                domain=domain,
                stage=stage,
                required_capabilities=required,
                selected_prompt_id=manifest.prompt_id,
                selected_version=manifest.version,
                selected_manifest_digest=manifest.manifest_digest,
                candidate_prompt_ids=tuple(item.manifest.prompt_id for item in candidates),
            )
        )
    plan = AutonomousPromptSelectionPlan(registry_digest=registry.registry_digest, rows=tuple(rows))
    return AutonomousPromptAdaptiveSelection(registry.registry_digest, state_value.generation, plan, tuple(selected_arm_ids), exploration)


@dataclass(frozen=True, slots=True)
class AutonomousPromptLearningSettlement:
    status: str
    next_state: AutonomousPromptLearningState
    selection_digest: str
    arm_id: str
    evaluator_id: str
    evaluator_version: str
    reward: float
    passed: bool
    outcome_digest: str
    idempotent_replay: bool

    @property
    def settlement_digest(self) -> str:
        return content_digest(self.to_dict())

    def to_dict(self) -> dict[str, Any]:
        body = {
            "schema": AUTONOMOUS_PROMPT_LEARNING_SETTLEMENT_SCHEMA,
            "status": self.status,
            "selection_digest": self.selection_digest,
            "arm_id": self.arm_id,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "reward": _json_number(self.reward),
            "passed": self.passed,
            "outcome_digest": self.outcome_digest,
            "idempotent_replay": self.idempotent_replay,
            "next_state_digest": self.next_state.state_digest,
            "retention": AUTONOMOUS_PROMPT_LEARNING_RETENTION,
            "secret_material": "never_returned",
        }
        return {**body, "settlement_digest": content_digest(body)}


def settle_autonomous_prompt_selection(
    registry: AutonomousPromptRegistry,
    state: AutonomousPromptLearningState | Mapping[str, Any],
    selection: AutonomousPromptAdaptiveSelection,
    *,
    arm_id: str,
    evaluator_id: str,
    evaluator_version: str,
    reward: float,
    passed: bool,
    outcome_digest: str | None = None,
    settlement_key: str | None = None,
) -> AutonomousPromptLearningSettlement:
    """Credit one selected prompt arm using explicit, replay-safe evaluator evidence."""

    if not isinstance(selection, AutonomousPromptAdaptiveSelection):
        raise ArgumentError("prompt learning selection is malformed")
    current = _state(state, registry)
    registry.verify_selection(selection.plan)
    if selection.registry_digest != current.registry_digest or arm_id not in selection.arm_ids:
        raise ArgumentError("prompt learning selection does not match the current state")
    arm_index = selection.arm_ids.index(arm_id)
    row = selection.plan.rows[arm_index]
    expected_arm_id = prompt_learning_arm_id(
        domain=row.domain,
        stage=row.stage,
        required_capabilities=row.required_capabilities,
        prompt_id=row.selected_prompt_id,
        version=row.selected_version,
        manifest_digest=row.selected_manifest_digest,
    )
    if expected_arm_id != arm_id:
        raise ArgumentError("prompt learning arm identity does not match its selection row")
    evaluator_id = _text("prompt learning evaluator_id", evaluator_id)
    evaluator_version = _text("prompt learning evaluator_version", evaluator_version, 128)
    reward = _finite("prompt learning reward", reward, -1.0, 1.0)
    if not isinstance(passed, bool):
        raise ArgumentError("prompt learning passed must be boolean")
    if outcome_digest is None:
        outcome_digest = content_digest({"selection_digest": selection.selection_digest, "arm_id": arm_id, "evaluator_id": evaluator_id, "evaluator_version": evaluator_version, "reward": _json_number(reward), "passed": passed})
    else:
        outcome_digest = _digest("prompt learning outcome_digest", outcome_digest)
    key = _digest("prompt learning settlement_key", settlement_key) if settlement_key is not None else content_digest({"arm_id": arm_id, "outcome_digest": outcome_digest, "evaluator_id": evaluator_id, "evaluator_version": evaluator_version})
    prior = next((item for item in current.settlements if item.get("settlement_key") == key), None)
    if prior is not None:
        if prior.get("outcome_digest") != outcome_digest or prior.get("arm_id") != arm_id:
            raise ArgumentError("prompt learning settlement key conflicts with prior evidence")
        return AutonomousPromptLearningSettlement("replayed", current, selection.selection_digest, arm_id, evaluator_id, evaluator_version, reward, passed, outcome_digest, True)
    existing = next((item for item in current.arms if item.arm_id == arm_id), None)
    if existing is None:
        existing = AutonomousPromptLearningArm(row.domain, row.stage, row.required_capabilities, row.selected_prompt_id, row.selected_version, row.selected_manifest_digest)
    updated = AutonomousPromptLearningArm(existing.domain, existing.stage, existing.required_capabilities, existing.prompt_id, existing.version, existing.manifest_digest, existing.pulls + 1, existing.failures + (0 if passed else 1), existing.reward_sum + reward)
    if len(current.settlements) >= MAX_AUTONOMOUS_PROMPT_LEARNING_SETTLEMENTS:
        raise ArgumentError("prompt learning settlement history is full")
    evidence = _settlement_record({"settlement_key": key, "arm_id": arm_id, "selection_digest": selection.selection_digest, "evaluator_id": evaluator_id, "evaluator_version": evaluator_version, "reward": _json_number(reward), "passed": passed, "outcome_digest": outcome_digest})
    next_state = AutonomousPromptLearningState(current.registry_digest, current.generation + 1, tuple(sorted((arm for arm in current.arms if arm.arm_id != arm_id), key=lambda item: item.arm_id)) + (updated,), current.settlements + (evidence,))
    return AutonomousPromptLearningSettlement("settled", next_state, selection.selection_digest, arm_id, evaluator_id, evaluator_version, reward, passed, outcome_digest, False)


__all__ = [
    "AUTONOMOUS_PROMPT_LEARNING_SCHEMA",
    "AUTONOMOUS_PROMPT_ADAPTIVE_SELECTION_SCHEMA",
    "AUTONOMOUS_PROMPT_LEARNING_SETTLEMENT_SCHEMA",
    "AUTONOMOUS_PROMPT_LEARNING_POLICY",
    "AUTONOMOUS_PROMPT_LEARNING_RETENTION",
    "AutonomousPromptLearningArm",
    "AutonomousPromptLearningState",
    "AutonomousPromptAdaptiveSelection",
    "AutonomousPromptLearningSettlement",
    "prompt_learning_arm_id",
    "select_adaptive_autonomous_prompts",
    "settle_autonomous_prompt_selection",
]
