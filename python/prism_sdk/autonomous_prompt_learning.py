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
import json
import math
import threading
from typing import Any, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
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
AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-prompt-learning-snapshot/0.1"
AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_RETENTION = "value_only_prompt_learning_state_snapshot"
MAX_AUTONOMOUS_PROMPT_LEARNING_ARMS = 4_096
MAX_AUTONOMOUS_PROMPT_LEARNING_SETTLEMENTS = 4_096
MAX_AUTONOMOUS_PROMPT_LEARNING_EXPLORATION = 2.0
MAX_AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_BYTES = 1_000_000


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

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousPromptAdaptiveSelection":
        """Rehydrate one metadata-only selection receipt for explicit evaluator settlement."""

        if not isinstance(value, Mapping):
            raise ArgumentError("adaptive prompt selection must be a mapping")
        expected = {
            "schema",
            "registry_digest",
            "generation",
            "plan_digest",
            "arm_ids",
            "exploration",
            "selection_policy",
            "selection_digest",
            "plan",
            "retention",
            "secret_material",
        }
        if set(value) != expected or value.get("schema") != AUTONOMOUS_PROMPT_ADAPTIVE_SELECTION_SCHEMA:
            raise ArgumentError("adaptive prompt selection fields are invalid")
        if value.get("selection_policy") != AUTONOMOUS_PROMPT_LEARNING_POLICY:
            raise ArgumentError("adaptive prompt selection policy is invalid")
        if value.get("retention") != "selection_metadata_only;rendered_messages_transient" or value.get("secret_material") != "never_returned":
            raise ArgumentError("adaptive prompt selection retention markers are invalid")
        raw_plan = value.get("plan")
        if not isinstance(raw_plan, Mapping):
            raise ArgumentError("adaptive prompt selection plan is malformed")
        plan = AutonomousPromptSelectionPlan.from_dict(raw_plan)
        if value.get("plan_digest") != plan.plan_digest:
            raise ArgumentError("adaptive prompt selection plan digest does not match its contents")
        arm_ids = value.get("arm_ids")
        if not isinstance(arm_ids, Sequence) or isinstance(arm_ids, (str, bytes, bytearray)):
            raise ArgumentError("adaptive prompt selection arm ids are malformed")
        selection = cls(
            registry_digest=value.get("registry_digest"),
            generation=value.get("generation"),
            plan=plan,
            arm_ids=tuple(arm_ids),
            exploration=value.get("exploration"),
        )
        if value.get("selection_digest") != selection.selection_digest:
            raise ArgumentError("adaptive prompt selection digest does not match its contents")
        return selection


