"""Crash-safe, metadata-only persistence for the outer autonomous goal loop.

The goal ledger and worker journal already protect objective state and effect settlement.  This
module protects the *decision process around them*: cycle numbering, bounded counters, evaluator
digest history, and the value-only contextual goal bandit.  A checkpoint is deliberately not executable
state.  It contains no task text, prompts, provider output, tool arguments, credentials, or live
callbacks; callers rehydrate those through the normal worker/agent seams after a successful
claim.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
import json
import re
from typing import Any, Protocol

from .authoring import canonical_json, content_digest
from .goals import AutonomousGoalError


AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA = "bioprism-autonomous-goal-control-checkpoint/0.1"
AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_RETENTION = "metadata_only_goal_control_checkpoint;tasks_prompts_parameters_credentials_and_results_not_retained"
AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES = 128
AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_RUNS = 8_192
AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_EVALUATIONS = 128
AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SIGNALS = 4_096
AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SNAPSHOT_BYTES = 2_000_000
_BANDIT_SCHEMA = "bioprism-autonomous-goal-control-bandit/0.1"
_BANDIT_RETENTIONS = frozenset({"value_only_goal_domain_bandit_state", "value_only_goal_contextual_bandit_state"})
_STOP_REASONS = {"all_terminal", "no_admissible_work", "cycle_budget_exhausted", "run_budget_exhausted"}
_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_IDENTIFIER = re.compile(r"^[A-Za-z0-9_.:/-]+$")


def _fail(message: str) -> None:
    raise AutonomousGoalError(f"autonomous goal control checkpoint {message}")


def _integer(value: Any, *, name: str, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum or value > maximum:
        _fail(f"{name} is outside its integer bounds")
    return value


def _text(value: Any, *, name: str, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        _fail(f"{name} is outside its text bounds")
    return value.strip()


def _identifier(value: Any, *, name: str, maximum: int = 256) -> str:
    text = _text(value, name=name, maximum=maximum)
    if not _IDENTIFIER.fullmatch(text):
        _fail(f"{name} contains unsupported identifier characters")
    return text


def _digest(value: Any, *, name: str, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or _DIGEST.fullmatch(value) is None:
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _number(value: Any, *, name: str, minimum: float, maximum: float) -> float | int:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        _fail(f"{name} is outside its numeric bounds")
    number = float(value)
    if number != number or number in {float("inf"), float("-inf")} or number < minimum or number > maximum:
        _fail(f"{name} is outside its numeric bounds")
    return int(value) if isinstance(value, float) and value.is_integer() else value


def _keys(value: Mapping[str, Any], expected: set[str], *, name: str) -> None:
    if set(value) != expected:
        _fail(f"{name} has unsupported or missing fields")


def _counts(value: Any, *, name: str, maximum: int) -> dict[str, int]:
    if not isinstance(value, Mapping) or len(value) > 128:
        _fail(f"{name} is malformed")
    result: dict[str, int] = {}
    for key, raw in value.items():
        result[_identifier(key, name=f"{name} key", maximum=128)] = _integer(raw, name=f"{name} value", minimum=0, maximum=maximum)
    return dict(sorted(result.items()))


def _signal(value: Any, *, index: int) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail(f"signal {index} is malformed")
    expected = {"goal_id", "priority", "urgency", "deadline_ns", "estimated_cost", "dependencies"}
    _keys(value, expected, name=f"signal {index}")
    dependencies = value["dependencies"]
    if not isinstance(dependencies, Sequence) or isinstance(dependencies, (str, bytes, bytearray)) or len(dependencies) > 64:
        _fail(f"signal {index} dependencies are outside their bounds")
    normalized_dependencies = sorted({_identifier(item, name=f"signal {index} dependency") for item in dependencies})
    deadline = value["deadline_ns"]
    if deadline is not None:
        deadline = _integer(deadline, name=f"signal {index} deadline_ns", minimum=0, maximum=2**63 - 1)
    return {
        "goal_id": _identifier(value["goal_id"], name=f"signal {index} goal_id"),
        "priority": _number(value["priority"], name=f"signal {index} priority", minimum=0, maximum=1),
        "urgency": _number(value["urgency"], name=f"signal {index} urgency", minimum=0, maximum=1),
        "deadline_ns": deadline,
        "estimated_cost": _integer(value["estimated_cost"], name=f"signal {index} estimated_cost", minimum=1, maximum=1_000_000),
        "dependencies": normalized_dependencies,
    }


def _cycle_summary(value: Any, *, name: str) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail(f"{name} is malformed")
    expected = {
        "cycle", "schedule_digest", "claim_digest", "worker_digest", "selected", "claimed", "runs", "counts",
        "selected_domains", "missing_domains", "retention", "secret_material",
    }
    optional = {"evaluated", "evaluation_digest", "learning_state_digest", "signals_digest"}
    if set(value).difference(expected | optional) or not expected.issubset(value):
        _fail(f"{name} has unsupported or missing fields")
    if value["retention"] != "metadata_only_goal_control;tasks_prompts_parameters_credentials_and_results_not_retained" or value["secret_material"] != "never_returned":
        _fail(f"{name} retention markers are invalid")
    cycle = _integer(value["cycle"], name=f"{name}.cycle", minimum=1, maximum=AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES)
    selected = _integer(value["selected"], name=f"{name}.selected", minimum=0, maximum=128)
    claimed = _integer(value["claimed"], name=f"{name}.claimed", minimum=0, maximum=128)
    runs = _integer(value["runs"], name=f"{name}.runs", minimum=0, maximum=128)
    if claimed > selected or runs > claimed:
        _fail(f"{name} counts are inconsistent")
    counts = _counts(value["counts"], name=f"{name}.counts", maximum=128)
    for required in ("selected", "claimed", "settled", "completed", "paused", "blocked", "failed"):
        if counts.get(required, 0) < 0:
            _fail(f"{name}.counts is invalid")
    selected_domains = value["selected_domains"]
    missing_domains = value["missing_domains"]
    if not isinstance(selected_domains, Sequence) or isinstance(selected_domains, (str, bytes, bytearray)) or len(selected_domains) > 128:
        _fail(f"{name}.selected_domains is malformed")
    if not isinstance(missing_domains, Sequence) or isinstance(missing_domains, (str, bytes, bytearray)) or len(missing_domains) > 128:
        _fail(f"{name}.missing_domains is malformed")
    result: dict[str, Any] = {
        "cycle": cycle,
        "schedule_digest": _digest(value["schedule_digest"], name=f"{name}.schedule_digest"),
        "claim_digest": _digest(value["claim_digest"], name=f"{name}.claim_digest", allow_none=True),
        "worker_digest": _digest(value["worker_digest"], name=f"{name}.worker_digest"),
        "selected": selected,
        "claimed": claimed,
        "runs": runs,
        "counts": counts,
        "selected_domains": [_identifier(item, name=f"{name}.selected_domain", maximum=128) for item in selected_domains],
        "missing_domains": [_identifier(item, name=f"{name}.missing_domain", maximum=128) for item in missing_domains],
        "retention": value["retention"],
        "secret_material": value["secret_material"],
    }
    if "evaluated" in value:
        result["evaluated"] = _integer(value["evaluated"], name=f"{name}.evaluated", minimum=0, maximum=128)
        result["evaluation_digest"] = _digest(value.get("evaluation_digest"), name=f"{name}.evaluation_digest")
    for field in ("learning_state_digest", "signals_digest"):
        if field in value:
            result[field] = _digest(value[field], name=f"{name}.{field}")
    return result


def _bandit(value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail("learner_state must be a mapping or null")
    expected = {"schema", "generation", "arms", "exploration", "retention", "secret_material", "state_digest"}
    _keys(value, expected, name="learner_state")
    if value["schema"] != _BANDIT_SCHEMA or value["retention"] not in _BANDIT_RETENTIONS or value["secret_material"] != "never_returned":
        _fail("learner_state markers are invalid")
    generation = _integer(value["generation"], name="learner_state.generation", minimum=0, maximum=2**31 - 1)
    exploration = _number(value["exploration"], name="learner_state.exploration", minimum=0, maximum=2)
    arms = value["arms"]
    if not isinstance(arms, Sequence) or isinstance(arms, (str, bytes, bytearray)) or len(arms) > 128:
        _fail("learner_state arms are outside their bounds")
    normalized_arms: list[dict[str, Any]] = []
    seen: set[str] = set()
    for index, raw in enumerate(arms):
        if not isinstance(raw, Mapping):
            _fail(f"learner_state arm {index} is malformed")
        required_arm_fields = {"domain", "pulls", "failures", "reward_sum"}
        optional_context_fields = {"capability", "risk_class", "arm_id"}
        if set(raw).difference(required_arm_fields | optional_context_fields) or not required_arm_fields.issubset(raw):
            _fail(f"learner_state arm {index} has unsupported or missing fields")
        domain = _identifier(raw["domain"], name=f"learner_state arm {index}.domain", maximum=128)
        capability = None if raw.get("capability") is None else _text(raw["capability"], name=f"learner_state arm {index}.capability", maximum=128)
        risk_class = None if raw.get("risk_class") is None else _text(raw["risk_class"], name=f"learner_state arm {index}.risk_class", maximum=128)
        arm_id = raw.get("arm_id")
        expected_arm_id = domain if capability is None and risk_class is None else content_digest(
            {
                "schema": f"{_BANDIT_SCHEMA}/context-arm",
                "domain": domain,
                "capability": capability,
                "risk_class": risk_class,
            }
        )
        if arm_id is not None:
            arm_id = _digest(arm_id, name=f"learner_state arm {index}.arm_id")
        if arm_id != expected_arm_id and not (arm_id is None and expected_arm_id == domain):
            _fail(f"learner_state arm {index}.arm_id does not match its context")
        arm_key = expected_arm_id
        if arm_key in seen:
            _fail("learner_state contains duplicate contextual arms")
        seen.add(arm_key)
        pulls = _integer(raw["pulls"], name=f"learner_state arm {index}.pulls", minimum=0, maximum=2**31 - 1)
        failures = _integer(raw["failures"], name=f"learner_state arm {index}.failures", minimum=0, maximum=2**31 - 1)
        if failures > pulls:
            _fail(f"learner_state arm {index} failures exceed pulls")
        reward_sum = _number(raw["reward_sum"], name=f"learner_state arm {index}.reward_sum", minimum=-pulls, maximum=pulls)
        normalized_arm: dict[str, Any] = {"domain": domain, "pulls": pulls, "failures": failures, "reward_sum": reward_sum}
        if capability is not None or risk_class is not None:
            normalized_arm["capability"] = capability
            normalized_arm["risk_class"] = risk_class
            normalized_arm["arm_id"] = arm_key
        normalized_arms.append(normalized_arm)
    normalized_arms.sort(key=lambda arm: arm.get("arm_id", arm["domain"]))
    body = {
        "schema": value["schema"],
        "generation": generation,
        "arms": normalized_arms,
        "exploration": exploration,
        "retention": value["retention"],
        "secret_material": value["secret_material"],
    }
    if _digest(value["state_digest"], name="learner_state.state_digest") != content_digest(body):
        _fail("learner_state digest mismatch")
    return {**body, "state_digest": value["state_digest"]}


def _normalize(value: Mapping[str, Any], *, require_digest: bool) -> dict[str, Any]:
    required = {
        "schema", "run_id", "next_cycle", "cycle_summaries", "previous_cycle", "completed_cycles", "total_selected",
        "total_claimed", "total_runs", "status_counts", "domain_counts", "evaluation_count", "evaluation_digests",
        "learning_state_digest", "learned_signals", "learner_state", "stop_reason", "generation", "previous_snapshot_digest",
        "retention", "secret_material",
    }
    allowed = required | {"snapshot_digest"}
    if not isinstance(value, Mapping) or set(value).difference(allowed) or not required.issubset(value) or (require_digest and "snapshot_digest" not in value):
        _fail("snapshot has unsupported or missing fields")
    if value["schema"] != AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA or value["retention"] != AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_RETENTION or value["secret_material"] != "never_returned":
        _fail("snapshot markers are invalid")
    run_id = _identifier(value["run_id"], name="run_id")
    next_cycle = _integer(value["next_cycle"], name="next_cycle", minimum=1, maximum=AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES + 1)
    completed_cycles = _integer(value["completed_cycles"], name="completed_cycles", minimum=0, maximum=AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES)
    if next_cycle != completed_cycles + 1:
        _fail("next_cycle is not bound to completed_cycles")
    raw_summaries = value["cycle_summaries"]
    if not isinstance(raw_summaries, Sequence) or isinstance(raw_summaries, (str, bytes, bytearray)) or len(raw_summaries) > AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES:
        _fail("cycle_summaries exceed capacity")
    summaries = [_cycle_summary(raw, name=f"cycle_summaries[{index}]") for index, raw in enumerate(raw_summaries)]
    if len(summaries) != completed_cycles or [row["cycle"] for row in summaries] != list(range(1, completed_cycles + 1)):
        _fail("cycle_summaries are not contiguous")
    previous = value["previous_cycle"]
    normalized_previous = None if previous is None else _cycle_summary(previous, name="previous_cycle")
    if (summaries[-1] if summaries else None) != normalized_previous:
        _fail("previous_cycle is not bound to cycle_summaries")
    total_selected = _integer(value["total_selected"], name="total_selected", minimum=0, maximum=AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_RUNS)
    total_claimed = _integer(value["total_claimed"], name="total_claimed", minimum=0, maximum=AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_RUNS)
    total_runs = _integer(value["total_runs"], name="total_runs", minimum=0, maximum=AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_RUNS)
    if total_claimed > total_selected or total_runs > total_claimed:
        _fail("aggregate counts are inconsistent")
    evaluation_count = _integer(value["evaluation_count"], name="evaluation_count", minimum=0, maximum=AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_EVALUATIONS * AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES)
    raw_evaluation_digests = value["evaluation_digests"]
    if not isinstance(raw_evaluation_digests, Sequence) or isinstance(raw_evaluation_digests, (str, bytes, bytearray)) or len(raw_evaluation_digests) > AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES:
        _fail("evaluation_digests are outside their bounds")
    evaluation_digests = [_digest(item, name="evaluation_digest") for item in raw_evaluation_digests]
    learning_state_digest = _digest(value["learning_state_digest"], name="learning_state_digest", allow_none=True)
    raw_signals = value["learned_signals"]
    if not isinstance(raw_signals, Sequence) or isinstance(raw_signals, (str, bytes, bytearray)) or len(raw_signals) > AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SIGNALS:
        _fail("learned_signals are outside their bounds")
    signals = [_signal(raw, index=index) for index, raw in enumerate(raw_signals)]
    learner_state = None if value["learner_state"] is None else _bandit(value["learner_state"])
    if value["stop_reason"] not in _STOP_REASONS:
        _fail("stop_reason is invalid")
    generation = _integer(value["generation"], name="generation", minimum=1, maximum=2**31 - 1)
    previous_snapshot_digest = _digest(value["previous_snapshot_digest"], name="previous_snapshot_digest", allow_none=True)
    body = {
        "schema": value["schema"], "run_id": run_id, "next_cycle": next_cycle, "cycle_summaries": summaries,
        "previous_cycle": normalized_previous, "completed_cycles": completed_cycles, "total_selected": total_selected,
        "total_claimed": total_claimed, "total_runs": total_runs, "status_counts": _counts(value["status_counts"], name="status_counts", maximum=AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_RUNS),
        "domain_counts": _counts(value["domain_counts"], name="domain_counts", maximum=AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_RUNS),
        "evaluation_count": evaluation_count, "evaluation_digests": evaluation_digests, "learning_state_digest": learning_state_digest,
        "learned_signals": signals, "learner_state": learner_state, "stop_reason": value["stop_reason"], "generation": generation,
        "previous_snapshot_digest": previous_snapshot_digest, "retention": value["retention"], "secret_material": value["secret_material"],
    }
    if require_digest:
        supplied = _digest(value.get("snapshot_digest"), name="snapshot_digest")
        if supplied != content_digest(body):
            _fail("snapshot digest mismatch")
        body["snapshot_digest"] = supplied
    return body


def seal_autonomous_goal_control_loop_snapshot(descriptor: Mapping[str, Any]) -> dict[str, Any]:
    """Validate and hash a new checkpoint descriptor before it crosses a persistence boundary."""

    normalized = _normalize(descriptor, require_digest=False)
    snapshot = {**normalized, "snapshot_digest": content_digest(normalized)}
    if len(canonical_json(snapshot).encode("utf-8")) > AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SNAPSHOT_BYTES:
        _fail("snapshot exceeds its byte bound")
    return snapshot


def validate_autonomous_goal_control_loop_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    """Strictly validate a checkpoint, including its content digest and retention posture."""

    normalized = _normalize(value, require_digest=True)
    if len(canonical_json(normalized).encode("utf-8")) > AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SNAPSHOT_BYTES:
        _fail("snapshot exceeds its byte bound")
    return normalized


class AutonomousGoalControlLoopSnapshotTextStore(Protocol):
    def read(self) -> str | None: ...

    def write(self, value: str) -> None: ...


class TransactionalAutonomousGoalControlLoopSnapshotTextStore(AutonomousGoalControlLoopSnapshotTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class JsonAutonomousGoalControlLoopSnapshotPersistence:
    """Canonical JSON adapter for a caller-owned checkpoint text store."""

    def __init__(self, store: AutonomousGoalControlLoopSnapshotTextStore, *, max_bytes: int = AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SNAPSHOT_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            _fail("JSON persistence requires a text store")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SNAPSHOT_BYTES:
            _fail("JSON persistence max_bytes is outside its bound")
        self.store = store
        self.max_bytes = max_bytes

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("stored JSON exceeds its byte bound")
        try:
            raw = json.loads(encoded)
        except (TypeError, ValueError, json.JSONDecodeError) as error:
            raise AutonomousGoalError("autonomous goal control checkpoint JSON is invalid") from error
        if not isinstance(raw, Mapping):
            _fail("stored JSON must be an object")
        normalized = validate_autonomous_goal_control_loop_snapshot(raw)
        if canonical_json(normalized) != encoded:
            _fail("stored JSON is not canonical")
        return normalized

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized = validate_autonomous_goal_control_loop_snapshot(snapshot)
        encoded = canonical_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("snapshot exceeds the configured byte bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousGoalControlLoopSnapshotPersistence(JsonAutonomousGoalControlLoopSnapshotPersistence):
    """Canonical JSON checkpoint adapter with stale-writer fencing."""

    def __init__(self, store: TransactionalAutonomousGoalControlLoopSnapshotTextStore, *, max_bytes: int = AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SNAPSHOT_BYTES) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            _fail("transactional JSON persistence requires write_if_unchanged")

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        if expected_snapshot_digest is not None:
            _digest(expected_snapshot_digest, name="expected_snapshot_digest")
        normalized = validate_autonomous_goal_control_loop_snapshot(snapshot)
        return self.store.write_if_unchanged(expected_snapshot_digest, canonical_json(normalized))


class AutonomousGoalControlLoopPersistenceCoordinator:
    """Restore and flush loop checkpoints with generation and compare-and-swap fencing."""

    def __init__(self, persistence: Any) -> None:
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            _fail("persistence adapter is malformed")
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None
        self._expected_generation = 0

    @property
    def expected_snapshot_digest(self) -> str | None:
        return self._expected_snapshot_digest

    def restore(self) -> dict[str, Any] | None:
        raw = self.persistence.read()
        if raw is None:
            self._expected_snapshot_digest = None
            self._expected_generation = 0
            return None
        snapshot = validate_autonomous_goal_control_loop_snapshot(raw)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        self._expected_generation = snapshot["generation"]
        return snapshot

    def flush(self, snapshot: Mapping[str, Any]) -> dict[str, Any]:
        normalized = validate_autonomous_goal_control_loop_snapshot(snapshot)
        if normalized["generation"] != self._expected_generation + 1:
            _fail("checkpoint generation is not contiguous")
        if normalized["previous_snapshot_digest"] != self._expected_snapshot_digest:
            _fail("checkpoint previous digest does not match the restored head")
        write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
        if callable(write_if_unchanged):
            if not write_if_unchanged(self._expected_snapshot_digest, normalized):
                _fail("persistence compare-and-swap conflict")
        else:
            self.persistence.write(normalized)
        self._expected_snapshot_digest = normalized["snapshot_digest"]
        self._expected_generation = normalized["generation"]
        return normalized


__all__ = [
    "AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA",
    "AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_RETENTION",
    "AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES",
    "AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_RUNS",
    "AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_EVALUATIONS",
    "AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SIGNALS",
    "AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SNAPSHOT_BYTES",
    "AutonomousGoalControlLoopSnapshotTextStore",
    "TransactionalAutonomousGoalControlLoopSnapshotTextStore",
    "JsonAutonomousGoalControlLoopSnapshotPersistence",
    "TransactionalJsonAutonomousGoalControlLoopSnapshotPersistence",
    "AutonomousGoalControlLoopPersistenceCoordinator",
    "seal_autonomous_goal_control_loop_snapshot",
    "validate_autonomous_goal_control_loop_snapshot",
]
