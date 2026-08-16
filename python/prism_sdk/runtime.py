"""Typed runtime authorization, replay, and deterministic simulation projections.

The Rust runtime is the authority for effect policy, world tapes, budgets, and fork semantics.
This module is intentionally a projection layer rather than a second runtime: request builders
validate the transport envelope and report parsers preserve the distinctions that make a replay
auditable.  In particular, an authorization is not an execution, a simulated outcome is not an
observed outcome, and a partial recording is not a verified complete replay.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


EFFECT_KINDS = frozenset(
    {
        "clock_now",
        "clock_sleep",
        "random_bytes",
        "network_fetch",
        "file_read",
        "file_write",
        "process_spawn",
        "service_call",
        "model_call",
        "outbound_message",
        "payment",
    }
)
EFFECT_CLASSES = frozenset({"pure", "reversible_sandbox", "compensable_external", "irreversible"})
AUTHORIZATIONS = frozenset({"perform", "simulate"})
RUNTIME_RESOURCES = frozenset(
    {
        "model_tokens",
        "model_calls",
        "tool_calls",
        "wall_clock_millis",
        "task_time_millis",
        "cpu_millis",
        "memory_bytes",
        "storage_bytes",
        "network_bytes",
        "cost_micros",
    }
)
RUNTIME_TAPE_VERIFY_SCHEMA = "bioprism-mcp/runtime-tape-verify/0.1"


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _optional_mapping(name: str, value: Any) -> dict[str, Any] | None:
    return None if value is None else _route_mapping(name, value)


def _payload(value: Mapping[str, Any], *, label: str, direct_keys: tuple[str, ...]) -> dict[str, Any]:
    """Extract a projection from direct, REST, MCP, or text-content envelopes."""

    raw = _route_mapping(f"{label} response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        return "ok" in candidate and any(key in candidate for key in direct_keys)

    if matches(raw):
        return raw
    envelopes: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        envelopes.append(mcp)
    for envelope in envelopes:
        result = envelope.get("result")
        candidates: list[Mapping[str, Any]] = [envelope]
        if isinstance(result, Mapping):
            candidates.append(result)
        for candidate in candidates:
            structured = candidate.get("structuredContent")
            if isinstance(structured, Mapping) and matches(structured):
                return dict(structured)
            content = candidate.get("content")
            if not isinstance(content, Sequence) or isinstance(content, (str, bytes)):
                continue
            for block in content:
                if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                    continue
                try:
                    decoded = json.loads(block["text"])
                except json.JSONDecodeError as error:
                    raise ArgumentError(f"{label} response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping(f"decoded {label} response", decoded)
                if matches(decoded_mapping):
                    return decoded_mapping
    raise ArgumentError(f"response does not contain a {label} projection")


def _refusal(raw: Mapping[str, Any], label: str) -> tuple[str, str, str | None]:
    stage = _route_text(f"{label} stage", raw.get("stage"))
    refusal = _route_text(f"{label} refusal", raw.get("refusal"))
    fail_closed = _bool(f"{label} fail_closed", raw.get("fail_closed"))
    if not fail_closed:
        raise ArgumentError(f"refused {label} results must be fail-closed")
    return stage, refusal, _optional_text(f"{label} guarantee", raw.get("guarantee"))


def _request(value: Mapping[str, Any], label: str) -> dict[str, Any]:
    request = _route_mapping(label, value)
    kind = _route_text(f"{label}.kind", request.get("kind"))
    if kind not in EFFECT_KINDS:
        raise ArgumentError(f"{label}.kind is not a known runtime effect kind: {kind!r}")
    return request


@dataclass(frozen=True)
class RuntimeEffectCheckArgs:
    policy: Mapping[str, Any]
    request: Mapping[str, Any]

    def __post_init__(self) -> None:
        object.__setattr__(self, "policy", _route_mapping("runtime effect policy", self.policy))
        object.__setattr__(self, "request", _request(self.request, "runtime effect request"))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RuntimeEffectCheckArgs":
        raw = _route_mapping("runtime effect arguments", value)
        return cls(raw.get("policy"), raw.get("request"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"policy": dict(self.policy), "request": dict(self.request)}


@dataclass(frozen=True)
class RuntimeTapeVerifyArgs:
    tape: Mapping[str, Any]
    other_tape: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "tape", _route_mapping("runtime tape", self.tape))
        object.__setattr__(self, "other_tape", _optional_mapping("runtime comparison tape", self.other_tape))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RuntimeTapeVerifyArgs":
        raw = _route_mapping("runtime tape arguments", value)
        return cls(raw.get("tape"), raw.get("other_tape"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result = {"tape": dict(self.tape)}
        if self.other_tape is not None:
            result["other_tape"] = dict(self.other_tape)
        return result


@dataclass(frozen=True)
class RuntimeExecutionSimulateArgs:
    policy: Mapping[str, Any]
    requests: tuple[Mapping[str, Any], ...]
    run: str | None = None
    world: Mapping[str, Any] | None = None
    budget: Mapping[str, Any] | None = None
    fork: Mapping[str, Any] | None = None

    def __init__(
        self,
        policy: Mapping[str, Any],
        requests: Sequence[Mapping[str, Any]],
        run: str | None = None,
        world: Mapping[str, Any] | None = None,
        budget: Mapping[str, Any] | None = None,
        fork: Mapping[str, Any] | None = None,
    ) -> None:
        normalized_policy = _route_mapping("runtime simulation policy", policy)
        if not isinstance(requests, Sequence) or isinstance(requests, (str, bytes)):
            raise ArgumentError("runtime simulation requests must be an array")
        normalized_requests = tuple(_request(item, f"runtime simulation requests[{index}]") for index, item in enumerate(requests))
        if len(normalized_requests) > 1_000:
            raise ArgumentError("runtime simulation requests may contain at most 1000 effects")
        normalized_run = None if run is None else _route_text("runtime simulation run", run)
        object.__setattr__(self, "policy", normalized_policy)
        object.__setattr__(self, "requests", normalized_requests)
        object.__setattr__(self, "run", normalized_run)
        object.__setattr__(self, "world", _optional_mapping("runtime simulation world", world))
        object.__setattr__(self, "budget", _optional_mapping("runtime simulation budget", budget))
        normalized_fork = _optional_mapping("runtime simulation fork", fork)
        if normalized_fork is not None and "step" not in normalized_fork:
            raise ArgumentError("runtime simulation fork must declare step")
        object.__setattr__(self, "fork", normalized_fork)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RuntimeExecutionSimulateArgs":
        raw = _route_mapping("runtime simulation arguments", value)
        return cls(raw.get("policy"), raw.get("requests"), raw.get("run"), raw.get("world"), raw.get("budget"), raw.get("fork"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "policy": dict(self.policy),
            "requests": [dict(request) for request in self.requests],
        }
        for key, value in (("run", self.run), ("world", self.world), ("budget", self.budget), ("fork", self.fork)):
            if value is not None:
                result[key] = dict(value) if isinstance(value, Mapping) else value
        return result


@dataclass(frozen=True)
class RuntimeEffectReport:
    raw: dict[str, Any]
    ok: bool
    request: dict[str, Any] | None
    kind: str | None
    effect_class: str | None
    class_label: str | None
    target_host: str | None
    target_path: str | None
    authorization: str | None
    simulated_outcome: Any
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RuntimeEffectReport":
        raw = _payload(value, label="runtime effect", direct_keys=("request", "authorization"))
        ok = _bool("runtime effect ok", raw.get("ok"))
        request_value = raw.get("request")
        request = None if request_value is None else _route_mapping("runtime effect request", request_value)
        kind = _optional_text("runtime effect kind", raw.get("kind"))
        if kind is not None and kind not in EFFECT_KINDS:
            raise ArgumentError(f"unknown runtime effect kind: {kind!r}")
        effect_class = _optional_text("runtime effect class", raw.get("class"))
        if effect_class is not None and effect_class not in EFFECT_CLASSES:
            raise ArgumentError(f"unknown runtime effect class: {effect_class!r}")
        class_label = _optional_text("runtime effect class_label", raw.get("class_label"))
        if effect_class is not None and class_label != effect_class:
            raise ArgumentError("runtime effect class and class_label do not reconcile")
        stage = refusal = guarantee = None
        fail_closed = _bool("runtime effect fail_closed", raw.get("fail_closed", False))
        if not ok:
            stage, refusal, guarantee = _refusal(raw, "runtime effect")
            if raw.get("authorization") is not None:
                raise ArgumentError("refused runtime effects cannot carry authorization")
        else:
            if fail_closed or raw.get("refusal") is not None or raw.get("stage") is not None:
                raise ArgumentError("successful runtime effects cannot carry refusal evidence")
            authorization = _route_text("runtime effect authorization", raw.get("authorization"))
            if authorization not in AUTHORIZATIONS:
                raise ArgumentError(f"unknown runtime effect authorization: {authorization!r}")
            if authorization == "perform" and raw.get("simulated_outcome") is not None:
                raise ArgumentError("performed runtime effects cannot carry simulated outcomes")
        authorization = None if raw.get("authorization") is None else _route_text("runtime effect authorization", raw.get("authorization"))
        return cls(
            raw,
            ok,
            request,
            kind,
            effect_class,
            class_label,
            _optional_text("runtime effect target_host", raw.get("target_host")),
            _optional_text("runtime effect target_path", raw.get("target_path")),
            authorization,
            raw.get("simulated_outcome"),
            stage,
            refusal,
            fail_closed,
            guarantee,
            _route_strings("runtime effect guarantees", raw.get("guarantees", ())),
            _route_strings("runtime effect limitations", raw.get("limitations", ())),
        )

    @property
    def executed(self) -> bool:
        """Always false: this inspection surface never performs the requested effect."""

        return False

    @property
    def simulated(self) -> bool:
        return self.authorization == "simulate"

    @property
    def refused(self) -> bool:
        return not self.ok


@dataclass(frozen=True)
class RuntimeCheckpointProjection:
    raw: dict[str, Any]
    id: str
    step: int
    tape_head: str
    provider: str
    restoration: dict[str, Any]
    ok: bool
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RuntimeCheckpointProjection":
        raw = _route_mapping("runtime checkpoint", value)
        ok = _bool("runtime checkpoint ok", raw.get("ok"))
        fail_closed_value = raw.get("fail_closed", False)
        fail_closed = _bool("runtime checkpoint fail_closed", fail_closed_value)
        refusal = _optional_text("runtime checkpoint refusal", raw.get("refusal"))
        if ok and (refusal is not None or fail_closed):
            raise ArgumentError("successful runtime checkpoints cannot carry refusal evidence")
        if not ok and (refusal is None or not fail_closed):
            raise ArgumentError("refused runtime checkpoints must be fail-closed and explain the refusal")
        restoration = _route_mapping("runtime checkpoint restoration", raw.get("restoration"))
        _bool("runtime checkpoint restoration portable", restoration.get("portable"))
        _optional_text("runtime checkpoint restoration provider", restoration.get("requires_provider"))
        _route_text("runtime checkpoint restoration notes", restoration.get("notes"))
        return cls(
            raw,
            _route_text("runtime checkpoint id", raw.get("id")),
            _route_count("runtime checkpoint step", raw.get("step")),
            _route_text("runtime checkpoint tape_head", raw.get("tape_head")),
            _route_text("runtime checkpoint provider", raw.get("provider")),
            restoration,
            ok,
            refusal,
            fail_closed,
        )


@dataclass(frozen=True)
class RuntimeArtifactsProjection:
    raw: dict[str, Any]
    consumed: tuple[str, ...]
    created: dict[str, str]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RuntimeArtifactsProjection":
        raw = _route_mapping("runtime tape artifacts", value)
        consumed = tuple(_route_text("runtime consumed artifact", item) for item in _array("runtime artifact consumed", raw.get("consumed")))
        created_raw = _route_mapping("runtime artifact created", raw.get("created"))
        created = {key: _route_text(f"runtime created artifact {key}", item) for key, item in created_raw.items()}
        if len(consumed) != len(set(consumed)):
            raise ArgumentError("runtime consumed artifacts must be unique")
        return cls(raw, consumed, created)


@dataclass(frozen=True)
class RuntimeTapeVerifyReport:
    raw: dict[str, Any]
    ok: bool
    run: str | None
    lineage: dict[str, Any] | None
    entries: int | None
    head: str | None
    chain_verified: bool
    checkpoint_results: tuple[dict[str, Any], ...]
    artifacts: dict[str, Any] | None
    simulated_steps: tuple[int, ...]
    first_divergence: int | None
    comparison_supplied: bool
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    checkpoint_records: tuple[RuntimeCheckpointProjection, ...] = ()
    artifacts_record: RuntimeArtifactsProjection | None = None
    checkpoint_count: int = 0
    checkpoint_pass_count: int = 0
    checkpoint_failure_count: int = 0
    simulated_step_count: int = 0
    artifact_consumed_count: int = 0
    artifact_created_count: int = 0

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RuntimeTapeVerifyReport":
        raw = _payload(value, label="runtime tape verification", direct_keys=("chain_verified", "checkpoint_results"))
        ok = _bool("runtime tape verification ok", raw.get("ok"))
        fail_closed = _bool("runtime tape verification fail_closed", raw.get("fail_closed", False))
        schema = _optional_text("runtime tape verification schema", raw.get("schema"))
        if schema is not None and schema != RUNTIME_TAPE_VERIFY_SCHEMA:
            raise ArgumentError(f"unknown runtime tape verification schema: {schema!r}")
        if not ok:
            stage, refusal, guarantee = _refusal(raw, "runtime tape verification")
            return cls(raw, False, None, None, None, None, False, (), None, (), None, False, stage, refusal, True, guarantee, (), (), schema=schema)
        if fail_closed or raw.get("refusal") is not None or raw.get("stage") is not None:
            raise ArgumentError("successful runtime tape verification cannot carry refusal evidence")
        checkpoints = tuple(_route_mapping(f"runtime checkpoint_results[{index}]", item) for index, item in enumerate(_array("runtime checkpoint_results", raw.get("checkpoint_results"))))
        checkpoint_records = tuple(RuntimeCheckpointProjection.from_wire(item) for item in checkpoints)
        checkpoint_count = _route_count("runtime checkpoint_count", raw.get("checkpoint_count"))
        checkpoint_pass_count = _route_count("runtime checkpoint_pass_count", raw.get("checkpoint_pass_count"))
        checkpoint_failure_count = _route_count("runtime checkpoint_failure_count", raw.get("checkpoint_failure_count"))
        if checkpoint_count != len(checkpoint_records) or checkpoint_pass_count + checkpoint_failure_count != checkpoint_count:
            raise ArgumentError("runtime checkpoint counts do not reconcile")
        if (checkpoint_pass_count, checkpoint_failure_count) != (
            sum(checkpoint.ok for checkpoint in checkpoint_records),
            sum(not checkpoint.ok for checkpoint in checkpoint_records),
        ):
            raise ArgumentError("runtime checkpoint counts do not match checkpoint rows")
        simulated = _array("runtime simulated_steps", raw.get("simulated_steps"))
        simulated_steps: list[int] = []
        for index, item in enumerate(simulated):
            if isinstance(item, bool) or not isinstance(item, int) or item < 0:
                raise ArgumentError(f"runtime simulated_steps[{index}] must be a non-negative integer")
            simulated_steps.append(item)
        first = raw.get("first_divergence")
        if first is not None:
            first = _route_count("runtime first_divergence", first)
        artifacts_record = RuntimeArtifactsProjection.from_wire(raw.get("artifacts"))
        artifact_consumed_count = _route_count("runtime artifact_consumed_count", raw.get("artifact_consumed_count"))
        artifact_created_count = _route_count("runtime artifact_created_count", raw.get("artifact_created_count"))
        if (artifact_consumed_count, artifact_created_count) != (len(artifacts_record.consumed), len(artifacts_record.created)):
            raise ArgumentError("runtime artifact counts do not reconcile")
        simulated_step_count = _route_count("runtime simulated_step_count", raw.get("simulated_step_count"))
        if simulated_step_count != len(simulated_steps):
            raise ArgumentError("runtime simulated step count does not reconcile")
        return cls(
            raw,
            True,
            _route_text("runtime tape run", raw.get("run")),
            _optional_mapping("runtime tape lineage", raw.get("lineage")),
            _route_count("runtime tape entries", raw.get("entries")),
            _route_text("runtime tape head", raw.get("head")),
            _bool("runtime chain_verified", raw.get("chain_verified")),
            checkpoints,
            _route_mapping("runtime tape artifacts", raw.get("artifacts")),
            tuple(simulated_steps),
            first,
            _bool("runtime comparison_supplied", raw.get("comparison_supplied")),
            None,
            None,
            False,
            None,
            _route_strings("runtime tape guarantees", raw.get("guarantees")),
            _route_strings("runtime tape limitations", raw.get("limitations")),
            schema=schema,
            checkpoint_records=checkpoint_records,
            artifacts_record=artifacts_record,
            checkpoint_count=checkpoint_count,
            checkpoint_pass_count=checkpoint_pass_count,
            checkpoint_failure_count=checkpoint_failure_count,
            simulated_step_count=simulated_step_count,
            artifact_consumed_count=artifact_consumed_count,
            artifact_created_count=artifact_created_count,
        )

    @property
    def diverged(self) -> bool:
        return self.first_divergence is not None

    @property
    def checkpoint_failures(self) -> tuple[dict[str, Any], ...]:
        return tuple(row for row in self.checkpoint_results if row.get("ok") is False)

    @property
    def has_simulated_steps(self) -> bool:
        return bool(self.simulated_steps)


@dataclass(frozen=True)
class RuntimeExecutionSimulateReport:
    raw: dict[str, Any]
    ok: bool
    run: str | None
    request_count: int | None
    recorded_requests: int | None
    live_outcomes: tuple[Any, ...]
    execution_error: str | None
    tape: dict[str, Any] | None
    world: dict[str, Any] | None
    policy_journal: tuple[Any, ...]
    budget: dict[str, Any] | None
    replay: dict[str, Any] | None
    fork: dict[str, Any] | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RuntimeExecutionSimulateReport":
        raw = _payload(value, label="runtime execution simulation", direct_keys=("tape", "replay"))
        ok = _bool("runtime simulation ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("runtime execution simulation transport projection is not successful")
        replay = _route_mapping("runtime simulation replay", raw.get("replay"))
        verified = _bool("runtime replay verified", replay.get("verified"))
        matched = _bool("runtime replay matched", replay.get("matched"))
        if verified and not matched:
            raise ArgumentError("verified runtime replay cannot be unmatched")
        request_count = _route_count("runtime simulation request_count", raw.get("request_count"))
        recorded_requests = _route_count("runtime simulation recorded_requests", raw.get("recorded_requests"))
        if recorded_requests > request_count:
            raise ArgumentError("runtime recorded_requests cannot exceed request_count")
        return cls(
            raw,
            True,
            _route_text("runtime simulation run", raw.get("run")),
            request_count,
            recorded_requests,
            _array("runtime live_outcomes", raw.get("live_outcomes")),
            _optional_text("runtime execution_error", raw.get("execution_error")),
            _route_mapping("runtime simulation tape", raw.get("tape")),
            _route_mapping("runtime simulation world", raw.get("world")),
            _array("runtime policy_journal", raw.get("policy_journal")),
            _optional_mapping("runtime simulation budget", raw.get("budget")),
            replay,
            _optional_mapping("runtime simulation fork", raw.get("fork")),
            _route_strings("runtime simulation guarantees", raw.get("guarantees")),
            _route_strings("runtime simulation limitations", raw.get("limitations")),
        )

    @property
    def complete_recording(self) -> bool:
        return self.execution_error is None and self.recorded_requests == self.request_count

    @property
    def partial_recording(self) -> bool:
        return not self.complete_recording

    @property
    def replay_verified(self) -> bool:
        return bool(self.replay and self.replay.get("verified") is True)

    @property
    def replay_matched(self) -> bool:
        return bool(self.replay and self.replay.get("matched") is True)

    @property
    def budget_exhausted(self) -> bool:
        budget = self.budget or {}
        accounting = budget.get("accounting")
        return bool(budget.get("aborted_on")) or "budget exhausted" in (self.execution_error or "").lower() or isinstance(accounting, Mapping) and any(
            isinstance(value, Mapping) and isinstance(value.get("used"), int) and isinstance(value.get("limit"), Mapping) and value["used"] >= value["limit"].get("hard", -1)
            for value in accounting.values()
        )

    @property
    def live_effects_reachable(self) -> bool:
        return False


def runtime_effect_check_report(value: Mapping[str, Any]) -> RuntimeEffectReport:
    return RuntimeEffectReport.from_wire(value)


def runtime_tape_verify_report(value: Mapping[str, Any]) -> RuntimeTapeVerifyReport:
    return RuntimeTapeVerifyReport.from_wire(value)


def runtime_execution_simulate_report(value: Mapping[str, Any]) -> RuntimeExecutionSimulateReport:
    return RuntimeExecutionSimulateReport.from_wire(value)


__all__ = [
    "AUTHORIZATIONS",
    "EFFECT_CLASSES",
    "EFFECT_KINDS",
    "RUNTIME_RESOURCES",
    "RUNTIME_TAPE_VERIFY_SCHEMA",
    "RuntimeEffectCheckArgs",
    "RuntimeEffectReport",
    "RuntimeExecutionSimulateArgs",
    "RuntimeExecutionSimulateReport",
    "RuntimeArtifactsProjection",
    "RuntimeCheckpointProjection",
    "RuntimeTapeVerifyArgs",
    "RuntimeTapeVerifyReport",
    "runtime_effect_check_report",
    "runtime_execution_simulate_report",
    "runtime_tape_verify_report",
]