def extract_autonomous_prompt_learning_selections(
    result: Any,
    registry: AutonomousPromptRegistry,
) -> tuple[AutonomousPromptAdaptiveSelection, ...]:
    """Extract exact adaptive selections from a run envelope without traversing provider values.

    High-level direct, cross-domain, workflow, and replan envelopes have different shapes. This
    bounded structural walker follows only reviewed result fields and the metadata-only prompt
    projection. It never calls ``to_dict`` on an arbitrary provider result and never inspects
    response, task, message, credential, or connector payloads.
    """

    if not isinstance(registry, AutonomousPromptRegistry):
        raise ArgumentError("prompt learning selection extraction requires an AutonomousPromptRegistry")
    found: list[AutonomousPromptAdaptiveSelection] = []
    seen: set[str] = set()
    visited: set[int] = set()
    max_nodes = 512

    def add(raw: Any) -> None:
        if not isinstance(raw, Mapping):
            raise ArgumentError("run adaptive prompt selection receipt is malformed")
        selection = AutonomousPromptAdaptiveSelection.from_dict(raw)
        registry.verify_selection(selection.plan)
        if selection.selection_digest not in seen:
            seen.add(selection.selection_digest)
            found.append(selection)
        if len(found) > 128:
            raise ArgumentError("run adaptive prompt selection receipts exceed their bound")

    def inspect_prompt(prompt: Mapping[str, Any]) -> None:
        adaptive = prompt.get("adaptive_selection")
        if isinstance(adaptive, Mapping):
            add(adaptive)
        autonomous_prompt = prompt.get("autonomous_prompt")
        if isinstance(autonomous_prompt, Mapping):
            adaptive = autonomous_prompt.get("adaptive_selection")
            if isinstance(adaptive, Mapping):
                add(adaptive)

    def visit(value: Any) -> None:
        if len(visited) >= max_nodes:
            raise ArgumentError("run adaptive prompt selection envelope is too deep")
        if isinstance(value, AutonomousPromptAdaptiveSelection):
            add(value.to_dict())
            return
        if isinstance(value, Mapping):
            identity = id(value)
            if identity in visited:
                return
            visited.add(identity)
            prompt = value.get("prompt")
            if isinstance(prompt, Mapping):
                inspect_prompt(prompt)
            for key in (
                "child_runs",
                "child_results",
                "synthesis",
                "synthesis_result",
                "cross_domain",
                "attempts",
                "final_result",
                "result",
                "stage_results",
            ):
                child = value.get(key)
                if isinstance(child, (Mapping, Sequence)) and not isinstance(child, (str, bytes, bytearray)):
                    visit(child)
            if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
                for child in value:
                    visit(child)
            return
        if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
            identity = id(value)
            if identity in visited:
                return
            visited.add(identity)
            for child in value:
                visit(child)
            return
        prompt = getattr(value, "prompt", None)
        if isinstance(prompt, Mapping):
            inspect_prompt(prompt)
        for attribute in (
            "child_runs",
            "child_results",
            "synthesis",
            "synthesis_result",
            "cross_domain",
            "attempts",
            "final_result",
            "result",
            "stage_results",
        ):
            child = getattr(value, attribute, None)
            if isinstance(child, (Mapping, Sequence)) or hasattr(child, "prompt"):
                visit(child)

    visit(result)
    return tuple(found)


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


@dataclass(frozen=True, slots=True)
class AutonomousPromptLearningSnapshot:
    """Restart image for prompt learning; only registry-bound value metadata is retained."""

    state: AutonomousPromptLearningState
    snapshot_generation: int = 1
    previous_snapshot_digest: str | None = None
    retention: str = AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_RETENTION
    secret_material: str = "never_returned"
    snapshot_digest: str = ""

    def __post_init__(self) -> None:
        if not isinstance(self.state, AutonomousPromptLearningState):
            raise ArgumentError("prompt learning snapshot state is malformed")
        _integer("prompt learning snapshot_generation", self.snapshot_generation, 1, 2_147_483_647)
        previous = None if self.previous_snapshot_digest is None else _digest(
            "prompt learning previous_snapshot_digest", self.previous_snapshot_digest
        )
        object.__setattr__(self, "previous_snapshot_digest", previous)
        if self.retention != AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_RETENTION or self.secret_material != "never_returned":
            raise ArgumentError("prompt learning snapshot retention markers are invalid")
        if (self.snapshot_generation == 1) != (previous is None):
            raise ArgumentError("prompt learning snapshot generation chain is malformed")
        descriptor = self._descriptor()
        expected = content_digest(descriptor)
        if self.snapshot_digest:
            if _digest("prompt learning snapshot_digest", self.snapshot_digest) != expected:
                raise ArgumentError("prompt learning snapshot digest does not match its contents")
        else:
            object.__setattr__(self, "snapshot_digest", expected)

    @property
    def registry_digest(self) -> str:
        return self.state.registry_digest

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_SCHEMA,
            "snapshot_generation": self.snapshot_generation,
            "previous_snapshot_digest": self.previous_snapshot_digest,
            "state": self.state.to_dict(),
            "retention": self.retention,
            "secret_material": self.secret_material,
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "snapshot_digest": self.snapshot_digest}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousPromptLearningSnapshot":
        if not isinstance(value, Mapping):
            raise ArgumentError("prompt learning snapshot must be a mapping")
        expected = {
            "schema", "snapshot_generation", "previous_snapshot_digest", "state",
            "retention", "secret_material", "snapshot_digest",
        }
        if set(value) != expected or value.get("schema") != AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_SCHEMA:
            raise ArgumentError("prompt learning snapshot fields are invalid")
        raw_state = value.get("state")
        if not isinstance(raw_state, Mapping):
            raise ArgumentError("prompt learning snapshot state is malformed")
        return cls(
            state=AutonomousPromptLearningState.from_dict(raw_state),
            snapshot_generation=value.get("snapshot_generation"),
            previous_snapshot_digest=value.get("previous_snapshot_digest"),
            retention=value.get("retention"),
            secret_material=value.get("secret_material"),
            snapshot_digest=value.get("snapshot_digest"),
        )


