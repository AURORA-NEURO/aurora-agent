"""Typed estimand and identification audits for the bioevaluation kernel."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BIOEVAL_ESTIMAND_SCHEMA = "bioprism-mcp/bioeval-estimand-audit/0.1"
BIOEVAL_CLAIM_KINDS = frozenset({"association", "intervention"})
BIOEVAL_EVIDENTIARY_KINDS = frozenset({"model_conditional", "observational", "experimental"})
BIOEVAL_IDENTIFICATION_STATES = frozenset({"not_assessed", "declared", "probed"})
MAX_BIOEVAL_ESTIMAND_CORROBORATIONS = 256
MAX_BIOEVAL_ESTIMAND_TRANSPORT_REQUESTS = 256
MAX_BIOEVAL_ESTIMAND_TEXT_BYTES = 4_096
MAX_BIOEVAL_ESTIMAND_INPUT_BYTES = 20_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _text(name: str, value: Any) -> str:
    text = _route_text(name, value)
    if not text.strip() or len(text.encode("utf-8")) > MAX_BIOEVAL_ESTIMAND_TEXT_BYTES:
        raise ArgumentError(f"{name} must contain 1 to {MAX_BIOEVAL_ESTIMAND_TEXT_BYTES} UTF-8 bytes")
    return text


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("bioeval estimand response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BIOEVAL_ESTIMAND_SCHEMA and isinstance(candidate.get("estimand"), Mapping)
        return candidate.get("ok") is False and isinstance(candidate.get("stage"), str) and isinstance(candidate.get("refusal"), str)

    candidates: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        candidates.append(mcp)
        result = mcp.get("result")
        if isinstance(result, Mapping):
            candidates.append(result)
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"bioeval estimand response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a bioeval estimand projection")


@dataclass(frozen=True)
class BioevalEstimandArgs:
    intervention: str
    comparator: str
    unit: str
    outcome: str
    horizon: str
    scope: str

    def __post_init__(self) -> None:
        for name in ("intervention", "comparator", "unit", "outcome", "horizon", "scope"):
            object.__setattr__(self, name, _text(f"bioeval estimand {name}", getattr(self, name)))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalEstimandArgs":
        raw = _route_mapping("bioeval estimand", value)
        return cls(
            _text("bioeval estimand intervention", raw.get("intervention")),
            _text("bioeval estimand comparator", raw.get("comparator")),
            _text("bioeval estimand unit", raw.get("unit")),
            _text("bioeval estimand outcome", raw.get("outcome")),
            _text("bioeval estimand horizon", raw.get("horizon")),
            _text("bioeval estimand scope", raw.get("scope")),
        )

    def to_wire(self) -> dict[str, str]:
        return {
            "intervention": self.intervention,
            "comparator": self.comparator,
            "unit": self.unit,
            "outcome": self.outcome,
            "horizon": self.horizon,
            "scope": self.scope,
        }


@dataclass(frozen=True)
class BioevalBasisArgs:
    evidentiary: str
    source: str

    def __post_init__(self) -> None:
        evidentiary = _text("bioeval evidentiary kind", self.evidentiary)
        source = _text("bioeval evidentiary source", self.source)
        if evidentiary not in BIOEVAL_EVIDENTIARY_KINDS:
            raise ArgumentError("bioeval evidentiary kind is not recognized")
        object.__setattr__(self, "evidentiary", evidentiary)
        object.__setattr__(self, "source", source)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalBasisArgs":
        raw = _route_mapping("bioeval evidentiary basis", value)
        kind = _text("bioeval evidentiary kind", raw.get("evidentiary"))
        key = {"model_conditional": "model", "observational": "dataset", "experimental": "study"}.get(kind)
        if key is None:
            raise ArgumentError("bioeval evidentiary kind is not recognized")
        return cls(kind, _text(f"bioeval evidentiary {key}", raw.get(key)))

    def to_wire(self) -> dict[str, str]:
        key = {"model_conditional": "model", "observational": "dataset", "experimental": "study"}[self.evidentiary]
        return {"evidentiary": self.evidentiary, key: self.source}


@dataclass(frozen=True)
class BioevalIdentificationCheckArgs:
    name: str
    passed: bool
    detail: str

    def __post_init__(self) -> None:
        object.__setattr__(self, "name", _text("bioeval identification check name", self.name))
        object.__setattr__(self, "passed", _bool("bioeval identification check passed", self.passed))
        object.__setattr__(self, "detail", _text("bioeval identification check detail", self.detail))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalIdentificationCheckArgs":
        raw = _route_mapping("bioeval identification check", value)
        return cls(
            _text("bioeval identification check name", raw.get("name")),
            _bool("bioeval identification check passed", raw.get("passed")),
            _text("bioeval identification check detail", raw.get("detail")),
        )

    def to_wire(self) -> dict[str, Any]:
        return {"name": self.name, "passed": self.passed, "detail": self.detail}


@dataclass(frozen=True)
class BioevalIdentificationArgs:
    identification: str = "not_assessed"
    strategy: str | None = None
    assumptions: tuple[str, ...] = ()
    checks: tuple[BioevalIdentificationCheckArgs, ...] = ()

    def __post_init__(self) -> None:
        state = _text("bioeval identification state", self.identification)
        if state not in BIOEVAL_IDENTIFICATION_STATES:
            raise ArgumentError("bioeval identification state is not recognized")
        strategy = None if self.strategy is None else _text("bioeval identification strategy", self.strategy)
        assumptions = tuple(_text(f"bioeval identification assumption[{index}]", item) for index, item in enumerate(self.assumptions))
        checks = tuple(item if isinstance(item, BioevalIdentificationCheckArgs) else BioevalIdentificationCheckArgs.from_wire(item) for item in self.checks)
        if state != "not_assessed" and strategy is None:
            raise ArgumentError("bioeval declared or probed identification requires strategy")
        if state != "probed" and checks:
            raise ArgumentError("bioeval identification checks require probed identification")
        if state == "probed" and not checks:
            raise ArgumentError("bioeval probed identification requires at least one check")
        object.__setattr__(self, "identification", state)
        object.__setattr__(self, "strategy", strategy)
        object.__setattr__(self, "assumptions", assumptions)
        object.__setattr__(self, "checks", checks)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalIdentificationArgs":
        raw = _route_mapping("bioeval identification", value)
        return cls(
            _text("bioeval identification state", raw.get("identification")),
            None if raw.get("strategy") is None else _text("bioeval identification strategy", raw.get("strategy")),
            tuple(_text(f"bioeval identification assumption[{index}]", item) for index, item in enumerate(_array("bioeval identification assumptions", raw.get("assumptions", [])))),
            tuple(BioevalIdentificationCheckArgs.from_wire(item) for item in _array("bioeval identification checks", raw.get("checks", []))),
        )

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"identification": self.identification}
        if self.identification != "not_assessed":
            result["strategy"] = self.strategy
            result["assumptions"] = list(self.assumptions)
        if self.identification == "probed":
            result["checks"] = [check.to_wire() for check in self.checks]
        return result


@dataclass(frozen=True)
class BioevalCorroborationArgs:
    source: str
    kind: str
    detail: str

    def __post_init__(self) -> None:
        object.__setattr__(self, "source", _text("bioeval corroboration source", self.source))
        kind = _text("bioeval corroboration kind", self.kind)
        if kind not in BIOEVAL_CLAIM_KINDS:
            raise ArgumentError("bioeval corroboration kind must be association or intervention")
        object.__setattr__(self, "kind", kind)
        object.__setattr__(self, "detail", _text("bioeval corroboration detail", self.detail))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalCorroborationArgs":
        raw = _route_mapping("bioeval corroboration", value)
        return cls(
            _text("bioeval corroboration source", raw.get("source")),
            _text("bioeval corroboration kind", raw.get("kind")),
            _text("bioeval corroboration detail", raw.get("detail")),
        )

    def to_wire(self) -> dict[str, str]:
        return {"source": self.source, "kind": self.kind, "detail": self.detail}


@dataclass(frozen=True)
class BioevalTransportRequestArgs:
    target: str
    declared_scopes: tuple[str, ...]

    def __post_init__(self) -> None:
        object.__setattr__(self, "target", _text("bioeval transport target", self.target))
        scopes = tuple(_text(f"bioeval declared scope[{index}]", item) for index, item in enumerate(self.declared_scopes))
        if len(scopes) != len(set(scopes)):
            raise ArgumentError("bioeval declared scopes must be unique")
        object.__setattr__(self, "declared_scopes", scopes)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalTransportRequestArgs":
        raw = _route_mapping("bioeval transport request", value)
        return cls(
            _text("bioeval transport target", raw.get("target")),
            tuple(_text(f"bioeval declared scope[{index}]", item) for index, item in enumerate(_array("bioeval declared scopes", raw.get("declared_scopes")))),
        )

    def to_wire(self) -> dict[str, Any]:
        return {"target": self.target, "declared_scopes": list(self.declared_scopes)}


@dataclass(frozen=True)
class BioevalEstimandAuditArgs:
    estimand: BioevalEstimandArgs | Mapping[str, Any]
    kind: str
    basis: BioevalBasisArgs | Mapping[str, Any]
    identification: BioevalIdentificationArgs | Mapping[str, Any] | None = None
    corroborations: tuple[BioevalCorroborationArgs, ...] = ()
    transport_requests: tuple[BioevalTransportRequestArgs, ...] = ()
    require_identification: bool = False
    require_corroboration: bool = False
    strict_transport: bool = False

    def __post_init__(self) -> None:
        estimand = self.estimand if isinstance(self.estimand, BioevalEstimandArgs) else BioevalEstimandArgs.from_wire(self.estimand)
        basis = self.basis if isinstance(self.basis, BioevalBasisArgs) else BioevalBasisArgs.from_wire(self.basis)
        kind = _text("bioeval claim kind", self.kind)
        if kind not in BIOEVAL_CLAIM_KINDS:
            raise ArgumentError("bioeval claim kind must be association or intervention")
        identification = None if self.identification is None else (self.identification if isinstance(self.identification, BioevalIdentificationArgs) else BioevalIdentificationArgs.from_wire(self.identification))
        corroborations = tuple(item if isinstance(item, BioevalCorroborationArgs) else BioevalCorroborationArgs.from_wire(item) for item in self.corroborations)
        transport_requests = tuple(item if isinstance(item, BioevalTransportRequestArgs) else BioevalTransportRequestArgs.from_wire(item) for item in self.transport_requests)
        if len(corroborations) > MAX_BIOEVAL_ESTIMAND_CORROBORATIONS or len(transport_requests) > MAX_BIOEVAL_ESTIMAND_TRANSPORT_REQUESTS:
            raise ArgumentError("bioeval corroborations and transport_requests are each bounded at 256 rows")
        if len({item.target for item in transport_requests}) != len(transport_requests):
            raise ArgumentError("bioeval transport targets must be unique")
        for name in ("require_identification", "require_corroboration", "strict_transport"):
            _bool(f"bioeval {name}", getattr(self, name))
        if self.require_identification and identification is not None and identification.identification == "not_assessed":
            raise ArgumentError("bioeval identification is not assessed but require_identification is true")
        object.__setattr__(self, "estimand", estimand)
        object.__setattr__(self, "basis", basis)
        object.__setattr__(self, "kind", kind)
        object.__setattr__(self, "identification", identification)
        object.__setattr__(self, "corroborations", corroborations)
        object.__setattr__(self, "transport_requests", transport_requests)
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_BIOEVAL_ESTIMAND_INPUT_BYTES:
            raise ArgumentError("bioeval estimand input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalEstimandAuditArgs":
        raw = _route_mapping("bioeval estimand arguments", value)
        return cls(
            BioevalEstimandArgs.from_wire(raw.get("estimand")),
            _text("bioeval claim kind", raw.get("kind")),
            BioevalBasisArgs.from_wire(raw.get("basis")),
            None if raw.get("identification") is None else BioevalIdentificationArgs.from_wire(raw.get("identification")),
            tuple(BioevalCorroborationArgs.from_wire(item) for item in _array("bioeval corroborations", raw.get("corroborations", []))),
            tuple(BioevalTransportRequestArgs.from_wire(item) for item in _array("bioeval transport requests", raw.get("transport_requests", []))),
            raw.get("require_identification", False),
            raw.get("require_corroboration", False),
            raw.get("strict_transport", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "estimand": self.estimand.to_wire(),
            "kind": self.kind,
            "basis": self.basis.to_wire(),
            "identification": None if self.identification is None else self.identification.to_wire(),
            "corroborations": [item.to_wire() for item in self.corroborations],
            "transport_requests": [item.to_wire() for item in self.transport_requests],
            "require_identification": self.require_identification,
            "require_corroboration": self.require_corroboration,
            "strict_transport": self.strict_transport,
        }


@dataclass(frozen=True)
class BioevalEstimandAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    estimand: Mapping[str, Any] | None
    claim: Mapping[str, Any] | None
    policies: Mapping[str, Any] | None
    transport: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalEstimandAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("bioeval estimand refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), None, None, None, None, _route_text("bioeval estimand refusal stage", raw.get("stage")), _route_text("bioeval estimand refusal", raw.get("refusal")), _route_strings("bioeval estimand refusal guarantees", raw.get("guarantees", [])), _route_strings("bioeval estimand refusal limitations", raw.get("limitations", [])), True)
        if raw.get("ok") is not True or raw.get("schema") != BIOEVAL_ESTIMAND_SCHEMA:
            raise ArgumentError("bioeval estimand projection has an invalid schema")
        return cls(
            raw,
            True,
            BIOEVAL_ESTIMAND_SCHEMA,
            _route_text("bioeval estimand workflow", raw.get("workflow")),
            _route_mapping("bioeval estimand projection", raw.get("estimand")),
            _route_mapping("bioeval claim projection", raw.get("claim")),
            _route_mapping("bioeval estimand policies", raw.get("policies")),
            _route_mapping("bioeval transport projection", raw.get("transport")),
            None,
            None,
            _route_strings("bioeval estimand guarantees", raw.get("guarantees", [])),
            _route_strings("bioeval estimand limitations", raw.get("limitations", [])),
            False,
        )

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def still_model_conditional(self) -> bool | None:
        return None if self.claim is None else self.claim.get("still_model_conditional")

    @property
    def identification_status(self) -> str | None:
        if self.claim is None or not isinstance(self.claim.get("identification_summary"), Mapping):
            return None
        value = self.claim["identification_summary"].get("status")
        return value if isinstance(value, str) else None

    @property
    def transport_refused_count(self) -> int | None:
        return None if self.transport is None else _route_count("bioeval transport refused", self.transport.get("refused"))

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioeval_estimand_audit_report(value: Mapping[str, Any]) -> BioevalEstimandAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BioevalEstimandAuditReport.from_wire(value)


__all__ = [
    "BIOEVAL_ESTIMAND_SCHEMA",
    "BIOEVAL_CLAIM_KINDS",
    "BIOEVAL_EVIDENTIARY_KINDS",
    "BIOEVAL_IDENTIFICATION_STATES",
    "MAX_BIOEVAL_ESTIMAND_CORROBORATIONS",
    "MAX_BIOEVAL_ESTIMAND_TRANSPORT_REQUESTS",
    "MAX_BIOEVAL_ESTIMAND_TEXT_BYTES",
    "MAX_BIOEVAL_ESTIMAND_INPUT_BYTES",
    "BioevalEstimandArgs",
    "BioevalBasisArgs",
    "BioevalIdentificationCheckArgs",
    "BioevalIdentificationArgs",
    "BioevalCorroborationArgs",
    "BioevalTransportRequestArgs",
    "BioevalEstimandAuditArgs",
    "BioevalEstimandAuditReport",
    "bioeval_estimand_audit_report",
]
