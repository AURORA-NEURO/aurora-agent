"""Deterministic multi-goal admission and optimistic claiming.

The goal ledger persists one objective at a time, but a useful autonomous worker also needs to
decide which objective to attempt next. This module combines caller-owned priority and urgency,
deadline pressure, aging fairness, explicit dependencies, retry policy, concurrency, cost, and
per-domain quotas into a canonical schedule digest. A later worker can replay and claim admitted
rows against the SQLite goal ledger without retaining task text or making provider calls.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
import time
from collections.abc import Mapping, Sequence
from typing import Any, Literal

from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .goals import AutonomousGoalError, AutonomousGoalLedger, AutonomousGoalRecord, GoalStatus


GOAL_SCHEDULE_SCHEMA = "bioprism-autonomous-goal-schedule/0.1"
GOAL_CLAIM_SCHEMA = "bioprism-autonomous-goal-claim/0.1"
GOAL_SCHEDULE_RETENTION = "metadata_only_goal_admission;task_text_and_payloads_not_retained"
MAX_GOAL_SCHEDULE_GOALS = 4_096
MAX_GOAL_SCHEDULE_SIGNALS = 4_096
MAX_GOAL_SCHEDULE_DEPENDENCIES = 64
MAX_GOAL_SCHEDULE_SELECTED = 128
MAX_GOAL_SCHEDULE_BYTES = 2_000_000
# ``cross_domain`` is a first-class member of the shared autonomous domain catalogue.  Keep an
# explicit alias here so admission callers can depend on the scheduler's supported set without
# accidentally introducing a second, divergent domain list.
AUTONOMOUS_GOAL_SCHEDULABLE_DOMAINS = AUTONOMOUS_DOMAIN_NAMES

ScheduleDecision = Literal["active", "admit", "defer", "ineligible"]


def _fail(message: str) -> None:
    raise AutonomousGoalError(f"autonomous goal scheduler {message}")


def _identifier(value: Any, *, name: str) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > 256:
        _fail(f"{name} is outside its bounded identifier contract")
    return value.strip()


def _number(value: Any, *, name: str, minimum: float, maximum: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or float(value) < minimum or float(value) > maximum:
        _fail(f"{name} is outside its numeric bounds")
    return float(value)


def _integer(value: Any, *, name: str, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum or value > maximum:
        _fail(f"{name} is outside its integer bounds")
    return value


def _digest(value: Any) -> str:
    try:
        encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise AutonomousGoalError("autonomous goal schedule is not canonical JSON") from error
    return hashlib.sha256(encoded).hexdigest()


def _rounded(value: float) -> float:
    # Four decimal places keeps small aging values in the same JSON number spelling in
    # Python and JavaScript, while remaining more precise than the admission score weights.
    rounded = round(value, 4)
    if rounded == 0:
        return 0
    return int(rounded) if rounded.is_integer() else rounded


def _zero_normalized(value: float) -> float | int:
    return 0 if value == 0 else int(value) if value.is_integer() else value


def _domain(value: Any) -> str:
    if not isinstance(value, str) or value not in AUTONOMOUS_GOAL_SCHEDULABLE_DOMAINS:
        _fail("goal domain is not a supported autonomous scheduling domain")
    return value


@dataclass(frozen=True, slots=True)
class AutonomousGoalSchedulingSignal:
    goal_id: str
    priority: float = 0.5
    urgency: float = 0.0
    deadline_ns: int | None = None
    estimated_cost: int = 1
    dependencies: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        object.__setattr__(self, "goal_id", _identifier(self.goal_id, name="signal.goal_id"))
        object.__setattr__(self, "priority", _number(self.priority, name="signal.priority", minimum=0, maximum=1))
        object.__setattr__(self, "urgency", _number(self.urgency, name="signal.urgency", minimum=0, maximum=1))
        if self.deadline_ns is not None:
            _integer(self.deadline_ns, name="signal.deadline_ns", minimum=0, maximum=2**63 - 1)
        object.__setattr__(self, "estimated_cost", _integer(self.estimated_cost, name="signal.estimated_cost", minimum=1, maximum=1_000_000))
        if not isinstance(self.dependencies, Sequence) or isinstance(self.dependencies, (str, bytes, bytearray)) or len(self.dependencies) > MAX_GOAL_SCHEDULE_DEPENDENCIES:
            _fail("signal.dependencies is outside its bounds")
        object.__setattr__(self, "dependencies", tuple(sorted({_identifier(item, name="signal dependency") for item in self.dependencies})))

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousGoalSchedulingSignal":
        if not isinstance(value, Mapping):
            _fail("signal is malformed")
        return cls(
            goal_id=value.get("goal_id"),
            priority=value.get("priority", 0.5),
            urgency=value.get("urgency", 0.0),
            deadline_ns=value.get("deadline_ns"),
            estimated_cost=value.get("estimated_cost", 1),
            dependencies=tuple(value.get("dependencies", ())),
        )

    def to_dict(self) -> dict[str, Any]:
        return {"goal_id": self.goal_id, "priority": self.priority, "urgency": self.urgency, "deadline_ns": self.deadline_ns, "estimated_cost": self.estimated_cost, "dependencies": list(self.dependencies)}


@dataclass(frozen=True, slots=True)
class AutonomousGoalScheduleRow:
    goal_id: str
    domain: str
    status: GoalStatus
    revision: int
    attempt: int
    max_attempts: int
    priority: float
    urgency: float
    deadline_ns: int | None
    estimated_cost: int
    age_score: float
    deadline_score: float
    retry_pressure: float
    score: float
    efficiency: float
    dependencies: tuple[str, ...]
    unmet_dependencies: tuple[str, ...]
    decision: ScheduleDecision
    reason: str
    expected_revision: int

    def to_dict(self) -> dict[str, Any]:
        return {"goal_id": self.goal_id, "domain": self.domain, "status": self.status, "revision": self.revision, "attempt": self.attempt, "max_attempts": self.max_attempts, "priority": self.priority, "urgency": self.urgency, "deadline_ns": self.deadline_ns, "estimated_cost": self.estimated_cost, "age_score": self.age_score, "deadline_score": self.deadline_score, "retry_pressure": self.retry_pressure, "score": self.score, "efficiency": self.efficiency, "dependencies": list(self.dependencies), "unmet_dependencies": list(self.unmet_dependencies), "decision": self.decision, "reason": self.reason, "expected_revision": self.expected_revision}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousGoalScheduleRow":
        if not isinstance(value, Mapping):
            _fail("schedule row is malformed")
        return cls(goal_id=value.get("goal_id"), domain=value.get("domain"), status=value.get("status"), revision=value.get("revision"), attempt=value.get("attempt"), max_attempts=value.get("max_attempts"), priority=value.get("priority"), urgency=value.get("urgency"), deadline_ns=value.get("deadline_ns"), estimated_cost=value.get("estimated_cost"), age_score=value.get("age_score"), deadline_score=value.get("deadline_score"), retry_pressure=value.get("retry_pressure"), score=value.get("score"), efficiency=value.get("efficiency"), dependencies=tuple(value.get("dependencies", ())), unmet_dependencies=tuple(value.get("unmet_dependencies", ())), decision=value.get("decision"), reason=value.get("reason"), expected_revision=value.get("expected_revision"))


@dataclass(frozen=True, slots=True)
class AutonomousGoalSchedule:
    now_ns: int
    max_selected: int
    max_concurrent: int
    max_cost: int
    active_count: int
    used_cost: int
    selected_goal_ids: tuple[str, ...]
    rows: tuple[AutonomousGoalScheduleRow, ...]
    required_domains: tuple[str, ...]
    selected_domains: tuple[str, ...]
    missing_domains: tuple[str, ...]
    schedule_digest: str

    def to_dict(self) -> dict[str, Any]:
        body = {"schema": GOAL_SCHEDULE_SCHEMA, "now_ns": self.now_ns, "max_selected": self.max_selected, "max_concurrent": self.max_concurrent, "max_cost": self.max_cost, "active_count": self.active_count, "used_cost": self.used_cost, "selected_goal_ids": list(self.selected_goal_ids), "rows": [row.to_dict() for row in self.rows], "coverage": {"required_domains": list(self.required_domains), "selected_domains": list(self.selected_domains), "missing_domains": list(self.missing_domains)}, "retention": GOAL_SCHEDULE_RETENTION, "secret_material": "never_returned"}
        return {**body, "schedule_digest": self.schedule_digest}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousGoalSchedule":
        normalized = validate_goal_schedule(value)
        return cls(now_ns=normalized["now_ns"], max_selected=normalized["max_selected"], max_concurrent=normalized["max_concurrent"], max_cost=normalized["max_cost"], active_count=normalized["active_count"], used_cost=normalized["used_cost"], selected_goal_ids=tuple(normalized["selected_goal_ids"]), rows=tuple(AutonomousGoalScheduleRow.from_mapping(row) for row in normalized["rows"]), required_domains=tuple(normalized["coverage"]["required_domains"]), selected_domains=tuple(normalized["coverage"]["selected_domains"]), missing_domains=tuple(normalized["coverage"]["missing_domains"]), schedule_digest=normalized["schedule_digest"])


@dataclass(frozen=True, slots=True)
class AutonomousGoalClaim:
    goal_id: str
    previous_status: GoalStatus
    previous_revision: int
    running_revision: int
    schedule_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {"goal_id": self.goal_id, "previous_status": self.previous_status, "previous_revision": self.previous_revision, "running_revision": self.running_revision, "schedule_digest": self.schedule_digest}


@dataclass(frozen=True, slots=True)
class AutonomousGoalClaimResult:
    schedule_digest: str
    claims: tuple[AutonomousGoalClaim, ...]
    claim_digest: str

    def to_dict(self) -> dict[str, Any]:
        body = {"schema": GOAL_CLAIM_SCHEMA, "schedule_digest": self.schedule_digest, "claims": [claim.to_dict() for claim in self.claims], "retention": GOAL_SCHEDULE_RETENTION, "secret_material": "never_returned"}
        return {**body, "claim_digest": self.claim_digest}


def _score(goal: AutonomousGoalRecord, signal: AutonomousGoalSchedulingSignal, *, now_ns: int, aging_window_ns: int) -> dict[str, Any]:
    age_score = _rounded(min(1.0, max(0, now_ns - goal.updated_ns) / aging_window_ns))
    if signal.deadline_ns is None:
        deadline_score = 0
    elif signal.deadline_ns <= now_ns:
        deadline_score = 1
    else:
        deadline_score = _rounded(min(1.0, aging_window_ns / (signal.deadline_ns - now_ns + aging_window_ns)))
    retry_pressure = _rounded(min(1.0, goal.attempt / max(1, goal.max_attempts)))
    score = _rounded(max(0.0, min(1.0, 0.45 * signal.priority + 0.25 * signal.urgency + 0.20 * deadline_score + 0.10 * age_score - 0.05 * retry_pressure)))
    return {"priority": _zero_normalized(signal.priority), "urgency": _zero_normalized(signal.urgency), "deadline_ns": signal.deadline_ns, "estimated_cost": signal.estimated_cost, "age_score": age_score, "deadline_score": deadline_score, "retry_pressure": retry_pressure, "score": score, "efficiency": _rounded(score / signal.estimated_cost)}


def _lifecycle(goal: AutonomousGoalRecord, *, allow_failed_retry: bool, include_paused: bool) -> tuple[bool, ScheduleDecision, str]:
    if goal.status == "running":
        return False, "active", "already_running"
    if goal.status == "ready":
        return True, "defer", "eligible"
    if goal.status == "paused":
        return (True, "defer", "eligible") if include_paused else (False, "ineligible", "paused_excluded_by_policy")
    if goal.status == "failed":
        if not allow_failed_retry:
            return False, "ineligible", "failed_retry_requires_explicit_policy"
        if goal.attempt >= goal.max_attempts:
            return False, "ineligible", "retry_budget_exhausted"
        return True, "defer", "eligible_retry"
    if goal.status == "blocked":
        return False, "ineligible", "blocked_requires_explicit_reopen"
    if goal.status == "completed":
        return False, "ineligible", "terminal_completed"
    return False, "ineligible", "terminal_cancelled"


def _validate_options(options: Mapping[str, Any]) -> dict[str, Any]:
    required_domains = tuple(options.get("required_domains", ()))
    if len(required_domains) > len(AUTONOMOUS_GOAL_SCHEDULABLE_DOMAINS) or len(set(required_domains)) != len(required_domains):
        _fail("required_domains is malformed")
    for item in required_domains:
        _domain(item)
    quotas = options.get("domain_quotas", {})
    if not isinstance(quotas, Mapping):
        _fail("domain_quotas must be a mapping")
    normalized_quotas: dict[str, int] = {}
    for key, value in quotas.items():
        normalized_quotas[_domain(key)] = _integer(value, name=f"domain_quotas.{key}", minimum=1, maximum=MAX_GOAL_SCHEDULE_SELECTED)
    allow_failed_retry = options.get("allow_failed_retry", False)
    include_paused = options.get("include_paused", True)
    if not isinstance(allow_failed_retry, bool) or not isinstance(include_paused, bool):
        _fail("retry and pause policies must be boolean")
    return {"now_ns": _integer(options.get("now_ns", time.time_ns()), name="now_ns", minimum=0, maximum=2**63 - 1), "max_selected": _integer(options.get("max_selected", 1), name="max_selected", minimum=1, maximum=MAX_GOAL_SCHEDULE_SELECTED), "max_concurrent": _integer(options.get("max_concurrent", options.get("max_selected", 1)), name="max_concurrent", minimum=1, maximum=MAX_GOAL_SCHEDULE_SELECTED), "max_cost": _integer(options.get("max_cost", 1_000_000), name="max_cost", minimum=1, maximum=1_000_000_000), "aging_window_ns": _integer(options.get("aging_window_ns", 86_400_000), name="aging_window_ns", minimum=1, maximum=2**63 - 1), "allow_failed_retry": allow_failed_retry, "include_paused": include_paused, "required_domains": tuple(sorted(required_domains, key=AUTONOMOUS_GOAL_SCHEDULABLE_DOMAINS.index)), "domain_quotas": normalized_quotas}


def _signal_map(goals: Mapping[str, AutonomousGoalRecord], signals: Sequence[AutonomousGoalSchedulingSignal | Mapping[str, Any]]) -> dict[str, AutonomousGoalSchedulingSignal]:
    if len(signals) > MAX_GOAL_SCHEDULE_SIGNALS:
        _fail("signals are outside their bounds")
    result: dict[str, AutonomousGoalSchedulingSignal] = {}
    for raw in signals:
        signal = raw if isinstance(raw, AutonomousGoalSchedulingSignal) else AutonomousGoalSchedulingSignal.from_mapping(raw)
        if signal.goal_id not in goals:
            _fail(f"signal references unknown goal {signal.goal_id}")
        if signal.goal_id in result:
            _fail(f"duplicate signal for goal {signal.goal_id}")
        result[signal.goal_id] = signal
    return result


def validate_goal_schedule(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping) or value.get("schema") != GOAL_SCHEDULE_SCHEMA:
        _fail("schedule schema is invalid")
    allowed = {"schema", "now_ns", "max_selected", "max_concurrent", "max_cost", "active_count", "used_cost", "selected_goal_ids", "rows", "coverage", "schedule_digest", "retention", "secret_material"}
    if set(value).difference(allowed):
        _fail("schedule contains unsupported fields")
    if value.get("retention") != GOAL_SCHEDULE_RETENTION or value.get("secret_material") != "never_returned":
        _fail("schedule retention posture is invalid")
    _integer(value.get("now_ns"), name="schedule.now_ns", minimum=0, maximum=2**63 - 1)
    _integer(value.get("max_selected"), name="schedule.max_selected", minimum=1, maximum=MAX_GOAL_SCHEDULE_SELECTED)
    _integer(value.get("max_concurrent"), name="schedule.max_concurrent", minimum=1, maximum=MAX_GOAL_SCHEDULE_SELECTED)
    _integer(value.get("max_cost"), name="schedule.max_cost", minimum=1, maximum=1_000_000_000)
    _integer(value.get("active_count"), name="schedule.active_count", minimum=0, maximum=MAX_GOAL_SCHEDULE_GOALS)
    _integer(value.get("used_cost"), name="schedule.used_cost", minimum=0, maximum=1_000_000_000)
    raw_rows = value.get("rows")
    selected = value.get("selected_goal_ids")
    if not isinstance(raw_rows, Sequence) or isinstance(raw_rows, (str, bytes, bytearray)) or len(raw_rows) > MAX_GOAL_SCHEDULE_GOALS:
        _fail("schedule rows are outside their bounds")
    if not isinstance(selected, Sequence) or isinstance(selected, (str, bytes, bytearray)) or len(selected) > MAX_GOAL_SCHEDULE_SELECTED:
        _fail("schedule selected_goal_ids are outside their bounds")
    rows: list[dict[str, Any]] = []
    row_ids: set[str] = set()
    for raw in raw_rows:
        row = AutonomousGoalScheduleRow.from_mapping(raw)
        if row.goal_id in row_ids:
            _fail("schedule contains duplicate goal rows")
        row_ids.add(row.goal_id)
        _domain(row.domain)
        if row.decision not in {"active", "admit", "defer", "ineligible"}:
            _fail(f"schedule row {row.goal_id} decision is invalid")
        _integer(row.revision, name=f"schedule row {row.goal_id}.revision", minimum=0, maximum=2**63 - 1)
        _integer(row.expected_revision, name=f"schedule row {row.goal_id}.expected_revision", minimum=0, maximum=2**63 - 1)
        rows.append(row.to_dict())
    selected_ids = [_identifier(item, name="schedule selected_goal_id") for item in selected]
    if len(set(selected_ids)) != len(selected_ids) or any(item not in row_ids for item in selected_ids):
        _fail("schedule selected_goal_ids do not match rows")
    by_id = {row["goal_id"]: row for row in rows}
    if any(by_id[item]["decision"] != "admit" for item in selected_ids):
        _fail("schedule selected_goal_ids include a non-admitted row")
    coverage = value.get("coverage")
    if not isinstance(coverage, Mapping):
        _fail("schedule coverage is malformed")
    for key in ("required_domains", "selected_domains", "missing_domains"):
        values = coverage.get(key)
        if not isinstance(values, Sequence) or isinstance(values, (str, bytes, bytearray)):
            _fail("schedule coverage is malformed")
        for item in values:
            _domain(item)
    schedule_digest = value.get("schedule_digest")
    if not isinstance(schedule_digest, str) or len(schedule_digest) != 64 or any(char not in "0123456789abcdef" for char in schedule_digest):
        _fail("schedule_digest is malformed")
    normalized = {"schema": GOAL_SCHEDULE_SCHEMA, "now_ns": value["now_ns"], "max_selected": value["max_selected"], "max_concurrent": value["max_concurrent"], "max_cost": value["max_cost"], "active_count": value["active_count"], "used_cost": value["used_cost"], "selected_goal_ids": selected_ids, "rows": sorted(rows, key=lambda row: row["goal_id"]), "coverage": {"required_domains": list(coverage["required_domains"]), "selected_domains": list(coverage["selected_domains"]), "missing_domains": list(coverage["missing_domains"])}, "retention": GOAL_SCHEDULE_RETENTION, "secret_material": "never_returned"}
    if _digest(normalized) != schedule_digest:
        _fail("schedule_digest does not match schedule content")
    if len(json.dumps({**normalized, "schedule_digest": schedule_digest}, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")) > MAX_GOAL_SCHEDULE_BYTES:
        _fail("schedule exceeds its byte bound")
    normalized["schedule_digest"] = schedule_digest
    return normalized


def schedule_autonomous_goals(goals: Sequence[AutonomousGoalRecord | Mapping[str, Any]], options: Mapping[str, Any] | None = None) -> AutonomousGoalSchedule:
    if not isinstance(goals, Sequence) or isinstance(goals, (str, bytes, bytearray)) or len(goals) > MAX_GOAL_SCHEDULE_GOALS:
        _fail("goals are outside their bounds")
    raw_options = {} if options is None else dict(options)
    limits = _validate_options(raw_options)
    goal_map: dict[str, AutonomousGoalRecord] = {}
    for index, raw in enumerate(goals):
        goal = raw if isinstance(raw, AutonomousGoalRecord) else AutonomousGoalRecord.from_mapping(raw)
        _identifier(goal.goal_id, name=f"goal {index}.goal_id")
        _domain(goal.domain)
        _integer(goal.revision, name=f"goal {index}.revision", minimum=0, maximum=2**63 - 1)
        _integer(goal.attempt, name=f"goal {index}.attempt", minimum=0, maximum=128)
        _integer(goal.max_attempts, name=f"goal {index}.max_attempts", minimum=1, maximum=128)
        _integer(goal.updated_ns, name=f"goal {index}.updated_ns", minimum=0, maximum=2**63 - 1)
        if goal.goal_id in goal_map:
            _fail(f"duplicate goal_id {goal.goal_id}")
        goal_map[goal.goal_id] = goal
    signals = _signal_map(goal_map, tuple(raw_options.get("signals", ())))
    active_count = sum(1 for goal in goal_map.values() if goal.status == "running")
    rows: dict[str, dict[str, Any]] = {}
    eligible: set[str] = set()
    dependencies: dict[str, tuple[str, ...]] = {}
    for goal in goal_map.values():
        signal = signals.get(goal.goal_id, AutonomousGoalSchedulingSignal(goal_id=goal.goal_id))
        dependencies[goal.goal_id] = signal.dependencies
        is_eligible, decision, reason = _lifecycle(goal, allow_failed_retry=limits["allow_failed_retry"], include_paused=limits["include_paused"])
        if is_eligible:
            eligible.add(goal.goal_id)
        rows[goal.goal_id] = {"goal_id": goal.goal_id, "domain": _domain(goal.domain), "status": goal.status, "revision": goal.revision, "attempt": goal.attempt, "max_attempts": goal.max_attempts, **_score(goal, signal, now_ns=limits["now_ns"], aging_window_ns=limits["aging_window_ns"]), "dependencies": list(signal.dependencies), "unmet_dependencies": [], "decision": decision, "reason": reason, "expected_revision": goal.revision}
    cycle_nodes: set[str] = set()
    visiting: list[str] = []
    visited: set[str] = set()

    def visit_cycle(goal_id: str) -> None:
        if goal_id in visiting:
            cycle_nodes.update(visiting[visiting.index(goal_id) :])
            return
        if goal_id in visited:
            return
        visited.add(goal_id)
        visiting.append(goal_id)
        for dependency in dependencies.get(goal_id, ()):
            if dependency in rows:
                visit_cycle(dependency)
        visiting.pop()

    for goal_id in rows:
        visit_cycle(goal_id)
    for goal_id in cycle_nodes:
        eligible.discard(goal_id)
        rows[goal_id]["decision"] = "ineligible"
        rows[goal_id]["reason"] = "dependency_cycle"
        rows[goal_id]["unmet_dependencies"] = list(dependencies[goal_id])
    ordered_candidates = sorted((rows[goal_id] for goal_id in eligible), key=lambda row: (-row["efficiency"], -row["score"], row["goal_id"]))
    ordered: list[str] = []
    ordered_set: set[str] = set()

    def visit_order(goal_id: str) -> None:
        if goal_id in ordered_set or goal_id not in eligible:
            return
        for dependency in dependencies.get(goal_id, ()):
            if dependency in eligible:
                visit_order(dependency)
        ordered_set.add(goal_id)
        ordered.append(goal_id)

    for row in ordered_candidates:
        visit_order(row["goal_id"])
    selected: set[str] = set()
    selected_goal_ids: list[str] = []
    selected_domain_counts: dict[str, int] = {}
    used_cost = 0
    quotas: Mapping[str, int] = limits["domain_quotas"]
    for goal_id in ordered:
        row = rows[goal_id]
        unmet = [dependency for dependency in dependencies[goal_id] if dependency not in goal_map or (goal_map[dependency].status != "completed" and dependency not in selected)]
        row["unmet_dependencies"] = unmet
        if unmet:
            row["decision"], row["reason"] = "defer", "dependency_not_ready"
            continue
        if active_count + len(selected_goal_ids) >= limits["max_concurrent"]:
            row["decision"], row["reason"] = "defer", "concurrency_budget_exhausted"
            continue
        if len(selected_goal_ids) >= limits["max_selected"]:
            row["decision"], row["reason"] = "defer", "selection_budget_exhausted"
            continue
        quota = quotas.get(row["domain"])
        if quota is not None and selected_domain_counts.get(row["domain"], 0) >= quota:
            row["decision"], row["reason"] = "defer", "domain_quota_exhausted"
            continue
        if used_cost + row["estimated_cost"] > limits["max_cost"]:
            row["decision"], row["reason"] = "defer", "cost_budget_exhausted"
            continue
        selected.add(goal_id)
        selected_goal_ids.append(goal_id)
        selected_domain_counts[row["domain"]] = selected_domain_counts.get(row["domain"], 0) + 1
        used_cost += row["estimated_cost"]
        row["decision"], row["reason"] = "admit", "admitted_dependency_closed_candidate"
    required_domains = limits["required_domains"]
    selected_domains = tuple(domain for domain in AUTONOMOUS_GOAL_SCHEDULABLE_DOMAINS if domain in selected_domain_counts)
    missing_domains = tuple(domain for domain in required_domains if domain not in selected_domain_counts)
    body = {"schema": GOAL_SCHEDULE_SCHEMA, "now_ns": limits["now_ns"], "max_selected": limits["max_selected"], "max_concurrent": limits["max_concurrent"], "max_cost": limits["max_cost"], "active_count": active_count, "used_cost": used_cost, "selected_goal_ids": selected_goal_ids, "rows": sorted(rows.values(), key=lambda row: row["goal_id"]), "coverage": {"required_domains": list(required_domains), "selected_domains": list(selected_domains), "missing_domains": list(missing_domains)}, "retention": GOAL_SCHEDULE_RETENTION, "secret_material": "never_returned"}
    return AutonomousGoalSchedule(now_ns=body["now_ns"], max_selected=body["max_selected"], max_concurrent=body["max_concurrent"], max_cost=body["max_cost"], active_count=body["active_count"], used_cost=body["used_cost"], selected_goal_ids=tuple(selected_goal_ids), rows=tuple(AutonomousGoalScheduleRow.from_mapping(row) for row in body["rows"]), required_domains=required_domains, selected_domains=selected_domains, missing_domains=missing_domains, schedule_digest=_digest(body))


def claim_autonomous_goals(ledger: AutonomousGoalLedger, schedule: AutonomousGoalSchedule | Mapping[str, Any], *, now_ns: int | None = None) -> AutonomousGoalClaimResult:
    if not isinstance(ledger, AutonomousGoalLedger):
        _fail("claim requires an AutonomousGoalLedger")
    normalized = validate_goal_schedule(schedule.to_dict() if isinstance(schedule, AutonomousGoalSchedule) else schedule)
    rows = {row["goal_id"]: row for row in normalized["rows"]}
    admitted = [rows[goal_id] for goal_id in normalized["selected_goal_ids"] if rows[goal_id]["decision"] == "admit"]
    for row in admitted:
        current = ledger.get(row["goal_id"])
        if current is None or current.revision != row["expected_revision"] or current.status != row["status"] or current.status not in {"ready", "paused", "failed"}:
            _fail(f"schedule is stale for goal {row['goal_id']}")
    claims: list[AutonomousGoalClaim] = []
    for row in admitted:
        current = ledger.get(row["goal_id"])
        assert current is not None
        previous_status = current.status
        previous_revision = current.revision
        if current.status == "failed":
            current = ledger.transition(current.goal_id, "ready", expected_revision=current.revision, now_ns=now_ns)
        running = ledger.transition(current.goal_id, "running", expected_revision=current.revision, now_ns=now_ns)
        claims.append(AutonomousGoalClaim(row["goal_id"], previous_status, previous_revision, running.revision, normalized["schedule_digest"]))
    body = {"schema": GOAL_CLAIM_SCHEMA, "schedule_digest": normalized["schedule_digest"], "claims": [claim.to_dict() for claim in claims], "retention": GOAL_SCHEDULE_RETENTION, "secret_material": "never_returned"}
    return AutonomousGoalClaimResult(normalized["schedule_digest"], tuple(claims), _digest(body))


class AutonomousGoalScheduler:
    """Reusable planner/claimer for workers servicing any built-in autonomous domain."""

    def plan(self, goals: Sequence[AutonomousGoalRecord | Mapping[str, Any]], options: Mapping[str, Any] | None = None) -> AutonomousGoalSchedule:
        return schedule_autonomous_goals(goals, options)

    def claim(self, ledger: AutonomousGoalLedger, schedule: AutonomousGoalSchedule | Mapping[str, Any], *, now_ns: int | None = None) -> AutonomousGoalClaimResult:
        return claim_autonomous_goals(ledger, schedule, now_ns=now_ns)
