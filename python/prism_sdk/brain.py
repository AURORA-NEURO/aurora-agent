"""High-level autonomous decision loop over the Rust brain and :mod:`llm_runtime`.

This facade is intentionally bounded but real: it selects a model, assembles a bounded prompt,
validates a plan, requires explicit approval for the provider effect, and invokes the model with a
caller-owned credential handle. A structured model decision can then be proposed to the existing
mission executor for server-side preflight and a separate caller approval; the model never grants
itself tools, side effects, or credentials.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import hashlib
import json
import os
from pathlib import Path
import re
import threading
import uuid
from typing import Any, Callable, Mapping, Protocol, Sequence

from .llm_runtime import (
    CredentialError,
    CredentialHandle,
    LLMRuntime,
    ProviderRequest,
    ProviderResponse,
    ProviderError,
    ProviderTool,
    ProviderToolCall,
    ProviderToolLoopResult,
    ProviderToolResult,
)
from .errors import ArgumentError
from .mission import MissionPolicy, MissionRequest
from .memory import BrainEpisodicMemory, BrainMemoryError, MemoryQuery
from .tooling import ToolCatalogue, ToolSchemaError


class BrainRunError(RuntimeError):
    """The bounded autonomous loop could not reach a provider invocation."""


DEFAULT_MISSION_RESPONSE_SCHEMA: dict[str, Any] = {
    "type": "object",
    "required": ["mission"],
    "properties": {
        "mission": {
            "type": "object",
            "required": ["steps"],
            "properties": {
                "steps": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 128,
                    "items": {
                        "type": "object",
                        "required": [
                            "id",
                            "domain",
                            "capability",
                            "objective",
                            "tool",
                            "arguments",
                        ],
                        "properties": {
                            "id": {"type": "string", "minLength": 1, "maxLength": 256},
                            "domain": {"type": "string", "minLength": 1, "maxLength": 256},
                            "capability": {"type": "string", "minLength": 1, "maxLength": 256},
                            "objective": {"type": "string", "minLength": 1, "maxLength": 4096},
                            "tool": {"type": "string", "minLength": 1, "maxLength": 256},
                            "arguments": {"type": "object"},
                            "depends_on": {"type": "array", "items": {"type": "string"}, "maxItems": 128},
                            "required": {"type": "boolean"},
                            "bindings": {"type": "array", "maxItems": 128},
                        },
                    },
                }
            },
        }
    },
    "additionalProperties": False,
}

MAX_ROUTE_REQUEST_BYTES = 2_000_000
MAX_ROUTE_PROMPT_BYTES = 750_000
MAX_ROUTE_PROMPT_SCHEMAS = 128
MAX_MISSION_AUTHORIZATION_CALLS = 128
MAX_MISSION_AUTHORIZATION_RESULT_BYTES = 750_000
MAX_MISSION_AUTHORIZATION_STEP_OUTPUT_BYTES = 350_000
MAX_ADAPTIVE_ROUTE_LABEL_BYTES = 256
MAX_BRAIN_EVALUATOR_ID_BYTES = 128
MAX_BRAIN_EVALUATOR_EVIDENCE_BYTES = 350_000
MAX_BRAIN_EVALUATOR_INPUT_BYTES = 500_000
MAX_BRAIN_REPLAY_BYTES = 16_000
MAX_BRAIN_REPLAN_INSTRUCTION_BYTES = 4_096
BRAIN_EVALUATOR_REPLAY_SCHEMA = "bioprism-brain-evaluator-replay/0.1"
_REPLAN_SECRET_PATTERNS = (
    re.compile(
        r"(?i)\b(?:api[_ -]?key|access[_ -]?token|refresh[_ -]?token|password|authorization|secret)\b\s*[:=]\s*\S+"
    ),
    re.compile(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{16,}"),
    re.compile(r"\b(?:sk|rk|pk)-[A-Za-z0-9_-]{16,}\b"),
)


def _bounded_route_prompt_context(route: Mapping[str, Any]) -> dict[str, Any]:
    """Project route evidence into a bounded, model-readable context packet.

    The capability route is authoritative evidence about the live catalogue, but it can contain
    many schemas. The model needs the candidate contract, not an unbounded registry dump. Schemas
    are admitted in deterministic order until the packet bound is reached; omitted schemas remain
    explicit so a model cannot mistake a truncated route for a complete catalogue.
    """

    recommended = route.get("recommended_tools", [])
    if not isinstance(recommended, list) or any(not isinstance(tool, str) for tool in recommended):
        raise BrainRunError("capability route returned malformed recommended_tools")
    needs = route.get("needs", [])
    if not isinstance(needs, list) or any(not isinstance(need, Mapping) for need in needs):
        raise BrainRunError("capability route returned malformed needs")
    raw_schemas = route.get("tool_schemas", [])
    if not isinstance(raw_schemas, list) or any(not isinstance(schema, Mapping) for schema in raw_schemas):
        raise BrainRunError("capability route returned malformed tool_schemas")

    compact_needs: list[dict[str, Any]] = []
    for need in needs:
        compact_needs.append(
            {
                "id": need.get("id"),
                "resolution": need.get("resolution"),
                "candidate_groups": need.get("candidate_groups", []),
                "candidate_domains": need.get("candidate_domains", []),
                "candidate_tools": need.get("candidate_tools", []),
            }
        )
    packet: dict[str, Any] = {
        "workflow": "capability_route_context",
        "route_id": route.get("route_id"),
        "catalog_digest": route.get("catalog_digest"),
        "goal": route.get("goal"),
        "needs": compact_needs,
        "recommended_tools": recommended,
        "schema_attachment": route.get("schema_attachment", {}),
        "tool_schemas": [],
        "tool_schemas_omitted": 0,
        "does_not_authorize": [
            "candidate ranking is routing evidence, not permission",
            "the caller mission policy remains the only tool allow-list",
            "tool schemas describe inputs but do not establish domain validity or readiness",
        ],
    }
    for schema in raw_schemas[:MAX_ROUTE_PROMPT_SCHEMAS]:
        candidate = dict(packet)
        candidate["tool_schemas"] = [*packet["tool_schemas"], dict(schema)]
        encoded = json.dumps(candidate, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        if len(encoded.encode("utf-8")) > MAX_ROUTE_PROMPT_BYTES:
            break
        packet["tool_schemas"] = candidate["tool_schemas"]
    packet["tool_schemas_omitted"] = len(raw_schemas) - len(packet["tool_schemas"])
    return packet


def _adaptive_route_context(
    route: Mapping[str, Any],
    *,
    task: str,
    route_request: Mapping[str, Any],
) -> dict[str, Any]:
    """Derive bounded contextual-selection labels from one authoritative live route."""

    if route.get("workflow") != "capability_route":
        raise BrainRunError("adaptive route must be a capability_route report")
    if route.get("goal") != task:
        raise BrainRunError("adaptive route goal must match the task")
    unresolved = route.get("unresolved_needs", [])
    if not isinstance(unresolved, list) or any(not isinstance(item, str) for item in unresolved):
        raise BrainRunError("adaptive route returned malformed unresolved_needs")
    if unresolved:
        raise BrainRunError("adaptive route contains unresolved needs: " + ", ".join(unresolved))
    needs = route.get("needs", [])
    if not isinstance(needs, list) or any(not isinstance(need, Mapping) for need in needs):
        raise BrainRunError("adaptive route returned malformed needs")
    domains: set[str] = set()
    capabilities: set[str] = set()
    for need in needs:
        for key, target in (("candidate_domains", domains), ("candidate_groups", capabilities)):
            values = need.get(key, [])
            if not isinstance(values, list) or any(not isinstance(value, str) for value in values):
                raise BrainRunError(f"adaptive route need {key} must be a string list")
            target.update(value for value in values if value.strip())
    coverage = route.get("route_coverage")
    if isinstance(coverage, Mapping):
        for value in coverage.get("candidate_domains", []):
            if isinstance(value, str) and value.strip():
                domains.add(value)
        for value in coverage.get("candidate_groups", []):
            if isinstance(value, str) and value.strip():
                capabilities.add(value)
    if not domains:
        domains.add("cross_domain")
    if not capabilities:
        capabilities.add("cross_domain")
    risk_class = route_request.get("risk_class", "routed_standard")
    task_family = route_request.get("task_family", "routed_task")
    if not isinstance(risk_class, str) or not risk_class.strip():
        raise BrainRunError("route_request.risk_class must be a non-empty string")
    if not isinstance(task_family, str) or not task_family.strip():
        raise BrainRunError("route_request.task_family must be a non-empty string")
    context = {
        "domain": "cross_domain:" + ",".join(sorted(domains)),
        "capability": "route:" + ",".join(sorted(capabilities)),
        "risk_class": risk_class,
        "task_family": task_family,
    }
    for name, value in context.items():
        if len(value.encode("utf-8")) > MAX_ADAPTIVE_ROUTE_LABEL_BYTES:
            raise BrainRunError(f"adaptive route context {name} exceeds the bounded label size")
    BrainLearningLedger._assert_safe(context)
    return context


class BrainLearningLedger:
    """Append-only, value-only persistence for evaluator judgments and bandit state.

    The ledger is deliberately separate from :class:`CredentialStore`: it accepts only the Rust
    learning report and the returned next state, rejects secret-shaped field names, bounds both
    record count and file size, and fsyncs each append. It never stores provider response text.
    """

    _SCHEMA = "bioprism-brain-learning-ledger/0.1"
    _FORBIDDEN_FIELDS = {
        "api_key",
        "apikey",
        "authorization",
        "credential",
        "password",
        "secret",
        "access_token",
        "refresh_token",
    }
    _FORBIDDEN_NORMALIZED_FIELDS = {
        "".join(character for character in field if character.isalnum())
        for field in _FORBIDDEN_FIELDS
    }

    def __init__(
        self,
        path: str | os.PathLike[str],
        *,
        max_records: int = 4096,
        max_bytes: int = 32_000_000,
    ) -> None:
        if max_records <= 0 or max_bytes <= 0:
            raise BrainRunError("learning ledger bounds must be positive")
        self.path = Path(path)
        self.max_records = max_records
        self.max_bytes = max_bytes
        self._lock = threading.RLock()

    def append(
        self,
        report: Mapping[str, Any],
        *,
        context_digest: str | None = None,
        replay: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        if not isinstance(report, Mapping):
            raise BrainRunError("learning ledger report must be an object")
        evidence = report.get("learning_evidence")
        next_state = report.get("next_state")
        if not isinstance(evidence, Mapping) or not isinstance(next_state, Mapping):
            raise BrainRunError("learning ledger report must contain evidence and next_state")
        if context_digest is not None and not _valid_digest(context_digest):
            raise BrainRunError("context_digest must be a lowercase SHA-256 digest")
        self._assert_safe(report)
        if replay is not None:
            if not isinstance(replay, Mapping):
                raise BrainRunError("learning ledger replay must be an object")
            self._assert_safe(replay)
            try:
                encoded_replay = json.dumps(
                    dict(replay),
                    ensure_ascii=False,
                    sort_keys=True,
                    separators=(",", ":"),
                    allow_nan=False,
                ).encode("utf-8")
            except (TypeError, ValueError) as error:
                raise BrainRunError("learning ledger replay must be JSON-safe") from error
            if len(encoded_replay) > MAX_BRAIN_REPLAY_BYTES:
                raise BrainRunError("learning ledger replay exceeds the bounded size")
        try:
            encoded_report = json.dumps(
                {"learning_evidence": evidence, "next_state": next_state},
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("learning ledger report must be JSON-safe") from error
        if len(encoded_report) > self.max_bytes:
            raise BrainRunError("learning ledger record exceeds max_bytes")
        record: dict[str, Any] = {
            "learning_evidence": evidence,
            "next_state": next_state,
        }
        if context_digest is not None:
            record["context_digest"] = context_digest
        if replay is not None:
            record["replay"] = dict(replay)
        line = json.dumps(
            {"schema": self._SCHEMA, "record": record},
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8") + b"\n"
        with self._lock:
            existing_size = self.path.stat().st_size if self.path.exists() else 0
            if existing_size + len(line) > self.max_bytes:
                raise BrainRunError("learning ledger capacity is exhausted")
            existing_records = self._read_records_locked()
            if len(existing_records) >= self.max_records:
                raise BrainRunError("learning ledger record capacity is exhausted")
            self.path.parent.mkdir(parents=True, exist_ok=True)
            with self.path.open("ab") as handle:
                handle.write(line)
                handle.flush()
                os.fsync(handle.fileno())
            record_digest = hashlib.sha256(line.rstrip(b"\n")).hexdigest()
            return {
                "schema": self._SCHEMA,
                "record_index": len(existing_records),
                "record_digest": record_digest,
                "evidence_digest": evidence.get("evidence_digest"),
                "replay_digest": None if replay is None else _json_digest(dict(replay)),
            }

    def records(self) -> list[dict[str, Any]]:
        with self._lock:
            return self._read_records_locked()

    def latest_state(self, context_digest: str | None = None) -> dict[str, Any] | None:
        if context_digest is not None and not _valid_digest(context_digest):
            raise BrainRunError("context_digest must be a lowercase SHA-256 digest")
        for row in reversed(self.records()):
            record = row.get("record")
            if not isinstance(record, Mapping):
                continue
            if context_digest is not None and record.get("context_digest") != context_digest:
                continue
            state = record.get("next_state")
            if isinstance(state, Mapping):
                return dict(state)
        return None

    def replays(
        self,
        *,
        run_id: str | None = None,
        evaluator_id: str | None = None,
        limit: int = 128,
    ) -> list[dict[str, Any]]:
        """Return bounded evaluator replay metadata without loading provider/evidence content."""

        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= self.max_records:
            raise BrainRunError("replay limit must be within the ledger record bound")
        for name, value in (("run_id", run_id), ("evaluator_id", evaluator_id)):
            if value is not None and (not isinstance(value, str) or not value.strip()):
                raise BrainRunError(f"{name} must be a non-empty string when supplied")
        matches: list[dict[str, Any]] = []
        for row in reversed(self.records()):
            record = row.get("record")
            replay = record.get("replay") if isinstance(record, Mapping) else None
            if not isinstance(replay, Mapping):
                continue
            if run_id is not None and replay.get("run_id") != run_id:
                continue
            if evaluator_id is not None and replay.get("evaluator_id") != evaluator_id:
                continue
            matches.append(dict(replay))
            if len(matches) >= limit:
                break
        matches.reverse()
        return matches

    def _read_records_locked(self) -> list[dict[str, Any]]:
        if not self.path.exists():
            return []
        if self.path.stat().st_size > self.max_bytes:
            raise BrainRunError("learning ledger exceeds max_bytes")
        rows: list[dict[str, Any]] = []
        with self.path.open("rb") as handle:
            for raw_line in handle:
                if len(rows) >= self.max_records:
                    raise BrainRunError("learning ledger exceeds max_records")
                try:
                    row = json.loads(raw_line.decode("utf-8"))
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    raise BrainRunError("learning ledger contains invalid JSON") from error
                if not isinstance(row, Mapping) or row.get("schema") != self._SCHEMA:
                    raise BrainRunError("learning ledger contains an invalid schema")
                rows.append(dict(row))
        return rows

    @classmethod
    def _assert_safe(cls, value: Any) -> None:
        if isinstance(value, Mapping):
            for key, child in value.items():
                normalized_key = (
                    "".join(character for character in key.lower() if character.isalnum())
                    if isinstance(key, str)
                    else ""
                )
                if normalized_key in cls._FORBIDDEN_NORMALIZED_FIELDS:
                    raise BrainRunError("learning evidence contains a forbidden secret field")
                cls._assert_safe(child)
        elif isinstance(value, (list, tuple)):
            for child in value:
                cls._assert_safe(child)


class BrainWorkspace(Protocol):
    def tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]: ...


def _valid_digest(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(
        character in "0123456789abcdef" for character in value
    )


def _context_identity_digest(context: Mapping[str, Any]) -> str:
    """Match the Rust contextual-selection digest without retaining arbitrary task text."""

    required = ("domain", "capability", "risk_class")
    for name in required:
        value = context.get(name)
        if not isinstance(value, str) or not value.strip():
            raise BrainRunError(f"context.{name} must be a non-empty string")
    task_family = context.get("task_family")
    if task_family is not None and (not isinstance(task_family, str) or not task_family.strip()):
        raise BrainRunError("context.task_family must be a non-empty string when supplied")
    normalized = {
        "domain": context["domain"],
        "capability": context["capability"],
        "risk_class": context["risk_class"],
        "task_family": task_family,
    }
    encoded = json.dumps(
        normalized,
        ensure_ascii=False,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _bandit_observations(state: Mapping[str, Any] | None) -> list[dict[str, Any]]:
    """Project caller-persisted bandit state into the model selector's observation contract."""

    if state is None:
        return []
    if not isinstance(state, Mapping):
        raise BrainRunError("bandit state must be a mapping")
    BrainLearningLedger._assert_safe(state)
    arms = state.get("arms", [])
    if not isinstance(arms, list):
        raise BrainRunError("bandit state arms must be a list")
    observations: list[dict[str, Any]] = []
    for arm in arms:
        if not isinstance(arm, Mapping):
            raise BrainRunError("bandit state arms must contain mappings")
        arm_id = arm.get("arm_id")
        pulls = arm.get("pulls", 0)
        reward_sum = arm.get("reward_sum", 0.0)
        failures = arm.get("failures", 0)
        disabled = arm.get("disabled", False)
        if (
            not isinstance(arm_id, str)
            or not arm_id.strip()
            or not isinstance(pulls, int)
            or isinstance(pulls, bool)
            or pulls < 0
            or not isinstance(reward_sum, (int, float))
            or isinstance(reward_sum, bool)
            or not isinstance(failures, int)
            or isinstance(failures, bool)
            or failures < 0
            or not isinstance(disabled, bool)
        ):
            raise BrainRunError("bandit state contains malformed arm statistics")
        observation = {
            "arm_id": arm_id,
            "pulls": pulls,
            "reward_sum": reward_sum,
            "failures": failures,
            "disabled": disabled,
        }
        try:
            json.dumps(observation, ensure_ascii=False, allow_nan=False)
        except (TypeError, ValueError) as error:
            raise BrainRunError("bandit state contains non-finite arm statistics") from error
        observations.append(observation)
    return observations


@dataclass(frozen=True, slots=True)
class BrainRunResult:
    run_id: str
    status: str
    selection: Mapping[str, Any]
    prompt: Mapping[str, Any]
    plan: Mapping[str, Any]
    response: ProviderResponse | None
    outcome_digest: str
    provider_failover: Mapping[str, Any] | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "run_id": self.run_id,
            "status": self.status,
            "selection": dict(self.selection),
            "prompt": dict(self.prompt),
            "plan": dict(self.plan),
            "response": None if self.response is None else self.response.to_dict(),
            "outcome_digest": self.outcome_digest,
            "provider_failover": None if self.provider_failover is None else dict(self.provider_failover),
            "credential_posture": "handle_only_not_serialized",
            "execution": "provider_call_only",
            "tool_execution": "not_started",
        }


