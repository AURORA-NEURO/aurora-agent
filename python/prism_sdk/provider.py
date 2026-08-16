"""Typed provider capability-gate projections.

Provider cards are evidence ledgers.  Untested, failed, and passed checks have different meanings;
performance observations are measurements rather than pass/fail claims; and a comparison with an
untested side is indeterminate.  The SDK preserves those distinctions without authorizing runtime
execution or treating a cleared gate as a general provider guarantee.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


PASS_FAIL_CHECKS = frozenset(
    {
        "file_write_and_rollback",
        "background_process_survival",
        "service_snapshot",
        "clock_control",
        "seeded_faults",
        "secret_isolation",
        "cancellation",
        "timeout",
        "artifact_closure",
        "branch_independence",
        "host_escape",
        "credential_exfiltration",
        "network_bypass",
        "cross_trial_contamination",
        "malicious_image",
        "decompression_bomb",
        "symlink_and_mount_attack",
    }
)
PERFORMANCE_CHECKS = frozenset(
    {
        "cold_startup",
        "warm_startup",
        "image_pull",
        "snapshot",
        "resume",
        "fork",
        "event_throughput",
        "artifact_upload",
        "cleanup",
        "cache_hit",
        "parallel_scaling",
    }
)
CHECK_NAMES = PASS_FAIL_CHECKS | PERFORMANCE_CHECKS
DEBUG_CHECK_NAMES = {"".join(part.title() for part in check.split("_")) for check in CHECK_NAMES}


def _object(name: str, value: Any) -> dict[str, Any]:
    return _route_mapping(name, value)


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("provider gate response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        return "ok" in candidate and "gate" in candidate

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
                    raise ArgumentError(f"provider gate response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping("decoded provider gate response", decoded)
                if matches(decoded_mapping):
                    return decoded_mapping
    raise ArgumentError("response does not contain a provider capability gate projection")


def _check(name: str, value: Any, *, pass_fail_only: bool = False) -> str:
    check = _route_text(name, value)
    if check not in CHECK_NAMES or (pass_fail_only and check not in PASS_FAIL_CHECKS):
        raise ArgumentError(f"{name} is not an allowed provider capability check")
    return check


def _run(name: str, value: Any) -> dict[str, Any]:
    run = _object(name, value)
    _route_text(f"{name}.run_id", run.get("run_id"))
    _route_text(f"{name}.reproducible_environment", run.get("reproducible_environment"))
    return run


def _state(name: str, value: Any) -> dict[str, Any]:
    state = _object(name, value)
    tag = _route_text(f"{name}.state", state.get("state"))
    if tag == "passed":
        _run(f"{name}.run", state.get("run"))
    elif tag == "failed":
        _route_text(f"{name}.witness", state.get("witness"))
        _run(f"{name}.run", state.get("run"))
    elif tag != "untested":
        raise ArgumentError(f"{name}.state is not recognized")
    return state


@dataclass(frozen=True)
class ProviderCapabilityGateArgs:
    card: Mapping[str, Any]
    required: tuple[str, ...]
    other_card: Mapping[str, Any] | None = None
    include_card: bool = False

    def __post_init__(self) -> None:
        card = _object("provider capability card", self.card)
        encoded = json.dumps(card, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
        if len(encoded) > 5_000_000:
            raise ArgumentError("provider capability card must be at most 5000000 bytes")
        _route_text("provider capability card.provider", card.get("provider"))
        states = _object("provider capability card.states", card.get("states", {}))
        for name, state in states.items():
            _state(f"provider capability card.states[{name!r}]", state)
        measurements = _array("provider capability card.measurements", card.get("measurements", ()))
        for index, measurement in enumerate(measurements):
            item = _object(f"provider capability card.measurements[{index}]", measurement)
            _check(f"provider capability card.measurements[{index}].check", item.get("check"))
            _finite(f"provider capability card.measurements[{index}].value", item.get("value"))
            _route_text(f"provider capability card.measurements[{index}].unit", item.get("unit"))
            _run(f"provider capability card.measurements[{index}].run", item.get("run"))
        object.__setattr__(self, "card", card)
        required = tuple(_check(f"provider required[{index}]", item, pass_fail_only=True) for index, item in enumerate(_array("provider required", self.required)))
        if not 1 <= len(required) <= 17:
            raise ArgumentError("provider required must contain between 1 and 17 checks")
        if len(set(required)) != len(required):
            raise ArgumentError("provider required must not contain duplicate checks")
        object.__setattr__(self, "required", required)
        if self.other_card is not None:
            other = _object("other provider capability card", self.other_card)
            _route_text("other provider capability card.provider", other.get("provider"))
            object.__setattr__(self, "other_card", other)
        object.__setattr__(self, "include_card", _bool("provider include_card", self.include_card))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ProviderCapabilityGateArgs":
        raw = _object("provider gate arguments", value)
        return cls(raw.get("card"), tuple(_route_text("provider required", item) for item in _array("provider required", raw.get("required"))), raw.get("other_card"), raw.get("include_card", False))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"card": dict(self.card), "required": list(self.required), "include_card": self.include_card}
        if self.other_card is not None:
            result["other_card"] = dict(self.other_card)
        return result


@dataclass(frozen=True)
class ProviderCapabilityGateReport:
    raw: dict[str, Any]
    ok: bool
    provider: str | None
    required: tuple[str, ...]
    required_states: dict[str, dict[str, Any]]
    gate: dict[str, Any]
    claims: tuple[str, ...]
    measurement_count: int
    differential: dict[str, dict[str, Any]] | None
    card: dict[str, Any] | None
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ProviderCapabilityGateReport":
        raw = _payload(value)
        if not _bool("provider gate ok", raw.get("ok")):
            raise ArgumentError("provider capability gate projection must be successful")
        provider = None if raw.get("provider") is None else _route_text("provider gate provider", raw.get("provider"))
        required = tuple(_check(f"provider gate required[{index}]", item, pass_fail_only=True) for index, item in enumerate(_array("provider gate required", raw.get("required"))))
        states_raw = _object("provider gate required_states", raw.get("required_states"))
        required_states = {name: _state(f"provider gate required_states[{name!r}]", state) for name, state in states_raw.items()}
        gate = _object("provider gate gate", raw.get("gate"))
        outcome = _route_text("provider gate gate.outcome", gate.get("outcome"))
        if outcome == "blocked":
            unproven = _route_strings("provider gate gate.unproven", gate.get("unproven"))
            if not unproven:
                raise ArgumentError("blocked provider gates must identify unproven checks")
            gate = {**gate, "unproven": list(unproven)}
        elif outcome != "cleared":
            raise ArgumentError("provider gate outcome must be cleared or blocked")
        claims = tuple(_check(f"provider gate claims[{index}]", item, pass_fail_only=True) for index, item in enumerate(_array("provider gate claims", raw.get("claims", ()))))
        measurement_count = _route_count("provider gate measurement_count", raw.get("measurement_count"))
        differential_raw = raw.get("differential")
        differential = None
        if differential_raw is not None:
            differential_map = _object("provider gate differential", differential_raw)
            differential = {}
            for name, drift_value in differential_map.items():
                if name not in DEBUG_CHECK_NAMES:
                    raise ArgumentError(f"provider gate differential contains unknown check {name!r}")
                drift = _object(f"provider gate differential[{name!r}]", drift_value)
                tag = _route_text(f"provider gate differential[{name!r}].drift", drift.get("drift"))
                if tag == "indeterminate":
                    _route_strings(f"provider gate differential[{name!r}].untested", drift.get("untested"))
                elif tag == "differ":
                    _route_text(f"provider gate differential[{name!r}].left", drift.get("left"))
                    _route_text(f"provider gate differential[{name!r}].right", drift.get("right"))
                elif tag != "agree":
                    raise ArgumentError(f"provider gate differential[{name!r}].drift is not recognized")
                differential[name] = drift
        card = None if raw.get("card") is None else _object("provider gate returned card", raw.get("card"))
        return cls(raw, True, provider, required, required_states, gate, claims, measurement_count, differential, card, _route_strings("provider gate guarantees", raw.get("guarantees", ())))

    @property
    def cleared(self) -> bool:
        return self.gate.get("outcome") == "cleared"

    @property
    def blocked(self) -> bool:
        return not self.cleared

    @property
    def has_untested_required(self) -> bool:
        return any(state.get("state") == "untested" for state in self.required_states.values())


def provider_capability_gate_report(value: Mapping[str, Any]) -> ProviderCapabilityGateReport:
    return ProviderCapabilityGateReport.from_wire(value)