def snapshot_autonomous_prompt_learning(
    state: AutonomousPromptLearningState,
    *,
    snapshot_generation: int = 1,
    previous_snapshot_digest: str | None = None,
) -> AutonomousPromptLearningSnapshot:
    if not isinstance(state, AutonomousPromptLearningState):
        raise ArgumentError("prompt learning snapshot requires a typed state")
    return AutonomousPromptLearningSnapshot(
        state=state,
        snapshot_generation=snapshot_generation,
        previous_snapshot_digest=previous_snapshot_digest,
    )


class AutonomousPromptLearningSnapshotPersistence(Protocol):
    def read(self) -> AutonomousPromptLearningSnapshot | Mapping[str, Any] | None: ...
    def write(self, snapshot: AutonomousPromptLearningSnapshot | Mapping[str, Any]) -> None: ...


class AutonomousPromptLearningTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class AutonomousPromptLearningTransactionalTextStore(AutonomousPromptLearningTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class JsonAutonomousPromptLearningSnapshotPersistence:
    """Canonical JSON persistence over a caller-owned text store."""

    def __init__(self, store: AutonomousPromptLearningTextStore, *, max_bytes: int = MAX_AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise ArgumentError("prompt learning JSON persistence requires a text store")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= MAX_AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_BYTES:
            raise ArgumentError("prompt learning JSON persistence max_bytes is outside its bound")
        self.store = store
        self.max_bytes = max_bytes

    def read(self) -> AutonomousPromptLearningSnapshot | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("prompt learning JSON exceeds its byte bound")
        try:
            raw = json.loads(encoded)
        except json.JSONDecodeError as error:
            raise ArgumentError("prompt learning JSON is invalid") from error
        if not isinstance(raw, Mapping):
            raise ArgumentError("prompt learning JSON must be an object")
        snapshot = AutonomousPromptLearningSnapshot.from_dict(raw)
        if encoded != canonical_json(snapshot.to_dict()):
            raise ArgumentError("prompt learning JSON is not canonical")
        return snapshot

    def write(self, snapshot: AutonomousPromptLearningSnapshot | Mapping[str, Any]) -> None:
        normalized = snapshot if isinstance(snapshot, AutonomousPromptLearningSnapshot) else AutonomousPromptLearningSnapshot.from_dict(snapshot)
        encoded = canonical_json(normalized.to_dict())
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("prompt learning JSON exceeds its byte bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousPromptLearningSnapshotPersistence(JsonAutonomousPromptLearningSnapshotPersistence):
    """Canonical JSON persistence with compare-and-swap writer fencing."""

    def __init__(self, store: AutonomousPromptLearningTransactionalTextStore, *, max_bytes: int = MAX_AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_BYTES) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ArgumentError("prompt learning transactional persistence requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: AutonomousPromptLearningSnapshot | Mapping[str, Any]) -> bool:
        if expected_snapshot_digest is not None:
            _digest("prompt learning expected_snapshot_digest", expected_snapshot_digest)
        normalized = snapshot if isinstance(snapshot, AutonomousPromptLearningSnapshot) else AutonomousPromptLearningSnapshot.from_dict(snapshot)
        encoded = canonical_json(normalized.to_dict())
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("prompt learning JSON exceeds its byte bound")
        return bool(self.store.write_if_unchanged(expected_snapshot_digest, encoded))


class AutonomousPromptLearningPersistenceCoordinator:
    """Serialize selection settlement and restart persistence for one prompt learner."""

    def __init__(
        self,
        registry: AutonomousPromptRegistry,
        *,
        state: AutonomousPromptLearningState | Mapping[str, Any] | None = None,
        persistence: AutonomousPromptLearningSnapshotPersistence | None = None,
    ) -> None:
        if not isinstance(registry, AutonomousPromptRegistry):
            raise ArgumentError("prompt learning persistence requires an AutonomousPromptRegistry")
        self.registry = registry
        self._state = _state(state, registry)
        if persistence is not None and not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise ArgumentError("prompt learning persistence adapter is malformed")
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None
        self._snapshot_generation = 0
        self._lock = threading.RLock()

    @property
    def state(self) -> AutonomousPromptLearningState:
        with self._lock:
            return self._state

    def select(self, requests: Sequence[Mapping[str, Any]], *, exploration: float = 0.35) -> AutonomousPromptAdaptiveSelection:
        with self._lock:
            return select_adaptive_autonomous_prompts(self.registry, requests, state=self._state, exploration=exploration)

    def restore(self) -> AutonomousPromptLearningSnapshot | None:
        if self.persistence is None:
            raise ArgumentError("prompt learning restore requires persistence")
        with self._lock:
            raw = self.persistence.read()
            if raw is None:
                self._expected_snapshot_digest = None
                self._snapshot_generation = 0
                return None
            snapshot = raw if isinstance(raw, AutonomousPromptLearningSnapshot) else AutonomousPromptLearningSnapshot.from_dict(raw)
            if snapshot.registry_digest != self.registry.registry_digest:
                raise ArgumentError("prompt learning snapshot is stale for the current registry")
            self._state = _state(snapshot.state, self.registry)
            self._expected_snapshot_digest = snapshot.snapshot_digest
            self._snapshot_generation = snapshot.snapshot_generation
            return snapshot

    def flush(self) -> AutonomousPromptLearningSnapshot:
        if self.persistence is None:
            raise ArgumentError("prompt learning flush requires persistence")
        with self._lock:
            snapshot = snapshot_autonomous_prompt_learning(
                self._state,
                snapshot_generation=self._snapshot_generation + 1,
                previous_snapshot_digest=None if self._snapshot_generation == 0 else self._expected_snapshot_digest,
            )
            write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
            if callable(write_if_unchanged):
                if not write_if_unchanged(self._expected_snapshot_digest, snapshot):
                    raise ArgumentError("prompt learning persistence compare-and-swap conflict")
            else:
                self.persistence.write(snapshot)
            self._expected_snapshot_digest = snapshot.snapshot_digest
            self._snapshot_generation = snapshot.snapshot_generation
            return snapshot

    def settle(self, selection: AutonomousPromptAdaptiveSelection, **kwargs: Any) -> AutonomousPromptLearningSettlement:
        with self._lock:
            settlement = settle_autonomous_prompt_selection(self.registry, self._state, selection, **kwargs)
            if settlement.status == "replayed":
                return settlement
            next_state = settlement.next_state
            if self.persistence is not None:
                prior_state = self._state
                self._state = next_state
                try:
                    self.flush()
                except Exception:
                    self._state = prior_state
                    raise
            else:
                self._state = next_state
            return settlement


__all__ = [
    "AUTONOMOUS_PROMPT_LEARNING_SCHEMA",
    "AUTONOMOUS_PROMPT_ADAPTIVE_SELECTION_SCHEMA",
    "AUTONOMOUS_PROMPT_LEARNING_SETTLEMENT_SCHEMA",
    "AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_SCHEMA",
    "AUTONOMOUS_PROMPT_LEARNING_POLICY",
    "AUTONOMOUS_PROMPT_LEARNING_RETENTION",
    "AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_RETENTION",
    "MAX_AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_BYTES",
    "AutonomousPromptLearningArm",
    "AutonomousPromptLearningState",
    "AutonomousPromptAdaptiveSelection",
    "AutonomousPromptLearningSettlement",
    "AutonomousPromptLearningSnapshot",
    "snapshot_autonomous_prompt_learning",
    "extract_autonomous_prompt_learning_selections",
    "AutonomousPromptLearningSnapshotPersistence",
    "AutonomousPromptLearningTextStore",
    "AutonomousPromptLearningTransactionalTextStore",
    "JsonAutonomousPromptLearningSnapshotPersistence",
    "TransactionalJsonAutonomousPromptLearningSnapshotPersistence",
    "AutonomousPromptLearningPersistenceCoordinator",
    "prompt_learning_arm_id",
    "select_adaptive_autonomous_prompts",
    "settle_autonomous_prompt_selection",
]