@dataclass(frozen=True, slots=True)
class BrainToolLoopResult:
    """Brain-level envelope for a provider continuation loop.

    The first decision is still planned and approved through the brain kernel. Subsequent native
    tool turns are represented by the runtime's bounded loop; caller code remains the sole effect
    authority through its authorization callback.
    """

    brain_run: BrainRunResult
    status: str
    provider_loop: ProviderToolLoopResult | None
    route: Mapping[str, Any] | None = None
    authorization_receipts: tuple[Mapping[str, Any], ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status,
            "brain_run": self.brain_run.to_dict(),
            "provider_loop": None if self.provider_loop is None else self.provider_loop.to_dict(),
            "route": None if self.route is None else dict(self.route),
            "authorization_receipts": [dict(receipt) for receipt in self.authorization_receipts],
            "authorization": {
                "provider_call": "caller_approved_brain_plan",
                "tool_execution": "caller_callback_only",
            },
        }


@dataclass(frozen=True, slots=True)
class BrainMissionResult:
    """The outcome of proposing and optionally executing one model-authored mission.

    ``preflight`` is always the non-executing server response. ``execution`` is present only
    after the caller explicitly authorizes mission dispatch. The normalized mission carries the
    caller's policy, not a policy selected by the model.
    """

    brain_run: BrainRunResult
    status: str
    mission: Mapping[str, Any] | None
    preflight: Mapping[str, Any] | None
    execution: Mapping[str, Any] | None
    route: Mapping[str, Any] | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status,
            "brain_run": self.brain_run.to_dict(),
            "mission": None if self.mission is None else dict(self.mission),
            "preflight": None if self.preflight is None else dict(self.preflight),
            "execution": None if self.execution is None else dict(self.execution),
            "route": None if self.route is None else dict(self.route),
            "authorization": {
                "provider_call": "recorded_in_brain_run",
                "mission_dispatch": "caller_approved_only",
            },
            "tool_execution": "bounded_agent_mission_executor",
        }


@dataclass(frozen=True, slots=True)
class BrainLearningCycleResult:
    """A bounded mission/evaluation/memory/replan cycle.

    Each attempt is evaluated independently and contributes a separate append-only memory
    episode.  Replanning is proposal-only after a failed attempt unless the caller explicitly
    supplied a mission option that dispatches it; the cycle refuses to replay after a dispatched
    mission because a transport failure is not proof that an external effect did not happen.
    """

    status: str
    final_result: BrainMissionResult
    attempts: tuple[BrainMissionResult, ...]
    evaluations: tuple[Mapping[str, Any], ...]
    memory_receipts: tuple[Mapping[str, Any], ...]
    recalled_memory: tuple[Mapping[str, Any], ...]
    replan_count: int

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "bioprism-brain-learning-cycle/0.1",
            "status": self.status,
            "final_result": self.final_result.to_dict(),
            "attempts": [attempt.to_dict() for attempt in self.attempts],
            "evaluations": [dict(evaluation) for evaluation in self.evaluations],
            "memory_receipts": [dict(receipt) for receipt in self.memory_receipts],
            "recalled_memory": [dict(episode) for episode in self.recalled_memory],
            "replan_count": self.replan_count,
            "authorization": {
                "memory": "value_only_hash_chained",
                "mission_dispatch": "caller_approved_only",
            },
        }


@dataclass(frozen=True, slots=True)
class BrainJobRunResult:
    """Result envelope for one claimed, resolver-backed durable brain job."""

    status: str
    job: Mapping[str, Any]
    cycle: BrainLearningCycleResult | None
    error_class: str | None = None
    workflow: Any | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "bioprism-brain-job-run/0.1",
            "status": self.status,
            "job": dict(self.job),
            "cycle": None if self.cycle is None else self.cycle.to_dict(),
            "workflow": None if self.workflow is None else self.workflow.to_dict(),
            "error_class": self.error_class,
            "retention": "job_metadata_and_learning_digests_only; workflow_checkpoint_caller_owned",
        }


def _mission_tool_identifier(value: Any) -> bool:
    return isinstance(value, str) and bool(value) and all(
        character.isalnum() or character == "_" for character in value
    )


def _json_digest(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _mission_wire_output(value: Any) -> Any:
    """Extract structured tool output while dropping opaque wire envelopes."""

    if not isinstance(value, Mapping):
        return None
    result = value.get("result")
    if not isinstance(result, Mapping):
        return None
    structured = result.get("structuredContent")
    if structured is not None:
        return structured
    content = result.get("content")
    if isinstance(content, list) and content:
        first = content[0]
        if isinstance(first, Mapping) and isinstance(first.get("text"), str):
            try:
                return json.loads(first["text"])
            except (TypeError, ValueError):
                return None
    return None


def _bounded_mission_report_projection(
    report: Mapping[str, Any],
    *,
    include_outputs: bool,
) -> dict[str, Any]:
    """Project an agent_mission report for continuation without replaying opaque envelopes."""

    if not isinstance(report, Mapping):
        raise BrainRunError("agent_mission returned a non-object report")
    if report.get("workflow") not in (None, "agent_mission"):
        raise BrainRunError("agent_mission returned the wrong workflow")
    projection: dict[str, Any] = {
        "workflow": "agent_mission",
        "ok": report.get("ok", True),
        "execution": report.get("execution", "unknown"),
        "mission_status": report.get("mission_status", "unknown"),
        "dispatch": report.get("dispatch", "unknown"),
        "preflight": report.get("preflight", False),
        "plan_digest": None,
        "succeeded": report.get("succeeded", 0),
        "refused": report.get("refused", 0),
        "blocked": report.get("blocked", 0),
        "cancelled": report.get("cancelled", 0),
        "required_failures": report.get("required_failures", 0),
        "returned_bytes": report.get("returned_bytes", 0),
        "results": [],
        "result_digest": _json_digest(dict(report)),
        "retention": "structured_step_outputs_only",
    }
    plan = report.get("plan")
    if isinstance(plan, Mapping):
        projection["plan_digest"] = plan.get("digest") or plan.get("plan_digest")
        projection["mission_id"] = plan.get("mission_id")
    raw_results = report.get("results", [])
    if isinstance(raw_results, list):
        for raw in raw_results[:MAX_MISSION_AUTHORIZATION_CALLS]:
            if not isinstance(raw, Mapping):
                continue
            row: dict[str, Any] = {
                "id": raw.get("id"),
                "tool": raw.get("tool"),
                "status": raw.get("status"),
                "required": raw.get("required"),
                "arguments_digest": raw.get("arguments_digest"),
                "bytes": raw.get("bytes", 0),
            }
            if raw.get("error") is not None:
                row["error_digest"] = _json_digest({"error": raw.get("error")})
            if include_outputs:
                output = _mission_wire_output(raw.get("wire"))
                if output is not None:
                    encoded_output = json.dumps(
                        output,
                        ensure_ascii=False,
                        sort_keys=True,
                        separators=(",", ":"),
                        allow_nan=False,
                    ).encode("utf-8")
                    if len(encoded_output) <= MAX_MISSION_AUTHORIZATION_STEP_OUTPUT_BYTES:
                        row["output"] = output
                    else:
                        row["output_digest"] = hashlib.sha256(encoded_output).hexdigest()
                elif raw.get("wire") is not None:
                    row["output_digest"] = _json_digest(raw.get("wire"))
            projection["results"].append(row)
    BrainLearningLedger._assert_safe(projection)
    encoded = json.dumps(
        projection,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    if len(encoded) > MAX_MISSION_AUTHORIZATION_RESULT_BYTES:
        raise BrainRunError("agent_mission continuation result exceeds the bounded size")
    return projection


@dataclass(frozen=True, slots=True)
class MissionAuthorizationReceipt:
    """One caller-owned tool-intent authorization attempt and its bounded evidence."""

    mission_id: str
    call_ids: tuple[str, ...]
    status: str
    preflight: Mapping[str, Any]
    execution: Mapping[str, Any] | None
    result: Mapping[str, Any]

    def __post_init__(self) -> None:
        if self.status not in {
            "preflight_refused",
            "approval_required",
            "executed",
            "execution_refused",
            "execution_failed",
        }:
            raise BrainRunError("mission authorization receipt has an invalid status")
        if not self.mission_id or not self.call_ids:
            raise BrainRunError("mission authorization receipt is missing identity")

    def to_dict(self) -> dict[str, Any]:
        return {
            "mission_id": self.mission_id,
            "call_ids": list(self.call_ids),
            "status": self.status,
            "preflight": dict(self.preflight),
            "execution": None if self.execution is None else dict(self.execution),
            "result": dict(self.result),
            "authorization": "caller_owned",
        }


class MissionToolAuthorizer:
    """Route, preflight, and optionally dispatch native provider tool intents.

    The object is intentionally a callable so it can be passed directly to
    :meth:`LLMRuntime.invoke_tool_loop` or :meth:`AutonomousBrain.run_tool_loop`. It never treats
    a route recommendation as permission: every call must pass the caller policy, the route
    candidate set, the local route schema when available, and the authoritative ``agent_mission``
    preflight. Dispatch remains disabled unless ``approve_mission_dispatch`` is true.
    """

    def __init__(
        self,
        workspace: BrainWorkspace,
        *,
        task: str,
        mission_policy: MissionPolicy | Mapping[str, Any],
        route: Mapping[str, Any] | None = None,
        approve_mission_dispatch: bool = False,
        mission_id_prefix: str = "brain-tool",
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
        operations_gate_acceptance: Mapping[str, Any] | None = None,
    ) -> None:
        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("mission authorizer task must be non-empty")
        if not hasattr(workspace, "tool") or not callable(getattr(workspace, "tool")):
            raise BrainRunError("mission authorizer requires a workspace tool boundary")
        if not isinstance(mission_policy, (MissionPolicy, Mapping)):
            raise BrainRunError("mission authorizer policy must be a MissionPolicy or mapping")
        normalized_policy = (
            mission_policy.to_dict()
            if isinstance(mission_policy, MissionPolicy)
            else dict(mission_policy)
        )
        allowed = normalized_policy.get("allowed_tools")
        if not isinstance(allowed, Sequence) or isinstance(allowed, (str, bytes)) or not allowed:
            raise BrainRunError("mission authorizer requires an explicit allowed_tools policy")
        if any(not _mission_tool_identifier(tool) for tool in allowed):
            raise BrainRunError("mission authorizer policy contains an unsafe tool name")
        BrainLearningLedger._assert_safe(normalized_policy)
        if not isinstance(approve_mission_dispatch, bool):
            raise BrainRunError("approve_mission_dispatch must be a boolean")
        if not isinstance(claim_requests, Sequence) or isinstance(claim_requests, (str, bytes)):
            raise BrainRunError("mission authorizer claim_requests must be a sequence")
        if any(not isinstance(value, Mapping) for value in claim_requests):
            raise BrainRunError("mission authorizer claim_requests must contain mappings")
        if evaluator_review is not None and not isinstance(evaluator_review, Mapping):
            raise BrainRunError("mission authorizer evaluator_review must be a mapping")
        if workflow_binding is not None and not isinstance(workflow_binding, Mapping):
            raise BrainRunError("mission authorizer workflow_binding must be a mapping")
        if operations_gate_acceptance is not None and not isinstance(operations_gate_acceptance, Mapping):
            raise BrainRunError("mission authorizer operations_gate_acceptance must be a mapping")
        BrainLearningLedger._assert_safe(
            {
                "claim_requests": list(claim_requests),
                "evaluator_review": evaluator_review,
                "workflow_binding": workflow_binding,
                "operations_gate_acceptance": operations_gate_acceptance,
            }
        )
        self.workspace = workspace
        self.task = task
        self.policy = normalized_policy
        self.policy["execute"] = False
        self.route = None if route is None else dict(route)
        self.approve_mission_dispatch = approve_mission_dispatch
        self.mission_id_prefix = mission_id_prefix
        self.claim_requests = tuple(dict(value) for value in claim_requests)
        self.evaluator_review = None if evaluator_review is None else dict(evaluator_review)
        self.workflow_binding = None if workflow_binding is None else dict(workflow_binding)
        self.operations_gate_acceptance = (
            None if operations_gate_acceptance is None else dict(operations_gate_acceptance)
        )
        self._receipts: list[MissionAuthorizationReceipt] = []
        self._invocation = 0
        self._route_recommended: set[str] | None = None
        self._route_candidates: dict[str, tuple[str, ...]] = {}
        self._route_metadata: dict[str, tuple[str, str, str]] = {}
        self._schema_catalogue: ToolCatalogue | None = None
        if self.route is not None:
            self._configure_route(self.route)

    @property
    def receipts(self) -> tuple[MissionAuthorizationReceipt, ...]:
        return tuple(self._receipts)

    def __call__(
        self,
        calls: tuple[ProviderToolCall, ...],
    ) -> tuple[ProviderToolResult, ...]:
        if not isinstance(calls, tuple) or not calls or len(calls) > MAX_MISSION_AUTHORIZATION_CALLS:
            raise BrainRunError("mission authorizer received an invalid tool-call batch")
        if any(not isinstance(call, ProviderToolCall) for call in calls):
            raise BrainRunError("mission authorizer received malformed tool calls")
        self._invocation += 1
        call_ids = tuple(call.call_id for call in calls)
        if len(set(call_ids)) != len(call_ids):
            return self._refuse(calls, "duplicate provider tool call ids")
        validation_error = self._validate_calls(calls)
        if validation_error is not None:
            return self._refuse(calls, validation_error)
        mission_id = self._mission_id(calls)
        steps = []
        for index, call in enumerate(calls):
            domain, capability, objective = self._route_metadata.get(
                call.name,
                ("cross_domain", call.name, f"Execute caller-authorized tool intent {call.name}"),
            )
            steps.append(
                {
                    "id": f"provider-tool-{self._invocation}-{index}",
                    "domain": domain,
                    "capability": capability,
                    "objective": objective,
                    "tool": call.name,
                    "arguments": dict(call.arguments),
                    "required": True,
                    "depends_on": [],
                    "bindings": [],
                }
            )
        request = MissionRequest(
            mission_id=mission_id,
            goal=self.task,
            steps=steps,
            policy=dict(self.policy),
            claim_requests=self.claim_requests,
            evaluator_review=self.evaluator_review,
            workflow_binding=self.workflow_binding,
            operations_gate_acceptance=self.operations_gate_acceptance,
        )
        try:
            preflight_raw = self.workspace.tool("agent_mission", request.to_mcp_arguments())
            preflight = _bounded_mission_report_projection(preflight_raw, include_outputs=False)
        except Exception as error:
            return self._refuse(calls, "mission preflight transport or validation failed", mission_id=mission_id)
        if not self._preflight_ready(preflight_raw):
            self._record_receipt(
                mission_id,
                calls,
                "preflight_refused",
                preflight,
                None,
                preflight,
            )
            return tuple(
                ProviderToolResult(call.call_id, preflight, approved=False, is_error=True)
                for call in calls
            )
        if not self.approve_mission_dispatch:
            self._record_receipt(
                mission_id,
                calls,
                "approval_required",
                preflight,
                None,
                preflight,
            )
            return tuple(
                ProviderToolResult(call.call_id, preflight, approved=False, is_error=True)
                for call in calls
            )
        execute_policy = dict(self.policy)
        execute_policy["execute"] = True
        execute_request = MissionRequest(
            mission_id=mission_id,
            goal=self.task,
            steps=steps,
            policy=execute_policy,
            claim_requests=self.claim_requests,
            evaluator_review=self.evaluator_review,
            workflow_binding=self.workflow_binding,
            operations_gate_acceptance=self.operations_gate_acceptance,
        )
        try:
            execution_raw = self.workspace.tool("agent_mission", execute_request.to_mcp_arguments())
            execution = _bounded_mission_report_projection(execution_raw, include_outputs=True)
        except Exception:
            execution = {
                "workflow": "agent_mission",
                "ok": False,
                "execution": "refused",
                "mission_status": "failed",
                "result_digest": _json_digest({"mission_id": mission_id, "status": "transport_failed"}),
                "retention": "structured_step_outputs_only",
            }
            self._record_receipt(
                mission_id,
                calls,
                "execution_failed",
                preflight,
                execution,
                execution,
            )
            return tuple(
                ProviderToolResult(call.call_id, execution, approved=False, is_error=True)
                for call in calls
            )
        mission_status = execution.get("mission_status")
        status = "executed" if mission_status == "succeeded" else (
            "execution_refused" if mission_status in {"refused", "blocked", "cancelled"} else "execution_failed"
        )
        self._record_receipt(mission_id, calls, status, preflight, execution, execution)
        return tuple(
            ProviderToolResult(
                call.call_id,
                execution,
                approved=True,
                is_error=status != "executed",
            )
            for call in calls
        )

    def _configure_route(self, route: Mapping[str, Any]) -> None:
        if route.get("workflow") != "capability_route":
            raise BrainRunError("mission authorizer route must be a capability_route report")
        if route.get("goal") != self.task:
            raise BrainRunError("mission authorizer route goal must match the task")
        unresolved = route.get("unresolved_needs", [])
        if not isinstance(unresolved, list) or unresolved:
            raise BrainRunError("mission authorizer route contains unresolved needs")
        recommended = route.get("recommended_tools")
        needs = route.get("needs")
        if not isinstance(recommended, list) or any(not isinstance(tool, str) for tool in recommended):
            raise BrainRunError("mission authorizer route has malformed recommended_tools")
        if not isinstance(needs, list) or any(not isinstance(need, Mapping) for need in needs):
            raise BrainRunError("mission authorizer route has malformed needs")
        self._route_recommended = set(recommended)
        for need in needs:
            need_id = need.get("id")
            candidate_tools = need.get("candidate_tools", [])
            if not isinstance(need_id, str) or not isinstance(candidate_tools, list):
                raise BrainRunError("mission authorizer route need is malformed")
            domains = need.get("candidate_domains", [])
            groups = need.get("candidate_groups", [])
            domain = domains[0] if isinstance(domains, list) and domains and isinstance(domains[0], str) else "cross_domain"
            capability = groups[0] if isinstance(groups, list) and groups and isinstance(groups[0], str) else need_id
            objective = need.get("query") if isinstance(need.get("query"), str) else f"Resolve routed need {need_id}"
            self._route_metadata.update(
                {tool: (domain, capability, objective) for tool in candidate_tools if isinstance(tool, str)}
            )
        raw_schemas = route.get("tool_schemas", [])
        omitted = route.get("tool_schemas_omitted", 0)
        if isinstance(raw_schemas, list) and raw_schemas and omitted == 0:
            try:
                self._schema_catalogue = ToolCatalogue.from_definitions(raw_schemas)
            except (ArgumentError, ToolSchemaError, TypeError, ValueError) as error:
                raise BrainRunError("mission authorizer route schemas are invalid") from error

    def _validate_calls(self, calls: Sequence[ProviderToolCall]) -> str | None:
        allowed = set(self.policy["allowed_tools"])
        for call in calls:
            if not _mission_tool_identifier(call.name):
                return f"tool {call.name!r} is not an executable mission tool identifier"
            if call.name not in allowed:
                return f"tool {call.name!r} is not in the caller mission policy"
            if self._route_recommended is not None and call.name not in self._route_recommended:
                return f"tool {call.name!r} is not recommended by the live route"
            if self._route_recommended is not None and call.name not in self._route_metadata:
                return f"tool {call.name!r} is not attached to a resolved route need"
            if self._schema_catalogue is not None:
                try:
                    report = self._schema_catalogue.validate(call.name, call.arguments)
                except ToolSchemaError:
                    return f"tool {call.name!r} is absent from the retained route schema set"
                if not report.ok:
                    return f"tool {call.name!r} failed route schema preflight"
        return None

    def _mission_id(self, calls: Sequence[ProviderToolCall]) -> str:
        digest = _json_digest([call.to_dict() for call in calls])[:32]
        prefix = self.mission_id_prefix if _mission_tool_identifier(self.mission_id_prefix) else "brain_tool"
        return f"{prefix}-{self._invocation}-{digest}"

    def _preflight_ready(self, report: Mapping[str, Any]) -> bool:
        if not isinstance(report, Mapping) or report.get("ok") is False:
            return False
        if report.get("workflow") not in (None, "agent_mission"):
            return False
        if report.get("dispatch") in {"executed", "started"}:
            return False
        return report.get("mission_status") in {None, "planned", "succeeded"} or report.get("execution") == "planned"

    def _record_receipt(
        self,
        mission_id: str,
        calls: Sequence[ProviderToolCall],
        status: str,
        preflight: Mapping[str, Any],
        execution: Mapping[str, Any] | None,
        result: Mapping[str, Any],
    ) -> None:
        self._receipts.append(
            MissionAuthorizationReceipt(
                mission_id=mission_id,
                call_ids=tuple(call.call_id for call in calls),
                status=status,
                preflight=preflight,
                execution=execution,
                result=result,
            )
        )

    def _refuse(
        self,
        calls: Sequence[ProviderToolCall],
        reason: str,
        *,
        mission_id: str | None = None,
    ) -> tuple[ProviderToolResult, ...]:
        projection = {
            "workflow": "agent_mission",
            "ok": False,
            "execution": "not_started",
            "mission_status": "refused",
            "refusal": reason,
            "result_digest": _json_digest({"reason": reason}),
            "retention": "structured_step_outputs_only",
        }
        self._receipts.append(
            MissionAuthorizationReceipt(
                mission_id=mission_id or f"{self.mission_id_prefix}-refused-{self._invocation}",
                call_ids=tuple(call.call_id for call in calls),
                status="preflight_refused",
                preflight=projection,
                execution=None,
                result=projection,
            )
        )
        return tuple(
            ProviderToolResult(call.call_id, projection, approved=False, is_error=True)
            for call in calls
        )


class AutonomousBrain:
    """Coordinate the value-only Rust kernel with a real caller-approved provider invocation."""

    def __init__(
        self,
        workspace: BrainWorkspace,
        runtime: LLMRuntime,
        memory: BrainEpisodicMemory | None = None,
    ) -> None:
        self.workspace = workspace
        self.runtime = runtime
        if memory is not None and not isinstance(memory, BrainEpisodicMemory):
            raise BrainRunError("memory must be a BrainEpisodicMemory or None")
        self.memory = memory

    def prepare_autonomous(self, **kwargs: Any) -> Any:
        """Build a domain-aware task blueprint without contacting a provider.

        The import is local to keep the low-level brain kernel independent from the convenience
        orchestration layer. The returned blueprint contains only transient task material and
        value-only public metadata; credentials are never accepted by this preparation method.
        """

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).prepare(**kwargs)

    def prepare_cross_domain(self, **kwargs: Any) -> Any:
        """Build bounded fan-out/fan-in domain work without contacting a provider."""

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).prepare_cross_domain(**kwargs)

    def run_autonomous(self, **kwargs: Any) -> Any:
        """Run a domain-aware task through adaptive selection and bounded provider execution.

        Use ``learn=True`` to require explicit evaluator evidence, update caller-owned bandit
        state, and append a metadata-only episodic record. Provider and mission approval flags are
        deliberately forwarded unchanged; this convenience method does not widen authority.
        """

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).run(**kwargs)

    def run_workflow(self, **kwargs: Any) -> Any:
        """Execute a prepared domain workflow as a resumable stage dependency graph.

        Stage outputs are structured and checkpointable; approval, malformed evidence, and
        model-declared uncertainty stop the graph without replaying completed stages.
        """

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).run_workflow(**kwargs)

    def run_workflow_learning(self, **kwargs: Any) -> Any:
        """Execute workflow stages and apply explicit per-stage evaluator updates."""

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).run_workflow_learning(**kwargs)

    def run_cross_domain(self, **kwargs: Any) -> Any:
        """Run bounded domain specialists and an optional cross-domain synthesis."""

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).run_cross_domain(**kwargs)

    def recall_memory(
        self,
        query: MemoryQuery | Mapping[str, Any] | None = None,
        *,
        limit: int | None = None,
        memory: BrainEpisodicMemory | None = None,
    ) -> list[dict[str, Any]]:
        """Recall bounded metadata/lessons from the configured episodic memory."""

        store = memory or self.memory
        if store is None:
            raise BrainRunError("episodic memory is not configured")
        if not isinstance(store, BrainEpisodicMemory):
            raise BrainRunError("memory must be a BrainEpisodicMemory")
        try:
            return store.retrieve(query, limit=limit)
        except BrainMemoryError as error:
            raise BrainRunError("episodic memory retrieval failed") from error

    @staticmethod
    def _result_memory_kind(
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
    ) -> tuple[BrainRunResult, str, str]:
        if isinstance(result, BrainRunResult):
            return result, "run", result.status
        if isinstance(result, BrainToolLoopResult):
            return result.brain_run, "tool_loop", result.status
        if isinstance(result, BrainMissionResult):
            return result.brain_run, "mission", result.status
        raise BrainRunError("result must be a BrainRunResult, BrainToolLoopResult, or BrainMissionResult")

    def remember_result(
        self,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
        *,
        task: str,
        episode_id: str | None = None,
        context: Mapping[str, Any] | None = None,
        tags: Sequence[str] = (),
        lesson: str | None = None,
        provenance: Mapping[str, Any] | None = None,
        memory: BrainEpisodicMemory | None = None,
    ) -> dict[str, Any]:
        """Persist one run as metadata-only episodic memory.

        The task is immediately reduced to a digest.  The provider response, prompt, tool
        arguments, and credentials are never passed to the memory store.
        """

        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("task must be a non-empty string")
        if context is not None and not isinstance(context, Mapping):
            raise BrainRunError("memory context must be a mapping or None")
        if not isinstance(tags, Sequence) or isinstance(tags, (str, bytes)):
            raise BrainRunError("memory tags must be a string sequence")
        if any(not isinstance(tag, str) or not tag.strip() for tag in tags):
            raise BrainRunError("memory tags must contain non-empty strings")
        if provenance is not None and not isinstance(provenance, Mapping):
            raise BrainRunError("memory provenance must be a mapping or None")
        store = memory or self.memory
        if store is None:
            raise BrainRunError("episodic memory is not configured")
        if not isinstance(store, BrainEpisodicMemory):
            raise BrainRunError("memory must be a BrainEpisodicMemory")
        brain_result, result_kind, status = self._result_memory_kind(result)
        selected = brain_result.selection.get("selected_model")
        if not isinstance(selected, Mapping):
            raise BrainRunError("cannot remember a result without selected model metadata")
        selected_model = {
            "provider": selected.get("provider"),
            "model": selected.get("model"),
        }
        if not isinstance(selected_model["provider"], str) or not isinstance(selected_model["model"], str):
            raise BrainRunError("selected model metadata is malformed")
        evaluator_input = build_brain_evaluation_input(result)
        context_copy = {} if context is None else dict(context)
        route = None
        if isinstance(result, (BrainToolLoopResult, BrainMissionResult)) and result.route is not None:
            route = {"route_digest": _json_digest(dict(result.route))}
        plan = brain_result.plan.get("plan")
        digests = {
            "selection_digest": brain_result.selection.get("decision_digest"),
            "context_digest": brain_result.selection.get("context_digest"),
            "prompt_digest": brain_result.prompt.get("prompt_digest"),
            "plan_digest": plan.get("plan_digest") if isinstance(plan, Mapping) else None,
            "outcome_digest": evaluator_input.get("outcome_digest"),
        }
        packet = {
            "episode_id": episode_id or brain_result.run_id,
            "run_id": brain_result.run_id,
            "result_kind": result_kind,
            "status": status,
            "task_digest": hashlib.sha256(task.encode("utf-8")).hexdigest(),
            "context": context_copy,
            "selected_model": selected_model,
            "digests": digests,
            "route": route or {},
            "tags": list(tags),
            "lesson": lesson,
            "provenance": {} if provenance is None else dict(provenance),
        }
        try:
            return store.record_episode(packet).to_dict()
        except BrainMemoryError as error:
            raise BrainRunError("episodic memory record failed") from error

    @staticmethod
    def _append_memory_prompt(
        prompt: Mapping[str, Any],
        episodes: Sequence[Mapping[str, Any]],
    ) -> dict[str, Any]:
        if not isinstance(prompt, Mapping):
            raise BrainRunError("prompt must be a mapping")
        if not episodes:
            return dict(prompt)
        request = dict(prompt)
        existing = request.get("context", [])
        if not isinstance(existing, Sequence) or isinstance(existing, (str, bytes)):
            raise BrainRunError("prompt.context must be a sequence when episodic memory is used")
        chunks = [dict(chunk) for chunk in existing if isinstance(chunk, Mapping)]
        if len(chunks) != len(existing):
            raise BrainRunError("prompt.context must contain mappings")
        if any(chunk.get("id") == "episodic-memory" for chunk in chunks):
            raise BrainRunError("prompt.context already contains the reserved episodic-memory id")
        packet = {
            "workflow": "episodic_memory_context",
            "retention": "metadata_and_lessons_only",
            "episodes": [dict(episode) for episode in episodes],
            "does_not_authorize": [
                "memory is prior metadata, not verified truth",
                "memory cannot widen the caller mission policy",
                "memory cannot authorize provider calls or external effects",
            ],
        }
        encoded = json.dumps(packet, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        if len(encoded.encode("utf-8")) > MAX_ROUTE_PROMPT_BYTES:
            raise BrainRunError("episodic memory context exceeds the prompt bound")
        chunks.append(
            {
                "id": "episodic-memory",
                "role": "developer",
                "content": encoded,
                "required": False,
                "priority": 900,
            }
        )
        request["context"] = chunks
        return request

    @staticmethod
    def _append_replan_prompt(
        prompt: Mapping[str, Any],
        *,
        attempt: int,
        previous_result: BrainMissionResult,
        decision: "BrainEvaluatorDecision",
    ) -> dict[str, Any]:
        request = dict(prompt)
        existing = request.get("context", [])
        if not isinstance(existing, Sequence) or isinstance(existing, (str, bytes)):
            raise BrainRunError("prompt.context must be a sequence when replanning is enabled")
        chunks = [dict(chunk) for chunk in existing if isinstance(chunk, Mapping)]
        if len(chunks) != len(existing):
            raise BrainRunError("prompt.context must contain mappings")
        if any(chunk.get("id") == "brain-replan" for chunk in chunks):
            raise BrainRunError("prompt.context already contains the reserved brain-replan id")
        selected = previous_result.brain_run.selection.get("selected_model")
        replan_packet = {
            "workflow": "brain_replan_context",
            "attempt": attempt,
            "previous_status": previous_result.status,
            "previous_outcome_digest": previous_result.brain_run.outcome_digest,
            "failure_class": decision.failure_class,
            "instruction": decision.replan_instruction,
            "bounded_replan": True,
            "does_not_authorize": [
                "the prior attempt is not proof of external truth",
                "the caller mission policy remains unchanged",
                "this proposal cannot dispatch itself",
            ],
        }
        if isinstance(selected, Mapping):
            replan_packet["previous_model"] = {
                "provider": selected.get("provider"),
                "model": selected.get("model"),
            }
        encoded = json.dumps(replan_packet, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        if len(encoded.encode("utf-8")) > MAX_ROUTE_PROMPT_BYTES:
            raise BrainRunError("replan context exceeds the prompt bound")
        chunks.append(
            {
                "id": "brain-replan",
                "role": "developer",
                "content": encoded,
                "required": True,
                "priority": 950,
            }
        )
        request["context"] = chunks
        return request

    def build_adaptive_model_selection(
        self,
        *,
        task: str,
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        ledger: BrainLearningLedger | None = None,
        context: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        required_capabilities: Sequence[str] = (),
        input_tokens: int = 4_096,
        requested_output_tokens: int = 2_048,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        selection_overrides: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Build a live model-selection request from registered transports and learned state.

        Applications own the model catalogue because provider model availability and pricing are
        deployment-specific. The brain owns the decision: it removes candidates whose transport
        is not registered or whose required user credential handle is absent, projects persisted
        bandit state into the Rust selector, and scopes observations to an optional domain /
        capability / risk context. No provider secret enters this request.
        """

        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("task must be a non-empty string")
        if not isinstance(model_candidates, Sequence) or isinstance(model_candidates, (str, bytes)):
            raise BrainRunError("model_candidates must be a sequence")
        if not model_candidates:
            raise BrainRunError("model_candidates must not be empty")
        if not isinstance(credentials, Mapping):
            raise BrainRunError("credentials must be a mapping")
        if not isinstance(required_capabilities, Sequence) or isinstance(
            required_capabilities, (str, bytes)
        ):
            raise BrainRunError("required_capabilities must be a sequence")
        if any(not isinstance(capability, str) or not capability.strip() for capability in required_capabilities):
            raise BrainRunError("required_capabilities must contain non-empty strings")
        if not isinstance(input_tokens, int) or isinstance(input_tokens, bool) or input_tokens < 1:
            raise BrainRunError("input_tokens must be a positive integer")
        if not isinstance(requested_output_tokens, int) or isinstance(requested_output_tokens, bool) or requested_output_tokens < 1:
            raise BrainRunError("requested_output_tokens must be a positive integer")
        for name, value in (("max_cost_per_million_tokens", max_cost_per_million_tokens), ("max_latency_ms", max_latency_ms)):
            if value is not None and (not isinstance(value, int) or isinstance(value, bool) or value < 0):
                raise BrainRunError(f"{name} must be a non-negative integer or None")
        if min_quality is not None and (
            not isinstance(min_quality, (int, float))
            or isinstance(min_quality, bool)
            or not 0 <= min_quality <= 1
        ):
            raise BrainRunError("min_quality must be within [0, 1] or None")
        if ledger is not None and not isinstance(ledger, BrainLearningLedger):
            raise BrainRunError("ledger must be a BrainLearningLedger or None")
        if context is not None and not isinstance(context, Mapping):
            raise BrainRunError("context must be a mapping or None")
        if not isinstance(contextual_observations, Sequence) or isinstance(
            contextual_observations, (str, bytes)
        ):
            raise BrainRunError("contextual_observations must be a sequence")
        if any(not isinstance(observation, Mapping) for observation in contextual_observations):
            raise BrainRunError("contextual_observations must contain mappings")
        if selection_overrides is not None and not isinstance(selection_overrides, Mapping):
            raise BrainRunError("selection_overrides must be a mapping or None")
        if selection_overrides is not None:
            BrainLearningLedger._assert_safe(selection_overrides)
        health_overrides: Mapping[str, Any] = {}
        if selection_overrides is not None and selection_overrides.get("provider_health") is not None:
            raw_health_overrides = selection_overrides.get("provider_health")
            if not isinstance(raw_health_overrides, Mapping):
                raise BrainRunError("selection_overrides.provider_health must be a mapping")
            health_overrides = raw_health_overrides

        provider_metadata = {
            row.get("provider"): row
            for row in self.runtime.provider_metadata()
            if isinstance(row, Mapping) and isinstance(row.get("provider"), str)
        }
        provider_health: dict[str, dict[str, Any]] = {}
        for provider, metadata in provider_metadata.items():
            status = self.runtime.provider_status(provider)
            provider_health[provider] = {
                "registered": True,
                "circuit": status.get("circuit"),
                "consecutive_failures": status.get("consecutive_failures", 0),
                "credential_ready": (
                    not bool(metadata.get("requires_credential", True))
                    or (
                        isinstance(credentials.get(provider), CredentialHandle)
                        and credentials[provider].provider == provider
                    )
                ),
            }
        normalized_models: list[dict[str, Any]] = []
        for candidate in model_candidates:
            if not isinstance(candidate, Mapping):
                raise BrainRunError("model_candidates must contain mappings")
            BrainLearningLedger._assert_safe(candidate)
            model = dict(candidate)
            for field in (
                "provider",
                "model",
                "context_window_tokens",
                "max_output_tokens",
                "quality",
                "latency_ms",
                "cost_per_million_tokens",
            ):
                if field not in model:
                    raise BrainRunError(f"model candidate is missing {field}")
            provider = model.get("provider")
            model_name = model.get("model")
            if not isinstance(provider, str) or not provider.strip() or not isinstance(model_name, str) or not model_name.strip():
                raise BrainRunError("model candidate provider and model must be non-empty strings")
            capabilities = model.get("capabilities", [])
            if not isinstance(capabilities, Sequence) or isinstance(capabilities, (str, bytes)) or any(
                not isinstance(capability, str) for capability in capabilities
            ):
                raise BrainRunError("model candidate capabilities must be a string sequence")
            model["capabilities"] = list(capabilities)
            model.setdefault("requires_credential", True)
            model.setdefault("enabled", True)
            if not isinstance(model["requires_credential"], bool) or not isinstance(model["enabled"], bool):
                raise BrainRunError("model candidate requires_credential and enabled must be booleans")
            registered = provider_metadata.get(provider)
            runtime_requires_credential = True if registered is None else bool(
                registered.get("requires_credential", True)
            )
            requires_credential = bool(model["requires_credential"]) or runtime_requires_credential
            model["requires_credential"] = requires_credential
            health = provider_health.get(provider)
            if health is None:
                health = provider_health[provider] = {
                    "registered": False,
                    "circuit": "unconfigured",
                    "consecutive_failures": 0,
                    "credential_ready": False,
                    "eligible": False,
                }
            if registered is None:
                model["enabled"] = False
            elif requires_credential:
                handle = credentials.get(provider)
                credential_ready = (
                    isinstance(handle, CredentialHandle)
                    and handle.provider == provider
                )
                if credential_ready:
                    try:
                        # Resolve only metadata here. This verifies that the handle belongs to
                        # this runtime and has not expired or been revoked without exposing the
                        # underlying value to the selector or Rust kernel.
                        self.runtime.credentials.metadata(handle)  # type: ignore[arg-type]
                    except CredentialError:
                        credential_ready = False
                health["credential_ready"] = credential_ready
                if not credential_ready:
                    model["enabled"] = False
            health = provider_health[provider]
            if health["circuit"] == "open":
                model["enabled"] = False
            health["eligible"] = bool(model["enabled"]) and bool(health["credential_ready"])
            normalized_models.append(model)

        # A durable health snapshot may add historical evidence to the live provider gate. It can
        # never make an unregistered or credential-ineligible provider eligible; an explicit open
        # historical circuit only narrows the candidate set until an operator resets it.
        for provider, historical in health_overrides.items():
            if not isinstance(provider, str) or not isinstance(historical, Mapping):
                raise BrainRunError("selection_overrides.provider_health must map provider names to objects")
            current = provider_health.setdefault(
                provider,
                {
                    "registered": False,
                    "circuit": "unconfigured",
                    "consecutive_failures": 0,
                    "credential_ready": False,
                    "eligible": False,
                },
            )
            current["historical"] = dict(historical)
            if historical.get("circuit") == "open":
                current["circuit"] = "open"
                for model in normalized_models:
                    if model.get("provider") == provider:
                        model["enabled"] = False
                        current["eligible"] = False

        global_state = None if ledger is None else ledger.latest_state()
        observations = _bandit_observations(global_state)
        scoped_observations: list[dict[str, Any]] = []
        if context is not None:
            context_digest = _context_identity_digest(context)
            scoped_state = None if ledger is None else ledger.latest_state(context_digest)
            scoped_by_arm = {
                observation["arm_id"]: observation
                for observation in _bandit_observations(scoped_state)
            }
            supplied = _bandit_observations({"arms": list(contextual_observations)})
            scoped_by_arm.update({observation["arm_id"]: observation for observation in supplied})
            scoped_observations = [
                {**observation, "context_digest": context_digest}
                for observation in scoped_by_arm.values()
            ]
        elif contextual_observations:
            raise BrainRunError("contextual_observations require context")

        request: dict[str, Any] = dict(selection_overrides or {})
        request.update(
            {
                "task": task,
                "required_capabilities": list(required_capabilities),
                "input_tokens": input_tokens,
                "requested_output_tokens": requested_output_tokens,
                "models": normalized_models,
                "observations": observations,
                "provider_health": provider_health,
            }
        )
        if max_cost_per_million_tokens is not None:
            request["max_cost_per_million_tokens"] = max_cost_per_million_tokens
        if max_latency_ms is not None:
            request["max_latency_ms"] = max_latency_ms
        if min_quality is not None:
            request["min_quality"] = min_quality
        if context is not None:
            request["context"] = dict(context)
            request["contextual_observations"] = scoped_observations
        BrainLearningLedger._assert_safe(request)
        try:
            json.dumps(request, ensure_ascii=False, allow_nan=False)
        except (TypeError, ValueError) as error:
            raise BrainRunError("adaptive model-selection request must be JSON-safe") from error
        return request

    def _prepare_adaptive_route(
        self,
        *,
        task: str,
        route_request: Mapping[str, Any],
    ) -> tuple[dict[str, Any], dict[str, Any]]:
        if not isinstance(route_request, Mapping):
            raise BrainRunError("route_request must be a mapping")
        BrainLearningLedger._assert_safe(route_request)
        arguments = dict(route_request)
        supplied_goal = arguments.get("goal")
        if supplied_goal is not None and supplied_goal != task:
            raise BrainRunError("route_request.goal must match the adaptive task")
        arguments["goal"] = task
        arguments.setdefault("needs", [{"id": "task", "query": task}])
        arguments.setdefault("include_tools", True)
        arguments.setdefault("max_tools", 128)
        try:
            encoded = json.dumps(
                arguments,
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("route_request must be JSON-safe") from error
        if len(encoded) > MAX_ROUTE_REQUEST_BYTES:
            raise BrainRunError("route_request exceeds the bounded size")
        response = self.workspace.tool("capability_route", arguments)
        if not isinstance(response, Mapping):
            raise BrainRunError("capability route returned a non-object")
        if response.get("ok") is False or response.get("workflow") != "capability_route":
            raise BrainRunError("capability route was refused")
        route = dict(response)
        BrainLearningLedger._assert_safe(route)
        context = _adaptive_route_context(route, task=task, route_request=arguments)
        return route, context

    def _preview_adaptive_selection(
        self,
        *,
        task: str,
        selection: Mapping[str, Any],
        context: Mapping[str, Any] | None,
    ) -> dict[str, Any]:
        arguments = dict(selection)
        arguments["task"] = task
        if context is None:
            report = self.workspace.tool("brain_model_select", arguments)
            if not isinstance(report, Mapping):
                raise BrainRunError("adaptive model selection preview returned a non-object")
            return dict(report)
        observations = selection.get("contextual_observations", [])
        if not isinstance(observations, list):
            raise BrainRunError("adaptive contextual selection observations are malformed")
        report = self.workspace.tool(
            "brain_model_select_contextual",
            {
                "context": dict(context),
                "base": arguments,
                "observations": [dict(observation) for observation in observations],
            },
        )
        if not isinstance(report, Mapping):
            raise BrainRunError("adaptive contextual selection preview returned a non-object")
        nested = report.get("selection")
        if not isinstance(nested, Mapping):
            raise BrainRunError("adaptive contextual selection preview omitted selection")
        return dict(nested)

    def run_adaptive(
        self,
        *,
        task: str,
        model_candidates: Sequence[Mapping[str, Any]],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        ledger: BrainLearningLedger | None = None,
        context: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        required_capabilities: Sequence[str] = (),
        input_tokens: int = 4_096,
        requested_output_tokens: int = 2_048,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        selection_overrides: Mapping[str, Any] | None = None,
        approve_provider_call: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 2_048,
        temperature: float | None = None,
        require_json: bool = False,
        response_schema: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
        max_provider_failovers: int = 2,
    ) -> BrainRunResult:
        """Select, plan, and invoke from live providers using caller-persisted learning state."""

        if not isinstance(max_provider_failovers, int) or isinstance(max_provider_failovers, bool) or not 0 <= max_provider_failovers <= 8:
            raise BrainRunError("max_provider_failovers must be within [0, 8]")

        selection = self.build_adaptive_model_selection(
            task=task,
            model_candidates=model_candidates,
            credentials=credentials,
            ledger=ledger,
            context=context,
            contextual_observations=contextual_observations,
            required_capabilities=required_capabilities,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            selection_overrides=selection_overrides,
        )
        effective_contextual_observations = (
            selection.get("contextual_observations", contextual_observations)
            if context is not None
            else contextual_observations
        )
        attempt_selection = dict(selection)
        failed_ids: set[str] = set()
        failover_attempts: list[dict[str, Any]] = []
        for attempt in range(max_provider_failovers + 1):
            if attempt:
                attempt_selection["models"] = [
                    {
                        **dict(candidate),
                        "enabled": False
                        if f"{candidate.get('provider')}/{candidate.get('model')}" in failed_ids
                        else candidate.get("enabled", True),
                    }
                    for candidate in selection.get("models", [])
                    if isinstance(candidate, Mapping)
                ]
            preview = self._preview_adaptive_selection(
                task=task,
                selection=attempt_selection,
                context=context,
            )
            selected = preview.get("selected_model")
            if not isinstance(selected, Mapping):
                raise BrainRunError("adaptive selection has no eligible provider after failover")
            provider = selected.get("provider")
            model = selected.get("model")
            if not isinstance(provider, str) or not isinstance(model, str):
                raise BrainRunError("adaptive selection returned malformed provider metadata")
            selected_id = f"{provider}/{model}"
            try:
                result = self.run(
                    task=task,
                    model_selection=attempt_selection,
                    prompt=prompt,
                    plan=plan,
                    credentials=credentials,
                    approve_provider_call=approve_provider_call,
                    run_id=run_id,
                    max_output_tokens=max_output_tokens,
                    temperature=temperature,
                    require_json=require_json,
                    response_schema=response_schema,
                    idempotency_key=idempotency_key,
                    context=context,
                    contextual_observations=effective_contextual_observations,
                    tools=tools,
                    tool_choice=tool_choice,
                )
                if not failover_attempts:
                    return result
                failover_attempts.append(
                    {
                        "attempt": attempt,
                        "provider": provider,
                        "model": model,
                        "arm_id": selected_id,
                        "status": "completed",
                    }
                )
                return replace(
                    result,
                    provider_failover={
                        "strategy": "deterministic_model_selector_with_provider_health_gating",
                        "attempts": list(failover_attempts),
                        "fallback_count": len(failover_attempts) - 1,
                        "retention": "metadata_only",
                    },
                )
            except ProviderError as error:
                failed_ids.add(selected_id)
                failover_attempts.append(
                    {
                        "attempt": attempt,
                        "provider": provider,
                        "model": model,
                        "arm_id": selected_id,
                        "status": "provider_refused",
                        "reason": "circuit_open" if error.circuit_open else "provider_error",
                        "status_code": error.status_code,
                    }
                )
                if attempt >= max_provider_failovers:
                    raise
        raise BrainRunError("adaptive provider failover exhausted")

    def run_adaptive_tool_loop(
        self,
        *,
        task: str,
        model_candidates: Sequence[Mapping[str, Any]],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        ledger: BrainLearningLedger | None = None,
        context: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        required_capabilities: Sequence[str] = (),
        input_tokens: int = 4_096,
        requested_output_tokens: int = 2_048,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        selection_overrides: Mapping[str, Any] | None = None,
        tool_loop_options: Mapping[str, Any] | None = None,
        max_provider_failovers: int = 2,
    ) -> BrainToolLoopResult:
        """Select adaptively, then enter the bounded route-aware native tool loop.

        ``tool_loop_options`` carries the explicit continuation/authorization options accepted by
        :meth:`run_tool_loop` (for example ``mission_policy``, ``route_request``,
        ``approve_mission_dispatch``, and ``provider_tools``). It intentionally cannot override
        the task, credentials, context, or learned selection assembled by this method.
        """

        if (
            not isinstance(max_provider_failovers, int)
            or isinstance(max_provider_failovers, bool)
            or not 0 <= max_provider_failovers <= 8
        ):
            raise BrainRunError("max_provider_failovers must be within [0, 8]")
        if not isinstance(tool_loop_options, (Mapping, type(None))):
            raise BrainRunError("tool_loop_options must be a mapping or None")
        options = {} if tool_loop_options is None else dict(tool_loop_options)
        allowed_options = {
            "authorize_and_execute",
            "approve_provider_call",
            "run_id",
            "max_output_tokens",
            "temperature",
            "require_json",
            "response_schema",
            "idempotency_key",
            "provider_tools",
            "tool_choice",
            "max_turns",
            "max_tool_calls",
            "stream",
            "mission_policy",
            "approve_mission_dispatch",
            "route_request",
            "enforce_route_tools",
            "require_resolved_route",
            "claim_requests",
            "evaluator_review",
            "workflow_binding",
            "operations_gate_acceptance",
        }
        unknown = sorted(set(options).difference(allowed_options))
        if unknown:
            raise BrainRunError(f"tool_loop_options contains unsupported fields: {', '.join(unknown)}")
        effective_context = context
        route_report: dict[str, Any] | None = None
        if "route_request" in options:
            route_report, route_context = self._prepare_adaptive_route(
                task=task,
                route_request=options["route_request"],
            )
            if effective_context is None:
                effective_context = route_context
            options["route_report"] = route_report
        selection = self.build_adaptive_model_selection(
            task=task,
            model_candidates=model_candidates,
            credentials=credentials,
            ledger=ledger,
            context=effective_context,
            contextual_observations=contextual_observations,
            required_capabilities=required_capabilities,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            selection_overrides=selection_overrides,
        )
        effective_contextual_observations = (
            selection.get("contextual_observations", contextual_observations)
            if effective_context is not None
            else contextual_observations
        )
        attempt_selection = dict(selection)
        failed_ids: set[str] = set()
        failover_attempts: list[dict[str, Any]] = []
        for attempt in range(max_provider_failovers + 1):
            if attempt:
                attempt_selection["models"] = [
                    {
                        **dict(candidate),
                        "enabled": False
                        if f"{candidate.get('provider')}/{candidate.get('model')}" in failed_ids
                        else candidate.get("enabled", True),
                    }
                    for candidate in selection.get("models", [])
                    if isinstance(candidate, Mapping)
                ]
            preview = self._preview_adaptive_selection(
                task=task,
                selection=attempt_selection,
                context=effective_context,
            )
            selected = preview.get("selected_model")
            if not isinstance(selected, Mapping):
                raise BrainRunError("adaptive tool-loop selection has no eligible provider after failover")
            provider = selected.get("provider")
            model = selected.get("model")
            if not isinstance(provider, str) or not isinstance(model, str):
                raise BrainRunError("adaptive tool-loop selection returned malformed provider metadata")
            selected_id = f"{provider}/{model}"
            attempt_state: dict[str, Any] = {}
            attempt_options = dict(options)
            attempt_options["attempt_state"] = attempt_state
            try:
                result = self.run_tool_loop(
                    task=task,
                    model_selection=attempt_selection,
                    prompt=prompt,
                    plan=plan,
                    credentials=credentials,
                    context=effective_context,
                    contextual_observations=effective_contextual_observations,
                    **attempt_options,
                )
                if not failover_attempts:
                    return result
                failover_attempts.append(
                    {
                        "attempt": attempt,
                        "provider": provider,
                        "model": model,
                        "arm_id": selected_id,
                        "status": "completed",
                    }
                )
                return replace(
                    result,
                    brain_run=replace(
                        result.brain_run,
                        provider_failover={
                            "strategy": "deterministic_tool_loop_selector_before_side_effects",
                            "attempts": list(failover_attempts),
                            "fallback_count": len(failover_attempts) - 1,
                            "retention": "metadata_only",
                        },
                    ),
                )
            except ProviderError as error:
                if attempt_state.get("tool_authorization_started"):
                    raise
                failed_ids.add(selected_id)
                failover_attempts.append(
                    {
                        "attempt": attempt,
                        "provider": provider,
                        "model": model,
                        "arm_id": selected_id,
                        "status": "provider_refused",
                        "reason": "circuit_open" if error.circuit_open else "provider_error",
                        "status_code": error.status_code,
                    }
                )
                if attempt >= max_provider_failovers:
                    raise
        raise BrainRunError("adaptive tool-loop provider failover exhausted")

    def run_adaptive_mission(
        self,
        *,
        task: str,
        model_candidates: Sequence[Mapping[str, Any]],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        mission_policy: MissionPolicy | Mapping[str, Any],
        ledger: BrainLearningLedger | None = None,
        context: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        required_capabilities: Sequence[str] = (),
        input_tokens: int = 4_096,
        requested_output_tokens: int = 2_048,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        selection_overrides: Mapping[str, Any] | None = None,
        approve_provider_call: bool = False,
        approve_mission_dispatch: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 2_048,
        temperature: float | None = None,
        response_schema: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
        route_review: Mapping[str, Any] | None = None,
        operations_gate_acceptance: Mapping[str, Any] | None = None,
        route_request: Mapping[str, Any] | None = None,
        enforce_route_tools: bool = True,
        require_resolved_route: bool = True,
        provider_tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
        max_provider_failovers: int = 2,
    ) -> BrainMissionResult:
        """Select, route, plan, and execute one bounded cross-domain mission.

        The route is resolved once and reused for contextual model selection, prompt assembly,
        tool narrowing, and mission authorization. Provider failover is allowed only while the
        model is still producing the mission proposal; once the proposal reaches ``agent_mission``
        this method never replays it against another provider.
        """

        if (
            not isinstance(max_provider_failovers, int)
            or isinstance(max_provider_failovers, bool)
            or not 0 <= max_provider_failovers <= 8
        ):
            raise BrainRunError("max_provider_failovers must be within [0, 8]")
        if route_request is not None and not isinstance(route_request, Mapping):
            raise BrainRunError("route_request must be a mapping or None")

        effective_context = context
        route_report: dict[str, Any] | None = None
        if route_request is not None:
            route_report, route_context = self._prepare_adaptive_route(
                task=task,
                route_request=route_request,
            )
            if effective_context is None:
                effective_context = route_context

        selection = self.build_adaptive_model_selection(
            task=task,
            model_candidates=model_candidates,
            credentials=credentials,
            ledger=ledger,
            context=effective_context,
            contextual_observations=contextual_observations,
            required_capabilities=required_capabilities,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            selection_overrides=selection_overrides,
        )
        effective_contextual_observations = (
            selection.get("contextual_observations", contextual_observations)
            if effective_context is not None
            else contextual_observations
        )
        attempt_selection = dict(selection)
        failed_ids: set[str] = set()
        failover_attempts: list[dict[str, Any]] = []
        for attempt in range(max_provider_failovers + 1):
            if attempt:
                attempt_selection["models"] = [
                    {
                        **dict(candidate),
                        "enabled": False
                        if f"{candidate.get('provider')}/{candidate.get('model')}" in failed_ids
                        else candidate.get("enabled", True),
                    }
                    for candidate in selection.get("models", [])
                    if isinstance(candidate, Mapping)
                ]
            preview = self._preview_adaptive_selection(
                task=task,
                selection=attempt_selection,
                context=effective_context,
            )
            selected = preview.get("selected_model")
            if not isinstance(selected, Mapping):
                raise BrainRunError("adaptive mission selection has no eligible provider after failover")
            provider = selected.get("provider")
            model = selected.get("model")
            if not isinstance(provider, str) or not isinstance(model, str):
                raise BrainRunError("adaptive mission selection returned malformed provider metadata")
            selected_id = f"{provider}/{model}"
            attempt_state: dict[str, Any] = {}
            try:
                result = self.run_mission(
                    task=task,
                    model_selection=attempt_selection,
                    prompt=prompt,
                    plan=plan,
                    credentials=credentials,
                    mission_policy=mission_policy,
                    approve_provider_call=approve_provider_call,
                    approve_mission_dispatch=approve_mission_dispatch,
                    run_id=run_id,
                    max_output_tokens=max_output_tokens,
                    temperature=temperature,
                    response_schema=response_schema,
                    idempotency_key=idempotency_key,
                    claim_requests=claim_requests,
                    context=effective_context,
                    contextual_observations=effective_contextual_observations,
                    evaluator_review=evaluator_review,
                    workflow_binding=workflow_binding,
                    route_review=route_review,
                    operations_gate_acceptance=operations_gate_acceptance,
                    route_request=route_request,
                    route_report=route_report,
                    enforce_route_tools=enforce_route_tools,
                    require_resolved_route=require_resolved_route,
                    provider_tools=provider_tools,
                    tool_choice=tool_choice,
                    attempt_state=attempt_state,
                )
                if not failover_attempts:
                    return result
                failover_attempts.append(
                    {
                        "attempt": attempt,
                        "provider": provider,
                        "model": model,
                        "arm_id": selected_id,
                        "status": "completed",
                    }
                )
                return replace(
                    result,
                    brain_run=replace(
                        result.brain_run,
                        provider_failover={
                            "strategy": "deterministic_mission_selector_before_dispatch",
                            "attempts": list(failover_attempts),
                            "fallback_count": len(failover_attempts) - 1,
                            "retention": "metadata_only",
                        },
                    ),
                )
            except ProviderError as error:
                if attempt_state.get("mission_dispatch_started"):
                    raise
                failed_ids.add(selected_id)
                failover_attempts.append(
                    {
                        "attempt": attempt,
                        "provider": provider,
                        "model": model,
                        "arm_id": selected_id,
                        "status": "provider_refused",
                        "reason": "circuit_open" if error.circuit_open else "provider_error",
                        "status_code": error.status_code,
                    }
                )
                if attempt >= max_provider_failovers:
                    raise
        raise BrainRunError("adaptive mission provider failover exhausted")

    def run_adaptive_mission_learning_cycle(
        self,
        *,
        task: str,
        model_candidates: Sequence[Mapping[str, Any]],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        mission_policy: MissionPolicy | Mapping[str, Any],
        evaluator: "BrainOutcomeEvaluator",
        bandit_state: Mapping[str, Any],
        provider_health: Mapping[str, Any] | None = None,
        ledger: BrainLearningLedger | None = None,
        memory: BrainEpisodicMemory | None = None,
        memory_query: MemoryQuery | Mapping[str, Any] | None = None,
        memory_limit: int = 8,
        memory_tags: Sequence[str] = (),
        evidence: Mapping[str, Any] | None = None,
        max_replans: int = 1,
        mission_options: Mapping[str, Any] | None = None,
    ) -> BrainLearningCycleResult:
        """Run, evaluate, remember, and boundedly replan a cross-domain mission.

        This is the high-level learning seam for applications that want the agent to improve
        across calls.  Recalled episodes are inserted as non-authorizing developer context.  Each
        outcome is sent through the explicit evaluator and Rust bandit recorder, then persisted as
        a separate memory evaluation event.  A replan can happen only when the evaluator requests
        one and the prior mission has not crossed the external-effect dispatch boundary.

        ``mission_options`` contains the optional keyword arguments accepted by
        :meth:`run_adaptive_mission`; keeping them in one mapping makes this orchestration API
        forward-compatible while rejecting accidental task/credential/policy overrides.
        """

        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("evaluator must be a BrainOutcomeEvaluator")
        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("bandit_state must be a mapping")
        if provider_health is not None:
            if not isinstance(provider_health, Mapping):
                raise BrainRunError("provider_health must be a mapping or None")
            BrainLearningLedger._assert_safe(provider_health)
        BrainLearningLedger._assert_safe(bandit_state)
        if not isinstance(max_replans, int) or isinstance(max_replans, bool) or not 0 <= max_replans <= 3:
            raise BrainRunError("max_replans must be within [0, 3]")
        if not isinstance(memory_limit, int) or isinstance(memory_limit, bool) or not 1 <= memory_limit <= 32:
            raise BrainRunError("memory_limit must be within [1, 32]")
        if not isinstance(memory_tags, Sequence) or isinstance(memory_tags, (str, bytes)):
            raise BrainRunError("memory_tags must be a string sequence")
        if any(not isinstance(tag, str) or not tag.strip() for tag in memory_tags):
            raise BrainRunError("memory_tags must contain non-empty strings")
        if evidence is not None:
            if not isinstance(evidence, Mapping):
                raise BrainRunError("evidence must be a mapping or None")
            BrainLearningLedger._assert_safe(evidence)
        store = memory or self.memory
        if store is None:
            raise BrainRunError("episodic memory is required for a learning cycle")
        if not isinstance(store, BrainEpisodicMemory):
            raise BrainRunError("memory must be a BrainEpisodicMemory")
        if mission_options is not None and not isinstance(mission_options, Mapping):
            raise BrainRunError("mission_options must be a mapping or None")
        options = {} if mission_options is None else dict(mission_options)
        if provider_health is not None:
            overrides = options.get("selection_overrides", {})
            if not isinstance(overrides, Mapping):
                raise BrainRunError("mission_options.selection_overrides must be a mapping")
            overrides = dict(overrides)
            prior_health = overrides.get("provider_health", {})
            if not isinstance(prior_health, Mapping):
                raise BrainRunError("mission_options.provider_health must be a mapping")
            merged_health = dict(prior_health)
            for provider, snapshot in provider_health.items():
                if not isinstance(provider, str) or not isinstance(snapshot, Mapping):
                    raise BrainRunError("provider_health must map provider names to objects")
                merged_health[provider] = dict(snapshot)
            overrides["provider_health"] = merged_health
            options["selection_overrides"] = overrides
        allowed_options = {
            "context",
            "contextual_observations",
            "required_capabilities",
            "input_tokens",
            "requested_output_tokens",
            "max_cost_per_million_tokens",
            "max_latency_ms",
            "min_quality",
            "selection_overrides",
            "approve_provider_call",
            "approve_mission_dispatch",
            "run_id",
            "max_output_tokens",
            "temperature",
            "response_schema",
            "idempotency_key",
            "claim_requests",
            "evaluator_review",
            "workflow_binding",
            "route_review",
            "operations_gate_acceptance",
            "route_request",
            "enforce_route_tools",
            "require_resolved_route",
            "provider_tools",
            "tool_choice",
            "max_provider_failovers",
        }
        unknown = sorted(set(options).difference(allowed_options))
        if unknown:
            raise BrainRunError("mission_options contains unsupported fields: " + ", ".join(unknown))
        context = options.get("context")
        if context is not None and not isinstance(context, Mapping):
            raise BrainRunError("mission_options.context must be a mapping")
        if memory_query is None and isinstance(context, Mapping):
            derived_query = {
                field: context[field]
                for field in ("domain", "capability", "risk_class")
                if isinstance(context.get(field), str) and context[field].strip()
            }
            resolved_query: MemoryQuery | Mapping[str, Any] | None = derived_query
        else:
            resolved_query = memory_query
        try:
            recalled = tuple(store.retrieve(resolved_query, limit=memory_limit))
        except BrainMemoryError as error:
            raise BrainRunError("episodic memory retrieval failed") from error
        base_prompt = self._append_memory_prompt(prompt, recalled)
        current_prompt = base_prompt
        current_bandit_state: Mapping[str, Any] = dict(bandit_state)
        attempts: list[BrainMissionResult] = []
        evaluations: list[dict[str, Any]] = []
        memory_receipts: list[dict[str, Any]] = []
        final_status = "completed"
        replan_count = 0

        for attempt in range(max_replans + 1):
            result = self.run_adaptive_mission(
                task=task,
                model_candidates=model_candidates,
                prompt=current_prompt,
                plan=plan,
                credentials=credentials,
                mission_policy=mission_policy,
                **options,
            )
            attempts.append(result)
            decision, report = evaluator.evaluate_and_record_with_decision(
                self,
                result,
                bandit_state=current_bandit_state,
                evidence=evidence,
                ledger=ledger,
            )
            next_state = report.get("next_state")
            if isinstance(next_state, Mapping):
                current_bandit_state = dict(next_state)
            BrainLearningLedger._assert_safe(report)
            episode_id = f"{result.brain_run.run_id}-attempt-{attempt}"
            if len(episode_id.encode("utf-8")) > 256:
                episode_id = "episode-" + hashlib.sha256(episode_id.encode("utf-8")).hexdigest()
            episode_receipt = self.remember_result(
                result,
                task=task,
                episode_id=episode_id,
                context=context,
                tags=[*memory_tags, f"attempt:{attempt}"],
                lesson=decision.replan_instruction if decision.replan_requested else None,
                provenance={
                    "evaluator_id": decision.evaluator_id,
                    "evaluator_version": decision.evaluator_version,
                    "replan_requested": decision.replan_requested,
                },
                memory=store,
            )
            try:
                evaluation_receipt = store.record_evaluation(
                    episode_id,
                    {
                        **decision.to_dict(),
                        "decision_digest": _json_digest(decision.to_dict()),
                    },
                ).to_dict()
            except BrainMemoryError as error:
                raise BrainRunError("episodic evaluation record failed") from error
            memory_receipts.extend((episode_receipt, evaluation_receipt))
            evaluations.append(
                {
                    "decision": decision.to_dict(),
                    "recording": {
                        "status": report.get("status"),
                        "next_state": report.get("next_state"),
                        "learning_evidence": report.get("learning_evidence"),
                    },
                }
            )
            if not decision.failed or not decision.replan_requested:
                final_status = "completed" if decision.passed else "completed_without_replan"
                break
            if result.status == "mission_dispatched" or result.execution is not None:
                final_status = "replan_blocked_after_dispatch"
                break
            if attempt >= max_replans:
                final_status = "replan_limit_reached"
                break
            replan_count += 1
            current_prompt = self._append_replan_prompt(
                base_prompt,
                attempt=attempt + 1,
                previous_result=result,
                decision=decision,
            )
        else:
            final_status = "replan_limit_reached"

        return BrainLearningCycleResult(
            status=final_status,
            final_result=attempts[-1],
            attempts=tuple(attempts),
            evaluations=tuple(evaluations),
            memory_receipts=tuple(memory_receipts),
            recalled_memory=recalled,
            replan_count=replan_count,
        )

    def run_resumable_learning_job(
        self,
        store: "BrainJobStore",
        *,
        job_id: str,
        worker_id: str,
        resolver: Callable[[Mapping[str, Any]], Mapping[str, Any]],
        evaluator: "BrainOutcomeEvaluator",
        bandit_state: Mapping[str, Any],
        provider_health: Mapping[str, Any] | None = None,
        lease_seconds: float = 60.0,
        ledger: BrainLearningLedger | None = None,
        memory: BrainEpisodicMemory | None = None,
        approval_router: Any | None = None,
        approval_scope: str | None = None,
        required_approval_role: str = "operator",
    ) -> BrainJobRunResult:
        """Claim and execute one restart-safe learning job through a caller resolver.

        The persisted job never contains the task, prompt, plan, provider response, evaluator
        evidence, or credential handle. ``resolver`` receives only the public job metadata and
        rehydrates those values in-process (typically by resolving a secret-manager reference and
        collecting a fresh BYOK handle). Any exception during the cycle is conservatively marked
        as reconciliation-required because the process cannot prove whether a side effect began.
        A mission that reaches an approval boundary is durably parked in ``waiting_approval``;
        it is never reported as completed merely because its proposal was generated.
        """

        from .jobs import BrainJobError, BrainJobStore
        from .control_plane import BrainApprovalRouter

        if not isinstance(store, BrainJobStore):
            raise BrainRunError("store must be a BrainJobStore")
        if not callable(resolver):
            raise BrainRunError("resolver must be callable")
        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("evaluator must be a BrainOutcomeEvaluator")
        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("bandit_state must be a mapping")
        if provider_health is not None:
            if not isinstance(provider_health, Mapping):
                raise BrainRunError("provider_health must be a mapping or None")
            BrainLearningLedger._assert_safe(provider_health)
        if not isinstance(lease_seconds, (int, float)) or isinstance(lease_seconds, bool) or not 1 <= lease_seconds <= 86_400:
            raise BrainRunError("lease_seconds must be within [1, 86400]")
        if approval_router is None:
            approval_router = BrainApprovalRouter(store)
        elif not isinstance(approval_router, BrainApprovalRouter):
            raise BrainRunError("approval_router must be a BrainApprovalRouter or None")
        if approval_scope is not None and (
            not isinstance(approval_scope, str)
            or not approval_scope.strip()
            or len(approval_scope.encode("utf-8")) > 512
        ):
            raise BrainRunError("approval_scope must be a bounded non-empty string or None")
        if (
            not isinstance(required_approval_role, str)
            or not required_approval_role.strip()
            or len(required_approval_role.encode("utf-8")) > 128
        ):
            raise BrainRunError("required_approval_role must be a bounded non-empty string")
        try:
            job = store.claim(job_id, worker_id, lease_seconds=lease_seconds)
        except BrainJobError as error:
            raise BrainRunError("brain job claim failed") from error
        if job.terminal:
            return BrainJobRunResult(
                status="already_terminal",
                job=job.to_dict(),
                cycle=None,
                error_class=None,
            )
        execution_started = False
        try:
            approval_released = job.checkpoint.get("phase") == "approval_released"
            job = store.checkpoint(
                job.job_id,
                worker_id,
                phase="resolving_spec",
                checkpoint={"spec_digest": job.spec_digest, "attempt": job.attempts},
                side_effect_boundary="not_started",
            )
            resolved = resolver(job.to_dict())
            if not isinstance(resolved, Mapping):
                raise BrainRunError("job resolver must return a mapping")
            allowed = {
                "task",
                "model_candidates",
                "prompt",
                "plan",
                "credentials",
                "mission_policy",
                "memory_query",
                "memory_limit",
                "memory_tags",
                "evidence",
                "max_replans",
                "mission_options",
            }
            unknown = sorted(set(resolved).difference(allowed))
            if unknown:
                raise BrainRunError("job resolver returned unsupported fields: " + ", ".join(unknown))
            required = {"task", "model_candidates", "prompt", "plan", "credentials", "mission_policy"}
            missing = sorted(required.difference(resolved))
            if missing:
                raise BrainRunError("job resolver omitted required fields: " + ", ".join(missing))
            store.checkpoint(
                job.job_id,
                worker_id,
                phase="learning_cycle_started",
                checkpoint={"spec_digest": job.spec_digest, "attempt": job.attempts},
                side_effect_boundary="not_started",
            )
            execution_started = True
            resolved_for_cycle = dict(resolved)
            if approval_released:
                options = resolved_for_cycle.get("mission_options", {})
                if not isinstance(options, Mapping):
                    raise BrainRunError("approved job mission_options must be a mapping")
                options = dict(options)
                # The durable approval router is the authorization boundary for this rehydrated
                # dispatch. The resolver still owns every private prompt/tool argument, but it
                # cannot accidentally discard the operator's decision by returning False here.
                options["approve_mission_dispatch"] = True
                resolved_for_cycle["mission_options"] = options
            if provider_health is not None:
                options = resolved_for_cycle.get("mission_options", {})
                if not isinstance(options, Mapping):
                    raise BrainRunError("job mission_options must be a mapping")
                options = dict(options)
                overrides = options.get("selection_overrides", {})
                if not isinstance(overrides, Mapping):
                    raise BrainRunError("job mission_options.selection_overrides must be a mapping")
                overrides = dict(overrides)
                prior_health = overrides.get("provider_health", {})
                if not isinstance(prior_health, Mapping):
                    raise BrainRunError("job mission_options.provider_health must be a mapping")
                merged_health = dict(prior_health)
                for provider, snapshot in provider_health.items():
                    if not isinstance(provider, str) or not isinstance(snapshot, Mapping):
                        raise BrainRunError("provider_health must map provider names to objects")
                    merged_health[provider] = dict(snapshot)
                overrides["provider_health"] = merged_health
                options["selection_overrides"] = overrides
                resolved_for_cycle["mission_options"] = options
            cycle = self.run_adaptive_mission_learning_cycle(
                **resolved_for_cycle,
                evaluator=evaluator,
                bandit_state=bandit_state,
                ledger=ledger,
                memory=memory or self.memory,
            )
            final_result = cycle.final_result
            requires_approval = getattr(final_result, "status", None) in {
                "mission_approval_required",
                "approval_required",
            }
            if requires_approval:
                request_digest = final_result.brain_run.outcome_digest
                effective_scope = approval_scope or (
                    f"{job.domain}:{job.capability}:{job.risk_class}:mission_dispatch"
                )
                approval_router.request(
                    job.job_id,
                    worker_id,
                    approval_scope=effective_scope,
                    request_digest=request_digest,
                    required_role=required_approval_role,
                )
                waiting = store.get(job.job_id)
                if waiting is None:
                    raise BrainRunError("approval-waiting job disappeared from the durable store")
                return BrainJobRunResult(
                    status="waiting_approval",
                    job=waiting.to_dict(),
                    cycle=cycle,
                )
            boundary = "dispatched" if (
                final_result.status == "mission_dispatched"
                or final_result.execution is not None
            ) else "preflight"
            store.checkpoint(
                job.job_id,
                worker_id,
                phase="learning_cycle_completed",
                checkpoint={
                    "cycle_status": cycle.status,
                    "attempt_count": len(cycle.attempts),
                    "replan_count": cycle.replan_count,
                    "final_outcome_digest": cycle.final_result.brain_run.outcome_digest,
                },
                side_effect_boundary=boundary,
            )
            completed = store.complete(
                job.job_id,
                worker_id,
                result_metadata={
                    "cycle_status": cycle.status,
                    "attempt_count": len(cycle.attempts),
                    "replan_count": cycle.replan_count,
                    "final_outcome_digest": cycle.final_result.brain_run.outcome_digest,
                },
            )
            return BrainJobRunResult(status=completed.state, job=completed.to_dict(), cycle=cycle)
        except Exception as error:
            error_class = type(error).__name__
            try:
                boundary = "unknown" if execution_started else "not_started"
                store.checkpoint(
                    job.job_id,
                    worker_id,
                    phase="execution_error",
                    checkpoint={"error_class": error_class},
                    side_effect_boundary=boundary,
                )
                failed = store.fail(
                    job.job_id,
                    worker_id,
                    reason=(
                        "execution failed before the cycle started"
                        if not execution_started
                        else "execution outcome is uncertain; reconciliation required"
                    ),
                    retryable=False,
                )
            except (BrainJobError, BrainRunError) as persistence_error:
                raise BrainRunError("brain job failure could not be durably recorded") from persistence_error
            return BrainJobRunResult(
                status=failed.state,
                job=failed.to_dict(),
                cycle=None,
                error_class=error_class,
            )

    def run_resumable_workflow_job(
        self,
        store: "BrainJobStore",
        *,
        job_id: str,
        worker_id: str,
        resolver: Callable[[Mapping[str, Any]], Mapping[str, Any]],
        evaluator: "BrainOutcomeEvaluator | None" = None,
        bandit_state: Mapping[str, Any],
        provider_health: Mapping[str, Any] | None = None,
        lease_seconds: float = 60.0,
        ledger: BrainLearningLedger | None = None,
        memory: BrainEpisodicMemory | None = None,
        approval_router: Any | None = None,
        approval_scope: str | None = None,
        required_approval_role: str = "operator",
        checkpoint_sink: Callable[[str, Any], Any] | None = None,
    ) -> BrainJobRunResult:
        """Execute exactly one bounded workflow continuation under a durable job lease.

        The resolver is the BYOK/process-restart boundary. It receives only the public job
        record and must rehydrate a prepared ``AutonomousTaskBlueprint``, model candidates, and
        live credential handles in memory. The job journal stores workflow identifiers, digests,
        completed stage ids, value-only bandit state, and either a bounded inline checkpoint or
        a reference posture for a caller-owned checkpoint sink. It never stores the raw task,
        prompt, provider response, credential handle, or evaluator evidence.

        One worker invocation runs at most one provider-backed stage. A successful non-terminal
        stage is checkpointed and cooperatively requeued, which makes process restart and worker
        hand-off ordinary control-plane events rather than implicit replay. Provider approval is
        parked in ``waiting_approval`` and the approval release is enforced on the rehydrated
        options before the next stage can run.
        """

        from .autonomy import AutonomousTaskBlueprint, AutonomousWorkflowCheckpoint
        from .control_plane import BrainApprovalRouter
        from .jobs import BrainJobError, BrainJobStore, MAX_JOB_CHECKPOINT_BYTES

        if not isinstance(store, BrainJobStore):
            raise BrainRunError("store must be a BrainJobStore")
        if not callable(resolver):
            raise BrainRunError("resolver must be callable")
        if evaluator is not None and not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("evaluator must be a BrainOutcomeEvaluator or None")
        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        if provider_health is not None:
            if not isinstance(provider_health, Mapping):
                raise BrainRunError("provider_health must be a mapping or None")
            BrainLearningLedger._assert_safe(provider_health)
        if not isinstance(lease_seconds, (int, float)) or isinstance(lease_seconds, bool) or not 1 <= lease_seconds <= 86_400:
            raise BrainRunError("lease_seconds must be within [1, 86400]")
        if approval_router is None:
            approval_router = BrainApprovalRouter(store)
        elif not isinstance(approval_router, BrainApprovalRouter):
            raise BrainRunError("approval_router must be a BrainApprovalRouter or None")
        if approval_scope is not None and (
            not isinstance(approval_scope, str)
            or not approval_scope.strip()
            or len(approval_scope.encode("utf-8")) > 512
        ):
            raise BrainRunError("approval_scope must be a bounded non-empty string or None")
        if (
            not isinstance(required_approval_role, str)
            or not required_approval_role.strip()
            or len(required_approval_role.encode("utf-8")) > 128
        ):
            raise BrainRunError("required_approval_role must be a bounded non-empty string")
        if checkpoint_sink is not None and not callable(checkpoint_sink):
            raise BrainRunError("checkpoint_sink must be callable or None")

        try:
            job = store.claim(job_id, worker_id, lease_seconds=lease_seconds)
        except BrainJobError as error:
            raise BrainRunError("brain workflow job claim failed") from error
        if job.terminal:
            return BrainJobRunResult(status="already_terminal", job=job.to_dict(), cycle=None, workflow=None)

        execution_started = False
        workflow_result: Any | None = None
        current_boundary = job.side_effect_boundary

        def _checkpoint_digest(value: Mapping[str, Any]) -> str:
            return _json_digest(value)

        def _persist_workflow_state(
            current_job: Any,
            result: Any,
            *,
            phase: str,
            side_effect_boundary: str = "preflight",
        ) -> Any:
            workflow_run = result.workflow
            checkpoint = workflow_run.checkpoint
            checkpoint_dict = checkpoint.to_dict()
            checkpoint_digest = checkpoint.checkpoint_digest
            state = result.bandit_state
            if not isinstance(state, Mapping):
                raise BrainRunError("workflow learning returned a non-mapping bandit state")
            BrainLearningLedger._assert_safe(state)
            metadata: dict[str, Any] = {
                "job_kind": "autonomous_workflow",
                "workflow_id": workflow_run.blueprint.workflow.workflow_id,
                "workflow_digest": workflow_run.blueprint.workflow.workflow_digest,
                "workflow_run_id": workflow_run.run_id,
                "workflow_checkpoint_digest": checkpoint_digest,
                "completed_stage_ids": list(checkpoint.completed_stage_ids),
                "next_stage_ids": list(workflow_run.next_stage_ids),
                "workflow_status": result.status,
                "bandit_state": dict(state),
                "stage_evaluation_count": len(result.evaluations),
            }
            inline_candidate = {**metadata, "checkpoint_storage": "inline", "workflow_checkpoint": checkpoint_dict}
            encoded_size = len(
                json.dumps(inline_candidate, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
            )
            # Approval and cooperative-release transitions append a small amount of metadata to
            # the same job record. Keep headroom so a valid inline checkpoint cannot become
            # unpersistable merely because an operator approves it or a worker releases it.
            inline_limit = MAX_JOB_CHECKPOINT_BYTES - 8_192
            if encoded_size <= inline_limit:
                persisted = inline_candidate
            else:
                if checkpoint_sink is None:
                    raise BrainRunError(
                        "workflow checkpoint exceeds the job journal bound; configure checkpoint_sink for caller-owned persistence"
                    )
                checkpoint_sink(current_job.job_id, checkpoint)
                persisted = {**metadata, "checkpoint_storage": "caller_owned"}
            return store.checkpoint(
                current_job.job_id,
                worker_id,
                phase=phase,
                checkpoint=persisted,
                side_effect_boundary=side_effect_boundary,
            )

        try:
            previous_checkpoint = job.checkpoint
            previous_kind = previous_checkpoint.get("job_kind")
            if previous_kind is not None and previous_kind != "autonomous_workflow":
                raise BrainRunError("job checkpoint belongs to a different execution kind")
            approval_released = previous_checkpoint.get("phase") == "approval_released"
            resolving = store.checkpoint(
                job.job_id,
                worker_id,
                phase="resolving_workflow",
                checkpoint={
                    **dict(previous_checkpoint),
                    "job_kind": "autonomous_workflow",
                    "spec_digest": job.spec_digest,
                    "attempt": job.attempts,
                },
                side_effect_boundary=current_boundary,
            )
            resolved = resolver(resolving.to_dict())
            if not isinstance(resolved, Mapping):
                raise BrainRunError("workflow job resolver must return a mapping")
            allowed = {"blueprint", "model_candidates", "credentials", "checkpoint", "workflow_options"}
            unknown = sorted(set(resolved).difference(allowed))
            if unknown:
                raise BrainRunError("workflow job resolver returned unsupported fields: " + ", ".join(unknown))
            required = {"blueprint", "model_candidates", "credentials"}
            missing = sorted(required.difference(resolved))
            if missing:
                raise BrainRunError("workflow job resolver omitted required fields: " + ", ".join(missing))
            blueprint = resolved["blueprint"]
            if not isinstance(blueprint, AutonomousTaskBlueprint):
                raise BrainRunError("workflow job blueprint must be an AutonomousTaskBlueprint")
            options = resolved.get("workflow_options", {})
            if not isinstance(options, Mapping):
                raise BrainRunError("workflow_options must be a mapping")
            options = dict(options)
            allowed_options = {
                "retry_blocked", "stage_execution_mode", "memory_query", "memory_limit",
                "contextual_observations", "input_tokens", "requested_output_tokens", "max_cost_per_million_tokens",
                "max_latency_ms", "min_quality", "selection_overrides", "approve_provider_call",
                "approve_mission_dispatch", "run_id", "max_output_tokens", "temperature", "idempotency_key",
                "mission_policy", "mission_options", "route_request", "auto_route", "enforce_route_tools",
                "require_resolved_route", "provider_tools", "tool_choice", "max_provider_failovers",
                "tool_loop_options", "stage_evidence", "memory_tags", "resume_after_replan", "max_stage_calls",
            }
            unknown_options = sorted(set(options).difference(allowed_options))
            if unknown_options:
                raise BrainRunError("workflow_options contains unsupported fields: " + ", ".join(unknown_options))
            resume_after_replan = bool(options.pop("resume_after_replan", False))
            if previous_checkpoint.get("workflow_status") == "learning_replan_requested" and not resume_after_replan:
                raise BrainRunError(
                    "workflow learning requested a replan; resolver must explicitly set resume_after_replan"
                )
            checkpoint_value = resolved.get("checkpoint")
            if checkpoint_value is None:
                checkpoint_value = previous_checkpoint.get("workflow_checkpoint")
            if checkpoint_value is None and previous_checkpoint.get("checkpoint_storage") == "caller_owned":
                raise BrainRunError("resolver did not rehydrate the caller-owned workflow checkpoint")
            if checkpoint_value is not None and not isinstance(checkpoint_value, AutonomousWorkflowCheckpoint):
                if not isinstance(checkpoint_value, Mapping):
                    raise BrainRunError("workflow resolver checkpoint must be a checkpoint or mapping")
                checkpoint_value = AutonomousWorkflowCheckpoint.from_dict(checkpoint_value)
            expected_checkpoint_digest = previous_checkpoint.get("workflow_checkpoint_digest")
            if checkpoint_value is not None and expected_checkpoint_digest is not None:
                if checkpoint_value.checkpoint_digest != expected_checkpoint_digest:
                    raise BrainRunError("rehydrated workflow checkpoint digest does not match the job journal")
            if checkpoint_value is not None:
                options["checkpoint"] = checkpoint_value
            else:
                options.setdefault("run_id", f"job-{job.job_id}")
            if options.get("max_stage_calls") not in (None, 1):
                raise BrainRunError("durable workflow jobs execute at most one stage per lease")
            options["max_stage_calls"] = 1
            if approval_released:
                options["approve_provider_call"] = True
                if str(previous_checkpoint.get("approval_scope", "")).endswith(":mission_dispatch"):
                    options["approve_mission_dispatch"] = True
            if provider_health is not None:
                overrides = options.get("selection_overrides", {})
                if not isinstance(overrides, Mapping):
                    raise BrainRunError("workflow_options.selection_overrides must be a mapping")
                merged_overrides = dict(overrides)
                prior_health = merged_overrides.get("provider_health", {})
                if not isinstance(prior_health, Mapping):
                    raise BrainRunError("workflow_options.provider_health must be a mapping")
                merged_health = dict(prior_health)
                for provider, snapshot in provider_health.items():
                    if not isinstance(provider, str) or not isinstance(snapshot, Mapping):
                        raise BrainRunError("provider_health must map provider names to objects")
                    merged_health[provider] = dict(snapshot)
                merged_overrides["provider_health"] = merged_health
                options["selection_overrides"] = merged_overrides
            store.checkpoint(
                job.job_id,
                worker_id,
                phase="workflow_stage_started",
                checkpoint={
                    **dict(resolving.checkpoint),
                    "workflow_id": blueprint.workflow.workflow_id,
                    "workflow_digest": blueprint.workflow.workflow_digest,
                    "workflow_run_id": options.get("run_id") or (
                        checkpoint_value.run_id if checkpoint_value is not None else f"job-{job.job_id}"
                    ),
                },
                side_effect_boundary="preflight",
            )
            execution_started = True
            workflow_result = self.run_workflow_learning(
                blueprint=blueprint,
                model_candidates=resolved["model_candidates"],
                credentials=resolved["credentials"],
                bandit_state=previous_checkpoint.get("bandit_state", bandit_state),
                evaluator=evaluator,
                ledger=ledger,
                memory=memory or self.memory,
                **options,
            )
            persisted = _persist_workflow_state(
                job,
                workflow_result,
                phase="workflow_stage_checkpointed",
                side_effect_boundary="preflight",
            )
            workflow_run = workflow_result.workflow
            if workflow_run.status == "approval_required":
                request_digest = _checkpoint_digest(
                    {
                        "workflow_id": workflow_run.blueprint.workflow.workflow_id,
                        "run_id": workflow_run.run_id,
                        "checkpoint_digest": workflow_run.checkpoint.checkpoint_digest,
                        "next_stage_ids": list(workflow_run.next_stage_ids),
                    }
                )
                stage_result = workflow_run.stage_results[-1] if workflow_run.stage_results else None
                raw_status = None if stage_result is None or stage_result.result is None else stage_result.result.status
                scope_suffix = "mission_dispatch" if raw_status == "mission_approval_required" else "provider_call"
                effective_scope = approval_scope or f"{job.domain}:{job.capability}:{job.risk_class}:{scope_suffix}"
                approval_router.request(
                    job.job_id,
                    worker_id,
                    approval_scope=effective_scope,
                    request_digest=request_digest,
                    required_role=required_approval_role,
                )
                waiting = store.get(job.job_id)
                if waiting is None:
                    raise BrainRunError("workflow approval-waiting job disappeared from the durable store")
                return BrainJobRunResult(
                    status="waiting_approval",
                    job=waiting.to_dict(),
                    cycle=None,
                    workflow=workflow_result,
                )
            if workflow_result.status == "completed" and workflow_run.status == "completed":
                completed = store.complete(
                    job.job_id,
                    worker_id,
                    result_metadata={
                        "job_kind": "autonomous_workflow",
                        "workflow_id": workflow_run.blueprint.workflow.workflow_id,
                        "workflow_run_id": workflow_run.run_id,
                        "workflow_status": workflow_result.status,
                        "workflow_checkpoint_digest": workflow_run.checkpoint.checkpoint_digest,
                        "completed_stage_ids": list(workflow_run.checkpoint.completed_stage_ids),
                        "stage_evaluation_count": len(workflow_result.evaluations),
                    },
                )
                return BrainJobRunResult(
                    status=completed.state,
                    job=completed.to_dict(),
                    cycle=None,
                    workflow=workflow_result,
                )
            released = store.release(
                job.job_id,
                worker_id,
                reason=(
                    "workflow learning requested explicit replan"
                    if workflow_result.status == "learning_replan_requested"
                    else "workflow stage checkpoint persisted"
                ),
            )
            return BrainJobRunResult(
                status=workflow_result.status if workflow_result.status == "learning_replan_requested" else "queued",
                job=released.to_dict(),
                cycle=None,
                workflow=workflow_result,
            )
        except Exception as error:
            error_class = type(error).__name__
            try:
                boundary = "unknown" if execution_started else current_boundary
                current = store.get(job.job_id)
                if current is not None and current.lease_owner == worker_id and current.state in {"leased", "running"}:
                    store.checkpoint(
                        job.job_id,
                        worker_id,
                        phase="workflow_execution_error",
                        checkpoint={
                            **dict(current.checkpoint),
                            "error_class": error_class,
                        },
                        side_effect_boundary=boundary,
                    )
                    failed = store.fail(
                        job.job_id,
                        worker_id,
                        reason=(
                            "workflow execution failed before provider dispatch"
                            if not execution_started
                            else "workflow execution outcome is uncertain; reconciliation required"
                        ),
                        retryable=False,
                    )
                    return BrainJobRunResult(
                        status=failed.state,
                        job=failed.to_dict(),
                        cycle=None,
                        error_class=error_class,
                        workflow=workflow_result,
                    )
            except (BrainJobError, BrainRunError) as persistence_error:
                raise BrainRunError("workflow job failure could not be durably recorded") from persistence_error
            raise BrainRunError("workflow job execution failed") from error

    def run(
        self,
        *,
        task: str,
        model_selection: Mapping[str, Any],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        approve_provider_call: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 1024,
        temperature: float | None = None,
        require_json: bool = False,
        response_schema: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        context: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
    ) -> BrainRunResult:
        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("task must be a non-empty string")
        if not isinstance(tools, Sequence) or isinstance(tools, (str, bytes)):
            raise BrainRunError("tools must be a sequence")
        if any(not isinstance(tool, ProviderTool) for tool in tools):
            raise BrainRunError("tools must contain ProviderTool values")
        resolved_run_id = run_id or f"brain-{uuid.uuid4().hex}"
        if not isinstance(resolved_run_id, str) or not resolved_run_id.strip() or len(resolved_run_id) > 256:
            raise BrainRunError("run_id must be a bounded non-empty string")
        selection_args = dict(model_selection)
        selection_args["task"] = task
        if context is None:
            if contextual_observations:
                raise BrainRunError("contextual_observations require a context mapping")
            selection = self.workspace.tool("brain_model_select", selection_args)
        else:
            if not isinstance(context, Mapping):
                raise BrainRunError("context must be a mapping")
            BrainLearningLedger._assert_safe(context)
            if not isinstance(contextual_observations, Sequence) or isinstance(
                contextual_observations, (str, bytes)
            ):
                raise BrainRunError("contextual_observations must be a sequence")
            if any(not isinstance(observation, Mapping) for observation in contextual_observations):
                raise BrainRunError("contextual_observations must contain mappings")
            BrainLearningLedger._assert_safe(list(contextual_observations))
            contextual_report = self.workspace.tool(
                "brain_model_select_contextual",
                {
                    "context": dict(context),
                    "base": selection_args,
                    "observations": [dict(observation) for observation in contextual_observations],
                },
            )
            nested_selection = contextual_report.get("selection")
            if not isinstance(nested_selection, Mapping):
                raise BrainRunError("contextual model selection did not produce a selection report")
            context_digest = contextual_report.get("context_digest")
            if not _valid_digest(context_digest):
                raise BrainRunError("contextual model selection returned an invalid context digest")
            selection = dict(nested_selection)
            selection["context_digest"] = context_digest
            selection["contextual_selection_status"] = contextual_report.get("selection_status")
        if isinstance(selection_args.get("provider_health"), Mapping):
            selection["provider_health"] = dict(selection_args["provider_health"])
        selected = selection.get("selected_model")
        if not isinstance(selected, Mapping):
            raise BrainRunError("model selection did not produce an eligible model")
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not provider or not isinstance(model, str) or not model:
            raise BrainRunError("model selection returned malformed provider/model metadata")

        prompt_args = dict(prompt)
        prompt_args["task"] = task
        prompt_report = self.workspace.tool("brain_prompt_assemble", prompt_args)
        messages = prompt_report.get("messages")
        if not isinstance(messages, list) or not messages:
            raise BrainRunError("prompt assembly did not produce messages")

        plan_args = dict(plan)
        plan_args.setdefault("objective", task)
        plan_report = self.workspace.tool("brain_plan", plan_args)
        if not plan_report.get("ok", False):
            return self._result(resolved_run_id, "plan_refused", selection, prompt_report, plan_report, None)
        planned = plan_report.get("plan")
        if not isinstance(planned, Mapping):
            raise BrainRunError("brain plan reported success without a plan")
        if planned.get("requires_approval", False) and not approve_provider_call:
            return self._result(resolved_run_id, "approval_required", selection, prompt_report, plan_report, None)
        if not approve_provider_call and any(
            isinstance(step, Mapping) and step.get("effect") == "provider_call"
            for step in planned.get("steps", [])
        ):
            return self._result(resolved_run_id, "approval_required", selection, prompt_report, plan_report, None)

        handle = credentials.get(provider)
        if self.runtime.provider_requires_credential(provider) and handle is None:
            raise BrainRunError(f"no user credential handle was supplied for provider {provider!r}")
        if handle is not None and handle.provider != provider:
            raise BrainRunError(f"credential handle does not belong to provider {provider!r}")
        provider_messages = tuple(
            {"role": message["role"], "content": message["content"]}
            for message in messages
            if isinstance(message, Mapping)
            and isinstance(message.get("role"), str)
            and isinstance(message.get("content"), str)
        )
        if len(provider_messages) != len(messages):
            raise BrainRunError("prompt assembly returned malformed provider messages")
        request = ProviderRequest(
            model=model,
            messages=provider_messages,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            require_json=require_json,
            response_schema=response_schema,
            idempotency_key=idempotency_key,
            tools=tuple(tools),
            tool_choice=tool_choice,
        )
        response = self.runtime.invoke(provider, request, credential=handle)
        return self._result(resolved_run_id, "completed_provider_call", selection, prompt_report, plan_report, response)

    def run_tool_loop(
        self,
        *,
        task: str,
        model_selection: Mapping[str, Any],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        authorize_and_execute: Callable[[tuple[ProviderToolCall, ...]], Sequence[ProviderToolResult]] | None = None,
        approve_provider_call: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 2048,
        temperature: float | None = None,
        require_json: bool = False,
        response_schema: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        context: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        provider_tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
        max_turns: int = 4,
        max_tool_calls: int = 128,
        stream: bool = False,
        mission_policy: MissionPolicy | Mapping[str, Any] | None = None,
        approve_mission_dispatch: bool = False,
        route_request: Mapping[str, Any] | None = None,
        enforce_route_tools: bool = True,
        require_resolved_route: bool = True,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
        operations_gate_acceptance: Mapping[str, Any] | None = None,
        route_report: Mapping[str, Any] | None = None,
        attempt_state: dict[str, Any] | None = None,
    ) -> BrainToolLoopResult:
        """Run the planned provider call and continue only through caller-approved tool results.

        This method is the high-level bridge for applications that want native function calling
        without converting every turn into a mission. The initial model decision still passes
        through ``brain_plan`` and provider approval. The callback is intentionally typed and
        explicit: it may invoke a caller-owned mission/executor, but the brain and provider
        runtime never do so implicitly.
        """

        if authorize_and_execute is not None and not callable(authorize_and_execute):
            raise BrainRunError("authorize_and_execute must be callable")
        if attempt_state is not None and not isinstance(attempt_state, dict):
            raise BrainRunError("attempt_state must be a mutable mapping")
        if attempt_state is not None:
            attempt_state["tool_authorization_started"] = False
        if not isinstance(provider_tools, Sequence) or isinstance(provider_tools, (str, bytes)):
            raise BrainRunError("provider_tools must be a sequence")
        if any(not isinstance(tool, ProviderTool) for tool in provider_tools):
            raise BrainRunError("provider_tools must contain ProviderTool values")
        if not isinstance(stream, bool):
            raise BrainRunError("stream must be a boolean")
        if not isinstance(enforce_route_tools, bool) or not isinstance(require_resolved_route, bool):
            raise BrainRunError("route enforcement flags must be booleans")
        if route_report is not None:
            if route_request is None:
                raise BrainRunError("route_report requires route_request")
            if not isinstance(route_report, Mapping):
                raise BrainRunError("route_report must be a mapping")
            BrainLearningLedger._assert_safe(route_report)
        prompt_request = dict(prompt)
        route: dict[str, Any] | None = None
        raw_route: dict[str, Any] | None = None
        if route_request is not None:
            if not isinstance(route_request, Mapping):
                raise BrainRunError("route_request must be a mapping")
            BrainLearningLedger._assert_safe(route_request)
            route_arguments = dict(route_request)
            supplied_goal = route_arguments.get("goal")
            if supplied_goal is not None and supplied_goal != task:
                raise BrainRunError("route_request.goal must match the tool-loop task")
            route_arguments["goal"] = task
            route_arguments.setdefault("needs", [{"id": "task", "query": task}])
            route_arguments.setdefault("include_tools", True)
            route_arguments.setdefault("max_tools", 128)
            try:
                encoded_route_request = json.dumps(
                    route_arguments,
                    ensure_ascii=False,
                    sort_keys=True,
                    separators=(",", ":"),
                    allow_nan=False,
                ).encode("utf-8")
            except (TypeError, ValueError) as error:
                raise BrainRunError("route_request must be JSON-safe") from error
            if len(encoded_route_request) > MAX_ROUTE_REQUEST_BYTES:
                raise BrainRunError("route_request exceeds the bounded size")
            route_response = (
                dict(route_report)
                if route_report is not None
                else self.workspace.tool("capability_route", route_arguments)
            )
            if not isinstance(route_response, Mapping):
                raise BrainRunError("capability route returned a non-object")
            if route_response.get("ok") is False or route_response.get("workflow") != "capability_route":
                raise BrainRunError("capability route was refused")
            raw_route = dict(route_response)
            BrainLearningLedger._assert_safe(raw_route)
            unresolved = raw_route.get("unresolved_needs", [])
            if not isinstance(unresolved, list) or any(not isinstance(item, str) for item in unresolved):
                raise BrainRunError("capability route returned malformed unresolved_needs")
            if unresolved and require_resolved_route:
                raise BrainRunError("capability route contains unresolved needs: " + ", ".join(unresolved))
            route_context = _bounded_route_prompt_context(raw_route)
            route = dict(route_context)
            route.update(
                {
                    "ok": True,
                    "workflow": "capability_route",
                    "evidence_digest": raw_route.get("evidence_digest"),
                    "unresolved_needs": list(unresolved),
                    "route_coverage": raw_route.get("route_coverage", {}),
                    "execution": raw_route.get("execution", "not_started"),
                }
            )
            existing_context = prompt_request.get("context", [])
            if not isinstance(existing_context, Sequence) or isinstance(existing_context, (str, bytes)):
                raise BrainRunError("prompt.context must be a sequence when routing is enabled")
            context_chunks = [dict(chunk) for chunk in existing_context if isinstance(chunk, Mapping)]
            if len(context_chunks) != len(existing_context):
                raise BrainRunError("prompt.context must contain mappings")
            if any(chunk.get("id") == "capability-route" for chunk in context_chunks):
                raise BrainRunError("prompt.context already contains the reserved capability-route id")
            context_chunks.append(
                {
                    "id": "capability-route",
                    "role": "developer",
                    "content": json.dumps(route_context, ensure_ascii=False, sort_keys=True, separators=(",", ":")),
                    "required": True,
                    "priority": 1_000,
                }
            )
            prompt_request["context"] = context_chunks
            if not provider_tools and route_context["tool_schemas"] and not route_context["tool_schemas_omitted"]:
                provider_tools = tuple(
                    ProviderTool.from_mcp_schema(schema) for schema in route_context["tool_schemas"]
                )
            if enforce_route_tools:
                if mission_policy is None:
                    raise BrainRunError("enforce_route_tools requires mission_policy")
                policy_for_route = (
                    mission_policy.to_dict() if isinstance(mission_policy, MissionPolicy) else dict(mission_policy)
                )
                allowed_tools = policy_for_route.get("allowed_tools")
                recommended_tools = route.get("recommended_tools")
                if not isinstance(allowed_tools, Sequence) or isinstance(allowed_tools, (str, bytes)):
                    raise BrainRunError("enforce_route_tools requires an explicit mission policy allowed_tools list")
                if not isinstance(recommended_tools, list) or any(not isinstance(tool, str) for tool in recommended_tools):
                    raise BrainRunError("capability route returned malformed recommended_tools")
                narrowed = [tool for tool in allowed_tools if tool in set(recommended_tools)]
                if not narrowed:
                    raise BrainRunError("route has no overlap with the caller mission policy allowed_tools")
                policy_for_route["allowed_tools"] = narrowed
                mission_policy = policy_for_route
        if authorize_and_execute is None:
            if mission_policy is None:
                raise BrainRunError("provide authorize_and_execute or mission_policy for the built-in mission authorizer")
            if not provider_tools:
                raise BrainRunError("the built-in mission authorizer requires provider_tools")
            authorizer = MissionToolAuthorizer(
                self.workspace,
                task=task,
                mission_policy=mission_policy,
                route=raw_route,
                approve_mission_dispatch=approve_mission_dispatch,
                claim_requests=claim_requests,
                evaluator_review=evaluator_review,
                workflow_binding=workflow_binding,
                operations_gate_acceptance=operations_gate_acceptance,
            )
            authorize_and_execute = authorizer
        else:
            authorizer = None
        if attempt_state is not None:
            original_authorizer = authorize_and_execute
            if original_authorizer is None:
                raise BrainRunError("tool authorization callback was not initialized")

            def tracked_authorizer(
                calls: tuple[ProviderToolCall, ...],
            ) -> Sequence[ProviderToolResult]:
                if calls:
                    attempt_state["tool_authorization_started"] = True
                return original_authorizer(calls)

            authorize_and_execute = tracked_authorizer
        first = self.run(
            task=task,
            model_selection=model_selection,
            prompt=prompt_request,
            plan=plan,
            credentials=credentials,
            approve_provider_call=approve_provider_call,
            run_id=run_id,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            require_json=require_json,
            response_schema=response_schema,
            idempotency_key=idempotency_key,
            context=context,
            contextual_observations=contextual_observations,
            tools=provider_tools,
            tool_choice=tool_choice,
        )
        if first.status != "completed_provider_call" or first.response is None:
            return BrainToolLoopResult(brain_run=first, status=first.status, provider_loop=None, route=route)
        selected = first.selection.get("selected_model")
        if not isinstance(selected, Mapping):
            raise BrainRunError("model selection did not produce a continuation model")
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not isinstance(model, str):
            raise BrainRunError("continuation model metadata is malformed")
        prompt_messages = first.prompt.get("messages")
        if not isinstance(prompt_messages, list) or not prompt_messages:
            raise BrainRunError("brain prompt did not retain bounded provider messages")
        provider_messages = tuple(
            {"role": message["role"], "content": message["content"]}
            for message in prompt_messages
            if isinstance(message, Mapping)
            and isinstance(message.get("role"), str)
            and isinstance(message.get("content"), str)
        )
        if len(provider_messages) != len(prompt_messages):
            raise BrainRunError("brain prompt returned malformed continuation messages")
        handle = credentials.get(provider)
        if self.runtime.provider_requires_credential(provider) and handle is None:
            raise BrainRunError(f"no user credential handle was supplied for provider {provider!r}")
        if handle is not None and handle.provider != provider:
            raise BrainRunError(f"credential handle does not belong to provider {provider!r}")
        request = ProviderRequest(
            model=model,
            messages=provider_messages,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            require_json=require_json,
            response_schema=response_schema,
            idempotency_key=idempotency_key,
            tools=tuple(provider_tools),
            tool_choice=tool_choice,
        )
        loop = self.runtime.invoke_tool_loop(
            provider,
            request,
            credential=handle,
            authorize_and_execute=authorize_and_execute,
            max_turns=max_turns,
            max_tool_calls=max_tool_calls,
            stream=stream,
            initial_response=first.response,
        )
        status = {
            "completed": "completed_provider_tool_loop",
            "authorization_required": "tool_authorization_required",
            "turn_limit_reached": "tool_turn_limit_reached",
        }[loop.status]
        receipts = () if authorizer is None else tuple(receipt.to_dict() for receipt in authorizer.receipts)
        return BrainToolLoopResult(
            brain_run=first,
            status=status,
            provider_loop=loop,
            route=route,
            authorization_receipts=receipts,
        )

    def record_evaluator_outcome(
        self,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
        *,
        bandit_state: Mapping[str, Any],
        evaluator_id: str,
        evaluator_version: str,
        reward: float,
        passed: bool,
        arm_id: str | None = None,
        failed: bool = False,
        feedback_digest: str | None = None,
        failure_class: str | None = None,
        evidence_digest: str | None = None,
        ledger: BrainLearningLedger | None = None,
        replay_metadata: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Submit one explicit evaluator judgment for a run, loop, or mission.

        The evaluator remains the only reward authority. For continuation results, the identity
        digest is extended with bounded response metadata without retaining provider text or tool
        wire envelopes in the learning ledger.
        """

        if isinstance(result, BrainRunResult):
            brain_result = result
            outcome_digest = result.outcome_digest
            outcome_request_id = result.response.request_id if result.response is not None else None
        elif isinstance(result, BrainToolLoopResult):
            brain_result = result.brain_run
            final_response = None if result.provider_loop is None else result.provider_loop.final_response
            outcome_digest = _json_digest(
                {
                    "brain_outcome_digest": brain_result.outcome_digest,
                    "status": result.status,
                    "provider_loop_status": None
                    if result.provider_loop is None
                    else result.provider_loop.status,
                    "turns": None if result.provider_loop is None else result.provider_loop.turns,
                    "tool_calls": None
                    if result.provider_loop is None
                    else result.provider_loop.tool_calls,
                    "final_provider": None if final_response is None else final_response.provider,
                    "final_model": None if final_response is None else final_response.model,
                    "final_request_id": None if final_response is None else final_response.request_id,
                }
            )
            outcome_request_id = None if final_response is None else final_response.request_id
        elif isinstance(result, BrainMissionResult):
            brain_result = result.brain_run
            execution = result.execution or {}
            outcome_digest = _json_digest(
                {
                    "brain_outcome_digest": brain_result.outcome_digest,
                    "status": result.status,
                    "mission_status": execution.get("mission_status"),
                    "execution": execution.get("execution"),
                    "result_digest": execution.get("result_digest"),
                }
            )
            outcome_request_id = brain_result.response.request_id if brain_result.response is not None else None
        else:
            raise BrainRunError("result must be a BrainRunResult, BrainToolLoopResult, or BrainMissionResult")

        selected = brain_result.selection.get("selected_model")
        if not isinstance(selected, Mapping):
            raise BrainRunError("cannot record an outcome without selected model metadata")
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not isinstance(model, str):
            raise BrainRunError("selected model metadata is malformed")
        selection_digest = brain_result.selection.get("decision_digest")
        prompt_digest = brain_result.prompt.get("prompt_digest")
        plan_digest = (brain_result.plan.get("plan") or {}).get("plan_digest") if isinstance(brain_result.plan.get("plan"), Mapping) else None
        for name, value in (("selection_digest", selection_digest), ("prompt_digest", prompt_digest), ("plan_digest", plan_digest)):
            if not isinstance(value, str) or len(value) != 64:
                raise BrainRunError(f"{name} is missing or is not a SHA-256 digest")
        report = self.workspace.tool(
            "brain_outcome_record",
            {
                "run": {
                    "run_id": brain_result.run_id,
                    "selection_digest": selection_digest,
                    "prompt_digest": prompt_digest,
                    "plan_digest": plan_digest,
                    "provider": provider,
                    "model": model,
                    "outcome_digest": outcome_digest,
                    "request_id": outcome_request_id,
                },
                "assessment": {
                    "evaluator_id": evaluator_id,
                    "evaluator_version": evaluator_version,
                    "reward": reward,
                    "passed": passed,
                    "failed": failed,
                    "feedback_digest": feedback_digest,
                    "failure_class": failure_class,
                    "evidence_digest": evidence_digest,
                },
                "bandit_state": dict(bandit_state),
                "arm_id": arm_id or f"{provider}/{model}",
            },
        )
        if not isinstance(report, Mapping) or not report.get("ok"):
            raise BrainRunError("brain outcome recording returned a refusal")
        if ledger is not None:
            context_digest = brain_result.selection.get("context_digest")
            ledger.append(
                report,
                context_digest=context_digest if isinstance(context_digest, str) else None,
                replay=replay_metadata,
            )
        return dict(report)

    def run_mission(
        self,
        *,
        task: str,
        model_selection: Mapping[str, Any],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        mission_policy: MissionPolicy | Mapping[str, Any],
        approve_provider_call: bool = False,
        approve_mission_dispatch: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 2048,
        temperature: float | None = None,
        response_schema: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        context: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
        route_review: Mapping[str, Any] | None = None,
        operations_gate_acceptance: Mapping[str, Any] | None = None,
        route_request: Mapping[str, Any] | None = None,
        route_report: Mapping[str, Any] | None = None,
        attempt_state: dict[str, Any] | None = None,
        enforce_route_tools: bool = False,
        require_resolved_route: bool = True,
        provider_tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
    ) -> BrainMissionResult:
        """Run a model decision through the existing bounded mission executor.

        The model supplies only step data. The caller supplies the mission policy and therefore
        the tool allow-list, output budgets, parallelism, and side-effect posture. The server
        receives a preview with ``execute=false`` first; dispatch is a separate request after
        ``approve_mission_dispatch=True``. Claims/evaluator bindings are caller-owned metadata and
        are not accepted from the model response.
        """

        if not isinstance(mission_policy, (MissionPolicy, Mapping)):
            raise BrainRunError("mission_policy must be a MissionPolicy or mapping")
        policy = (
            mission_policy.to_dict()
            if isinstance(mission_policy, MissionPolicy)
            else dict(mission_policy)
        )
        if not isinstance(claim_requests, Sequence) or isinstance(claim_requests, (str, bytes)):
            raise BrainRunError("claim_requests must be a sequence")
        if not isinstance(enforce_route_tools, bool) or not isinstance(require_resolved_route, bool):
            raise BrainRunError("route enforcement flags must be booleans")
        if not isinstance(provider_tools, Sequence) or isinstance(provider_tools, (str, bytes)):
            raise BrainRunError("provider_tools must be a sequence")
        if any(not isinstance(tool, ProviderTool) for tool in provider_tools):
            raise BrainRunError("provider_tools must contain ProviderTool values")
        if attempt_state is not None and not isinstance(attempt_state, dict):
            raise BrainRunError("attempt_state must be a mutable mapping")
        if attempt_state is not None:
            attempt_state["mission_dispatch_started"] = False
        if route_report is not None:
            if route_request is None:
                raise BrainRunError("route_report requires route_request")
            if not isinstance(route_report, Mapping):
                raise BrainRunError("route_report must be a mapping")
            BrainLearningLedger._assert_safe(route_report)

        route: dict[str, Any] | None = None
        prompt_request = dict(prompt)
        if route_request is not None:
            if not isinstance(route_request, Mapping):
                raise BrainRunError("route_request must be a mapping")
            BrainLearningLedger._assert_safe(route_request)
            route_arguments = dict(route_request)
            supplied_goal = route_arguments.get("goal")
            if supplied_goal is not None and supplied_goal != task:
                raise BrainRunError("route_request.goal must match the mission task")
            route_arguments["goal"] = task
            route_arguments.setdefault(
                "needs",
                [{"id": "task", "query": task}],
            )
            route_arguments.setdefault("include_tools", True)
            route_arguments.setdefault("max_tools", 128)
            try:
                encoded_route_request = json.dumps(
                    route_arguments,
                    ensure_ascii=False,
                    sort_keys=True,
                    separators=(",", ":"),
                    allow_nan=False,
                ).encode("utf-8")
            except (TypeError, ValueError) as error:
                raise BrainRunError("route_request must be JSON-safe") from error
            if len(encoded_route_request) > MAX_ROUTE_REQUEST_BYTES:
                raise BrainRunError("route_request exceeds the bounded size")
            route_response = (
                dict(route_report)
                if route_report is not None
                else self.workspace.tool("capability_route", route_arguments)
            )
            if not isinstance(route_response, Mapping):
                raise BrainRunError("capability route returned a non-object")
            if route_response.get("ok") is False or route_response.get("workflow") != "capability_route":
                raise BrainRunError("capability route was refused")
            raw_route = dict(route_response)
            BrainLearningLedger._assert_safe(raw_route)
            unresolved = raw_route.get("unresolved_needs", [])
            if not isinstance(unresolved, list) or any(not isinstance(item, str) for item in unresolved):
                raise BrainRunError("capability route returned malformed unresolved_needs")
            if unresolved and require_resolved_route:
                raise BrainRunError(
                    "capability route contains unresolved needs: " + ", ".join(unresolved)
                )
            route_context = _bounded_route_prompt_context(raw_route)
            route = dict(route_context)
            route.update(
                {
                    "ok": True,
                    "workflow": "capability_route",
                    "evidence_digest": raw_route.get("evidence_digest"),
                    "unresolved_needs": list(unresolved),
                    "route_coverage": raw_route.get("route_coverage", {}),
                    "execution": raw_route.get("execution", "not_started"),
                }
            )
            existing_context = prompt_request.get("context", [])
            if not isinstance(existing_context, Sequence) or isinstance(existing_context, (str, bytes)):
                raise BrainRunError("prompt.context must be a sequence when routing is enabled")
            context_chunks = [dict(chunk) for chunk in existing_context if isinstance(chunk, Mapping)]
            if len(context_chunks) != len(existing_context):
                raise BrainRunError("prompt.context must contain mappings")
            route_chunk_id = "capability-route"
            if any(chunk.get("id") == route_chunk_id for chunk in context_chunks):
                raise BrainRunError("prompt.context already contains the reserved capability-route id")
            context_chunks.append(
                {
                    "id": route_chunk_id,
                    "role": "developer",
                    "content": json.dumps(
                        route_context,
                        ensure_ascii=False,
                        sort_keys=True,
                        separators=(",", ":"),
                    ),
                    "required": True,
                    "priority": 1_000,
                }
            )
            prompt_request["context"] = context_chunks

            if not provider_tools and route_context["tool_schemas"] and not route_context["tool_schemas_omitted"]:
                provider_tools = tuple(
                    ProviderTool.from_mcp_schema(schema)
                    for schema in route_context["tool_schemas"]
                )

            if enforce_route_tools:
                recommended_tools = route.get("recommended_tools")
                if not isinstance(recommended_tools, list) or any(
                    not isinstance(tool, str) for tool in recommended_tools
                ):
                    raise BrainRunError("capability route returned malformed recommended_tools")
                recommended_set = set(recommended_tools)
                allowed_tools = policy.get("allowed_tools")
                if not isinstance(allowed_tools, Sequence) or isinstance(allowed_tools, (str, bytes)):
                    raise BrainRunError(
                        "enforce_route_tools requires an explicit mission policy allowed_tools list"
                    )
                narrowed = [tool for tool in allowed_tools if tool in recommended_set]
                if not narrowed:
                    raise BrainRunError(
                        "route has no overlap with the caller mission policy allowed_tools"
                    )
                policy["allowed_tools"] = narrowed
        policy["execute"] = False
        brain_run = self.run(
            task=task,
            model_selection=model_selection,
            prompt=prompt_request,
            plan=plan,
            credentials=credentials,
            approve_provider_call=approve_provider_call,
            run_id=run_id,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            require_json=True,
            response_schema=response_schema or DEFAULT_MISSION_RESPONSE_SCHEMA,
            idempotency_key=idempotency_key,
            context=context,
            contextual_observations=contextual_observations,
            tools=provider_tools,
            tool_choice=tool_choice,
        )
        if brain_run.status != "completed_provider_call" or brain_run.response is None:
            return BrainMissionResult(
                brain_run=brain_run,
                status="brain_run_not_completed",
                mission=None,
                preflight=None,
                execution=None,
                route=route,
            )
        if brain_run.response.tool_calls:
            raw_steps = []
            for index, call in enumerate(brain_run.response.tool_calls):
                domain = "cross_domain"
                if route is not None:
                    for need in route.get("needs", []):
                        if not isinstance(need, Mapping):
                            continue
                        candidate_tools = need.get("candidate_tools", [])
                        if call.name in candidate_tools:
                            domains = need.get("candidate_domains", [])
                            if isinstance(domains, list) and domains and isinstance(domains[0], str):
                                domain = domains[0]
                            break
                raw_steps.append(
                    {
                        "id": f"provider-tool-{index}",
                        "domain": domain,
                        "capability": call.name,
                        "objective": f"Execute the caller-authorized provider tool intent {call.name}",
                        "tool": call.name,
                        "arguments": dict(call.arguments),
                        "required": True,
                        "depends_on": [],
                        "bindings": [],
                    }
                )
        else:
            structured = brain_run.response.structured
            if not isinstance(structured, Mapping):
                raise BrainRunError("structured brain response did not contain a JSON object")
            proposed = structured.get("mission")
            if not isinstance(proposed, Mapping):
                raise BrainRunError("structured brain response did not contain a mission object")
            raw_steps = proposed.get("steps")
            if not isinstance(raw_steps, list) or not raw_steps:
                raise BrainRunError("model mission must contain a non-empty steps array")

        mission_id = f"{brain_run.run_id}-mission"
        preview_request = MissionRequest(
            mission_id=mission_id,
            goal=task,
            steps=raw_steps,
            policy=policy,
            claim_requests=claim_requests,
            evaluator_review=evaluator_review,
            workflow_binding=workflow_binding,
            route_review=route_review,
            operations_gate_acceptance=operations_gate_acceptance,
        )
        preview_arguments = preview_request.to_mcp_arguments()
        preflight = self.workspace.tool("agent_mission", preview_arguments)
        if not isinstance(preflight, Mapping):
            raise BrainRunError("agent mission preflight returned a non-object")
        if preflight.get("workflow") not in (None, "agent_mission"):
            raise BrainRunError("agent mission preflight returned the wrong workflow")
        mission = dict(preview_arguments)
        if not approve_mission_dispatch:
            return BrainMissionResult(
                brain_run=brain_run,
                status="mission_approval_required",
                mission=mission,
                preflight=dict(preflight),
                execution=None,
                route=route,
            )

        execute_policy = dict(policy)
        execute_policy["execute"] = True
        execute_request = MissionRequest(
            mission_id=mission_id,
            goal=task,
            steps=raw_steps,
            policy=execute_policy,
            claim_requests=claim_requests,
            evaluator_review=evaluator_review,
            workflow_binding=workflow_binding,
            route_review=route_review,
            operations_gate_acceptance=operations_gate_acceptance,
        )
        if attempt_state is not None:
            attempt_state["mission_dispatch_started"] = True
        execution = self.workspace.tool("agent_mission", execute_request.to_mcp_arguments())
        if not isinstance(execution, Mapping):
            raise BrainRunError("agent mission execution returned a non-object")
        return BrainMissionResult(
            brain_run=brain_run,
            status="mission_dispatched",
            mission=mission,
            preflight=dict(preflight),
            execution=dict(execution),
            route=route,
        )

    @staticmethod
    def _result(
        run_id: str,
        status: str,
        selection: Mapping[str, Any],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        response: ProviderResponse | None,
    ) -> BrainRunResult:
        digest_input = {
            "status": status,
            "selection": selection,
            "prompt_digest": prompt.get("prompt_digest"),
            "plan_digest": (plan.get("plan") or {}).get("plan_digest")
            if isinstance(plan.get("plan"), Mapping)
            else None,
            "response": None
            if response is None
            else {
                "provider": response.provider,
                "model": response.model,
                "text": response.text,
                "request_id": response.request_id,
                "usage": dict(response.usage),
            },
        }
        encoded = json.dumps(digest_input, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
        return BrainRunResult(
            run_id=run_id,
            status=status,
            selection=selection,
            prompt=prompt,
            plan=plan,
            response=response,
            outcome_digest=hashlib.sha256(encoded).hexdigest(),
        )


@dataclass(frozen=True, slots=True)
class BrainEvaluatorDecision:
    """A validated, value-only evaluator judgment for one brain outcome.

    The evaluator is intentionally separate from the provider. A provider response can be
    inspected by a caller-owned evaluator, but only this compact decision crosses the learning
    boundary. ``evidence_digest`` binds the decision to the optional caller-supplied evidence
    packet without copying that packet into the learning ledger.
    """

    evaluator_id: str
    evaluator_version: str
    reward: float
    passed: bool
    failed: bool = False
    feedback_digest: str | None = None
    failure_class: str | None = None
    evidence_digest: str | None = None
    replan_requested: bool = False
    replan_instruction: str | None = None

    def __post_init__(self) -> None:
        for field_name, value in (
            ("evaluator_id", self.evaluator_id),
            ("evaluator_version", self.evaluator_version),
        ):
            if (
                not isinstance(value, str)
                or not value.strip()
                or len(value.encode("utf-8")) > MAX_BRAIN_EVALUATOR_ID_BYTES
            ):
                raise BrainRunError(f"{field_name} must be a bounded non-empty string")
        if (
            not isinstance(self.reward, (int, float))
            or isinstance(self.reward, bool)
            or not isinstance(self.passed, bool)
            or not isinstance(self.failed, bool)
        ):
            raise BrainRunError("evaluator decision has malformed reward or status fields")
        try:
            json.dumps(self.reward, allow_nan=False)
        except (TypeError, ValueError) as error:
            raise BrainRunError("evaluator reward must be finite") from error
        if self.passed and self.failed:
            raise BrainRunError("evaluator decision cannot be both passed and failed")
        for field_name, value in (
            ("feedback_digest", self.feedback_digest),
            ("evidence_digest", self.evidence_digest),
        ):
            if value is not None and not _valid_digest(value):
                raise BrainRunError(f"{field_name} must be a lowercase SHA-256 digest")
        if self.failure_class is not None and (
            not isinstance(self.failure_class, str)
            or not self.failure_class.strip()
            or len(self.failure_class.encode("utf-8")) > MAX_BRAIN_EVALUATOR_ID_BYTES
        ):
            raise BrainRunError("failure_class must be a bounded non-empty string")
        if not isinstance(self.replan_requested, bool):
            raise BrainRunError("replan_requested must be boolean")
        if self.replan_instruction is not None and (
            not isinstance(self.replan_instruction, str)
            or not self.replan_instruction.strip()
            or len(self.replan_instruction.encode("utf-8")) > MAX_BRAIN_REPLAN_INSTRUCTION_BYTES
        ):
            raise BrainRunError("replan_instruction must be a bounded non-empty string")
        if self.replan_instruction is not None and any(
            pattern.search(self.replan_instruction) for pattern in _REPLAN_SECRET_PATTERNS
        ):
            raise BrainRunError("replan_instruction resembles secret material")
        if self.replan_requested and self.failed and self.replan_instruction is None and self.failure_class is None:
            raise BrainRunError("a requested replan must include an instruction or failure_class")

    def to_dict(self) -> dict[str, Any]:
        return {
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "reward": self.reward,
            "passed": self.passed,
            "failed": self.failed,
            "feedback_digest": self.feedback_digest,
            "failure_class": self.failure_class,
            "evidence_digest": self.evidence_digest,
            "replan_requested": self.replan_requested,
            "replan_instruction": self.replan_instruction,
        }


def _evaluator_metadata_projection(result: BrainRunResult) -> dict[str, Any]:
    """Project the common run identity without exposing prompt or provider response content."""

    selected = result.selection.get("selected_model")
    selected_model = (
        {"provider": selected.get("provider"), "model": selected.get("model")}
        if isinstance(selected, Mapping)
        else None
    )
    plan = result.plan.get("plan")
    plan_digest = plan.get("plan_digest") if isinstance(plan, Mapping) else None
    projection: dict[str, Any] = {
        "run_id": result.run_id,
        "status": result.status,
        "selected_model": selected_model,
        "selection_digest": result.selection.get("decision_digest"),
        "context_digest": result.selection.get("context_digest"),
        "prompt_digest": result.prompt.get("prompt_digest"),
        "plan_digest": plan_digest,
        "outcome_digest": result.outcome_digest,
        "provider_failover": None
        if result.provider_failover is None
        else {
            "strategy": result.provider_failover.get("strategy"),
            "fallback_count": result.provider_failover.get("fallback_count"),
            "attempt_count": len(result.provider_failover.get("attempts", []))
            if isinstance(result.provider_failover.get("attempts"), list)
            else None,
            "retention": result.provider_failover.get("retention"),
        },
    }
    if result.response is not None:
        projection["response"] = {
            "provider": result.response.provider,
            "model": result.response.model,
            "request_id": result.response.request_id,
            "usage": dict(result.response.usage),
            "structured": result.response.structured is not None,
            "tool_call_count": len(result.response.tool_calls),
        }
    else:
        projection["response"] = None
    return projection


def build_brain_evaluation_input(
    result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
    *,
    evidence: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Build a bounded evaluator input for any brain execution shape.

    Only identities, digests, status/count metadata, route identity, and caller-supplied bounded
    evidence are exposed. Provider text, prompt text, credentials, and opaque tool wire envelopes
    are deliberately absent. The returned value is JSON round-tripped so an evaluator cannot
    mutate the caller's original mappings through shared references.
    """

    if evidence is not None:
        if not isinstance(evidence, Mapping):
            raise BrainRunError("evaluator evidence must be a mapping or None")
        BrainLearningLedger._assert_safe(evidence)
        try:
            encoded_evidence = json.dumps(
                dict(evidence),
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("evaluator evidence must be JSON-safe") from error
        if len(encoded_evidence) > MAX_BRAIN_EVALUATOR_EVIDENCE_BYTES:
            raise BrainRunError("evaluator evidence exceeds the bounded size")
        evidence_copy = json.loads(encoded_evidence.decode("utf-8"))
        evidence_digest = hashlib.sha256(encoded_evidence).hexdigest()
    else:
        evidence_copy = None
        evidence_digest = None

    if isinstance(result, BrainRunResult):
        projection = _evaluator_metadata_projection(result)
        result_kind = "run"
    elif isinstance(result, BrainToolLoopResult):
        projection = _evaluator_metadata_projection(result.brain_run)
        loop = result.provider_loop
        final_response = None if loop is None else loop.final_response
        receipt_statuses: dict[str, int] = {}
        for receipt in result.authorization_receipts:
            if not isinstance(receipt, Mapping):
                continue
            status = receipt.get("status")
            if isinstance(status, str):
                receipt_statuses[status] = receipt_statuses.get(status, 0) + 1
        projection.update(
            {
                "result_kind": "tool_loop",
                "status": result.status,
                "route": None
                if result.route is None
                else {
                    "route_digest": _json_digest(dict(result.route)),
                    "evidence_digest": result.route.get("evidence_digest"),
                    "execution": result.route.get("execution"),
                },
                "tool_loop": None
                if loop is None
                else {
                    "status": loop.status,
                    "turns": loop.turns,
                    "tool_calls": loop.tool_calls,
                    "final_provider": None if final_response is None else final_response.provider,
                    "final_model": None if final_response is None else final_response.model,
                    "final_request_id": None
                    if final_response is None
                    else final_response.request_id,
                },
                "tool_receipts": {
                    "receipt_count": len(result.authorization_receipts),
                    "status_counts": receipt_statuses,
                },
            }
        )
        result_kind = "tool_loop"
    elif isinstance(result, BrainMissionResult):
        projection = _evaluator_metadata_projection(result.brain_run)
        execution = result.execution if isinstance(result.execution, Mapping) else None
        preflight = result.preflight if isinstance(result.preflight, Mapping) else None
        projection.update(
            {
                "result_kind": "mission",
                "status": result.status,
                "route": None
                if result.route is None
                else {
                    "route_digest": _json_digest(dict(result.route)),
                    "evidence_digest": result.route.get("evidence_digest"),
                    "execution": result.route.get("execution"),
                },
                "mission": {
                    "preflight": None
                    if preflight is None
                    else _bounded_mission_report_projection(preflight, include_outputs=False),
                    "execution": None
                    if execution is None
                    else _bounded_mission_report_projection(execution, include_outputs=False),
                },
            }
        )
        result_kind = "mission"
    else:
        raise BrainRunError("result must be a BrainRunResult, BrainToolLoopResult, or BrainMissionResult")

    projection["schema"] = "bioprism-brain-evaluator-input/0.1"
    projection["result_kind"] = result_kind
    projection["evidence_digest"] = evidence_digest
    projection["evidence"] = evidence_copy
    BrainLearningLedger._assert_safe(projection)
    try:
        encoded_projection = json.dumps(
            projection,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise BrainRunError("evaluator input must be JSON-safe") from error
    if len(encoded_projection) > MAX_BRAIN_EVALUATOR_INPUT_BYTES:
        raise BrainRunError("evaluator input exceeds the bounded size")
    return json.loads(encoded_projection.decode("utf-8"))


class BrainOutcomeEvaluator:
    """Adapt a caller-owned evaluator into the value-only learning boundary.

    The callback receives :func:`build_brain_evaluation_input`, never a raw provider response or
    runtime credential. It may return a :class:`BrainEvaluatorDecision` or a mapping containing
    only ``reward``, ``passed``, ``failed``, ``feedback_digest``, and ``failure_class``. The
    adapter computes and binds the evidence digest, then delegates persistence to the brain.
    """

    _ALLOWED_DECISION_FIELDS = {
        "reward",
        "passed",
        "failed",
        "feedback_digest",
        "failure_class",
        "evidence_digest",
        "replan_requested",
        "replan_instruction",
    }

    def __init__(
        self,
        evaluator: Callable[[Mapping[str, Any]], Mapping[str, Any] | BrainEvaluatorDecision],
        *,
        evaluator_id: str,
        evaluator_version: str,
    ) -> None:
        if not callable(evaluator):
            raise BrainRunError("evaluator must be callable")
        self.evaluator = evaluator
        self.evaluator_id = evaluator_id
        self.evaluator_version = evaluator_version
        BrainEvaluatorDecision(
            evaluator_id=evaluator_id,
            evaluator_version=evaluator_version,
            reward=0.0,
            passed=False,
        )

    def assess(
        self,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
        *,
        evidence: Mapping[str, Any] | None = None,
    ) -> BrainEvaluatorDecision:
        evaluation_input = build_brain_evaluation_input(result, evidence=evidence)
        return self._assess_input(evaluation_input)

    def _assess_input(self, evaluation_input: Mapping[str, Any]) -> BrainEvaluatorDecision:
        try:
            raw_decision = self.evaluator(evaluation_input)
        except Exception as error:
            raise BrainRunError("evaluator callback failed") from error
        if isinstance(raw_decision, BrainEvaluatorDecision):
            if (
                raw_decision.evaluator_id != self.evaluator_id
                or raw_decision.evaluator_version != self.evaluator_version
            ):
                raise BrainRunError("evaluator decision identity does not match the adapter")
            decision = raw_decision
        else:
            if not isinstance(raw_decision, Mapping):
                raise BrainRunError("evaluator callback must return a decision object")
            BrainLearningLedger._assert_safe(raw_decision)
            unknown_fields = set(raw_decision) - self._ALLOWED_DECISION_FIELDS
            if unknown_fields:
                raise BrainRunError("evaluator decision contains unsupported fields")
            if "reward" not in raw_decision or "passed" not in raw_decision:
                raise BrainRunError("evaluator decision requires reward and passed")
            passed = raw_decision["passed"]
            if not isinstance(passed, bool):
                raise BrainRunError("evaluator decision passed must be boolean")
            failed = raw_decision.get("failed", not passed)
            if not isinstance(failed, bool):
                raise BrainRunError("evaluator decision failed must be boolean")
            decision = BrainEvaluatorDecision(
                evaluator_id=self.evaluator_id,
                evaluator_version=self.evaluator_version,
                reward=raw_decision["reward"],
                passed=passed,
                failed=failed,
                feedback_digest=raw_decision.get("feedback_digest"),
                failure_class=raw_decision.get("failure_class"),
                evidence_digest=raw_decision.get("evidence_digest"),
                replan_requested=raw_decision.get("replan_requested", False),
                replan_instruction=raw_decision.get("replan_instruction"),
            )
        expected_evidence_digest = evaluation_input.get("evidence_digest")
        if decision.evidence_digest is not None and decision.evidence_digest != expected_evidence_digest:
            raise BrainRunError("evaluator decision evidence_digest does not match evidence")
        if decision.evidence_digest is None and expected_evidence_digest is not None:
            decision = replace(decision, evidence_digest=expected_evidence_digest)
        return decision

    def assess_value_only_input(self, evaluation_input: Mapping[str, Any]) -> BrainEvaluatorDecision:
        """Assess a replayed, already-projected input without requiring a live provider result.

        Offline replay may retain only a caller-owned evidence packet and its digest. This public
        seam applies the same decision validation as a live run while keeping prompts, responses,
        credentials, and tool envelopes out of the replay path.
        """

        if not isinstance(evaluation_input, Mapping):
            raise BrainRunError("value-only evaluator input must be a mapping")
        BrainLearningLedger._assert_safe(evaluation_input)
        try:
            encoded = json.dumps(
                dict(evaluation_input),
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("value-only evaluator input must be JSON-safe") from error
        if len(encoded) > MAX_BRAIN_EVALUATOR_INPUT_BYTES:
            raise BrainRunError("value-only evaluator input exceeds the bounded size")
        return self._assess_input(json.loads(encoded.decode("utf-8")))

    def evaluate_and_record(
        self,
        brain: AutonomousBrain,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
        *,
        bandit_state: Mapping[str, Any],
        evidence: Mapping[str, Any] | None = None,
        arm_id: str | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> dict[str, Any]:
        """Evaluate and persist an outcome, preserving the historical report-only API."""

        _decision, report = self.evaluate_and_record_with_decision(
            brain,
            result,
            bandit_state=bandit_state,
            evidence=evidence,
            arm_id=arm_id,
            ledger=ledger,
        )
        return report

    def evaluate_and_record_with_decision(
        self,
        brain: AutonomousBrain,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
        *,
        bandit_state: Mapping[str, Any],
        evidence: Mapping[str, Any] | None = None,
        arm_id: str | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> tuple[BrainEvaluatorDecision, dict[str, Any]]:
        """Return the compact evaluator decision alongside the persisted Rust report."""

        if not isinstance(brain, AutonomousBrain):
            raise BrainRunError("brain must be an AutonomousBrain")
        evaluation_input = build_brain_evaluation_input(result, evidence=evidence)
        decision = self._assess_input(evaluation_input)
        replay = {
            "schema": BRAIN_EVALUATOR_REPLAY_SCHEMA,
            "result_kind": evaluation_input["result_kind"],
            "run_id": evaluation_input["run_id"],
            "outcome_digest": evaluation_input["outcome_digest"],
            "evaluation_input_digest": _json_digest(evaluation_input),
            "evidence_digest": evaluation_input.get("evidence_digest"),
            "evaluator_id": decision.evaluator_id,
            "evaluator_version": decision.evaluator_version,
            "decision_digest": _json_digest(decision.to_dict()),
            "retention": "metadata_and_digests_only",
        }
        report = brain.record_evaluator_outcome(
            result,
            bandit_state=bandit_state,
            evaluator_id=decision.evaluator_id,
            evaluator_version=decision.evaluator_version,
            reward=decision.reward,
            passed=decision.passed,
            arm_id=arm_id,
            failed=decision.failed,
            feedback_digest=decision.feedback_digest,
            failure_class=decision.failure_class,
            evidence_digest=decision.evidence_digest,
            ledger=ledger,
            replay_metadata=replay,
        )
        return decision, report
