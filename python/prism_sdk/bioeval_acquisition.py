"""Typed acquisition-trace audits for the bioevaluation kernel."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BIOEVAL_ACQUISITION_SCHEMA = "bioprism-mcp/bioeval-acquisition-audit/0.1"
BIOEVAL_ACQUISITION_KINDS = frozenset({"retrieval", "assay", "metadata", "expert", "analysis"})
MAX_BIOEVAL_ACQUISITION_ROWS = 512
MAX_BIOEVAL_ACQUISITION_INPUT_BYTES = 20_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _non_negative_integer(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("bioeval acquisition response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BIOEVAL_ACQUISITION_SCHEMA and isinstance(candidate.get("status"), str)
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
                        raise ArgumentError(f"bioeval acquisition response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a bioeval acquisition projection")


@dataclass(frozen=True)
class BioevalAcquisitionObligationArgs:
    id: str
    required: bool

    def __post_init__(self) -> None:
        identifier = _route_text("bioeval acquisition obligation id", self.id)
        if not identifier.strip() or len(identifier.encode("utf-8")) > 256:
            raise ArgumentError("bioeval acquisition obligation ids must contain 1 to 256 UTF-8 bytes")
        if not isinstance(self.required, bool):
            raise ArgumentError("bioeval acquisition obligation required must be a boolean")
        object.__setattr__(self, "id", identifier)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalAcquisitionObligationArgs":
        raw = _route_mapping("bioeval acquisition obligation", value)
        return cls(_route_text("bioeval acquisition obligation id", raw.get("id")), raw.get("required"))

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "required": self.required}


@dataclass(frozen=True)
class BioevalAcquisitionActionArgs:
    id: str
    kind: str
    cost: int
    closes: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        identifier = _route_text("bioeval acquisition action id", self.id)
        if not identifier.strip() or len(identifier.encode("utf-8")) > 256:
            raise ArgumentError("bioeval acquisition action ids must contain 1 to 256 UTF-8 bytes")
        kind = _route_text("bioeval acquisition action kind", self.kind)
        if kind not in BIOEVAL_ACQUISITION_KINDS:
            raise ArgumentError("bioeval acquisition action kind is not recognized")
        cost = _non_negative_integer("bioeval acquisition action cost", self.cost)
        closes = tuple(_route_text(f"bioeval acquisition closes[{index}]", item) for index, item in enumerate(self.closes))
        if len(closes) != len(set(closes)):
            raise ArgumentError("bioeval acquisition action closes must be unique")
        object.__setattr__(self, "id", identifier)
        object.__setattr__(self, "kind", kind)
        object.__setattr__(self, "cost", cost)
        object.__setattr__(self, "closes", closes)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalAcquisitionActionArgs":
        raw = _route_mapping("bioeval acquisition action", value)
        return cls(
            _route_text("bioeval acquisition action id", raw.get("id")),
            _route_text("bioeval acquisition action kind", raw.get("kind")),
            _non_negative_integer("bioeval acquisition action cost", raw.get("cost")),
            tuple(_route_text(f"bioeval acquisition closes[{index}]", item) for index, item in enumerate(_array("bioeval acquisition closes", raw.get("closes", [])))),
        )

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "kind": self.kind, "cost": self.cost, "closes": list(self.closes)}


@dataclass(frozen=True)
class BioevalAcquisitionReferencePolicyArgs:
    name: str
    cost: int
    admissible: bool

    def __post_init__(self) -> None:
        name = _route_text("bioeval acquisition reference name", self.name)
        if not name.strip() or len(name.encode("utf-8")) > 256:
            raise ArgumentError("bioeval acquisition reference names must contain 1 to 256 UTF-8 bytes")
        cost = _non_negative_integer("bioeval acquisition reference cost", self.cost)
        if not isinstance(self.admissible, bool):
            raise ArgumentError("bioeval acquisition reference admissible must be a boolean")
        object.__setattr__(self, "name", name)
        object.__setattr__(self, "cost", cost)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalAcquisitionReferencePolicyArgs":
        raw = _route_mapping("bioeval acquisition reference policy", value)
        return cls(_route_text("bioeval acquisition reference name", raw.get("name")), _non_negative_integer("bioeval acquisition reference cost", raw.get("cost")), raw.get("admissible"))

    def to_wire(self) -> dict[str, Any]:
        return {"name": self.name, "cost": self.cost, "admissible": self.admissible}


@dataclass(frozen=True)
class BioevalAcquisitionAuditArgs:
    obligations: tuple[BioevalAcquisitionObligationArgs, ...]
    actions: tuple[BioevalAcquisitionActionArgs, ...]
    stopped_after: bool = False
    reference_policy: BioevalAcquisitionReferencePolicyArgs | None = None
    require_reference: bool = False

    def __post_init__(self) -> None:
        obligations = tuple(item if isinstance(item, BioevalAcquisitionObligationArgs) else BioevalAcquisitionObligationArgs.from_wire(item) for item in self.obligations)
        actions = tuple(item if isinstance(item, BioevalAcquisitionActionArgs) else BioevalAcquisitionActionArgs.from_wire(item) for item in self.actions)
        if len(obligations) > MAX_BIOEVAL_ACQUISITION_ROWS or len(actions) > MAX_BIOEVAL_ACQUISITION_ROWS:
            raise ArgumentError("bioeval acquisition obligations and actions are each bounded at 512 rows")
        if len({item.id for item in obligations}) != len(obligations):
            raise ArgumentError("bioeval acquisition obligation ids must be unique")
        if len({item.id for item in actions}) != len(actions):
            raise ArgumentError("bioeval acquisition action ids must be unique")
        if not isinstance(self.stopped_after, bool) or not isinstance(self.require_reference, bool):
            raise ArgumentError("bioeval acquisition stopping and require_reference must be booleans")
        reference = None if self.reference_policy is None else (self.reference_policy if isinstance(self.reference_policy, BioevalAcquisitionReferencePolicyArgs) else BioevalAcquisitionReferencePolicyArgs.from_wire(self.reference_policy))
        if self.require_reference and reference is None:
            raise ArgumentError("bioeval acquisition reference_policy is required when require_reference is true")
        object.__setattr__(self, "obligations", obligations)
        object.__setattr__(self, "actions", actions)
        object.__setattr__(self, "reference_policy", reference)
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_BIOEVAL_ACQUISITION_INPUT_BYTES:
            raise ArgumentError("bioeval acquisition input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalAcquisitionAuditArgs":
        raw = _route_mapping("bioeval acquisition arguments", value)
        reference = raw.get("reference_policy")
        return cls(
            tuple(BioevalAcquisitionObligationArgs.from_wire(item) for item in _array("bioeval acquisition obligations", raw.get("obligations"))),
            tuple(BioevalAcquisitionActionArgs.from_wire(item) for item in _array("bioeval acquisition actions", raw.get("actions"))),
            raw.get("stopped_after", False),
            None if reference is None else BioevalAcquisitionReferencePolicyArgs.from_wire(reference),
            raw.get("require_reference", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "obligations": [item.to_wire() for item in self.obligations],
            "actions": [item.to_wire() for item in self.actions],
            "stopped_after": self.stopped_after,
            "require_reference": self.require_reference,
        }
        if self.reference_policy is not None:
            result["reference_policy"] = self.reference_policy.to_wire()
        return result


@dataclass(frozen=True)
class BioevalAcquisitionAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    status: str | None
    stopped_after: bool | None
    admissible: bool | None
    obligations: tuple[Mapping[str, Any], ...]
    open_obligations: tuple[Mapping[str, Any], ...]
    actions: tuple[Mapping[str, Any], ...]
    cost: int | None
    cost_by_kind: tuple[Mapping[str, Any], ...]
    findings: Mapping[str, Any] | None
    reference_policy: Mapping[str, Any] | None
    regret: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalAcquisitionAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("bioeval acquisition refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), None, None, None, (), (), (), None, (), None, None, None, _route_text("bioeval acquisition refusal stage", raw.get("stage")), _route_text("bioeval acquisition refusal", raw.get("refusal")), _route_strings("bioeval acquisition refusal guarantees", raw.get("guarantees", [])), _route_strings("bioeval acquisition refusal limitations", raw.get("limitations", [])), True)
        if raw.get("ok") is not True or raw.get("schema") != BIOEVAL_ACQUISITION_SCHEMA:
            raise ArgumentError("bioeval acquisition projection has an invalid schema")
        obligations = tuple(_route_mapping("bioeval acquisition obligation row", item) for item in _array("bioeval acquisition obligations", raw.get("obligations", [])))
        open_obligations = tuple(_route_mapping("bioeval acquisition open obligation", item) for item in _array("bioeval acquisition open obligations", raw.get("open_obligations", [])))
        actions = tuple(_route_mapping("bioeval acquisition action row", item) for item in _array("bioeval acquisition actions", raw.get("actions", [])))
        cost_by_kind = tuple(_route_mapping("bioeval acquisition cost row", item) for item in _array("bioeval acquisition cost by kind", raw.get("cost_by_kind", [])))
        reference = None if raw.get("reference_policy") is None else _route_mapping("bioeval acquisition reference policy", raw.get("reference_policy"))
        regret = None if raw.get("regret") is None else _route_mapping("bioeval acquisition regret", raw.get("regret"))
        return cls(raw, True, BIOEVAL_ACQUISITION_SCHEMA, _route_text("bioeval acquisition status", raw.get("status")), raw.get("stopped_after"), raw.get("admissible"), obligations, open_obligations, actions, _route_count("bioeval acquisition cost", raw.get("cost")), cost_by_kind, _route_mapping("bioeval acquisition findings", raw.get("findings")), reference, regret, None, None, _route_strings("bioeval acquisition guarantees", raw.get("guarantees", [])), _route_strings("bioeval acquisition limitations", raw.get("limitations", [])), False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def like_for_like(self) -> bool | None:
        return None if self.regret is None else self.regret.get("like_for_like")

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioeval_acquisition_audit_report(value: Mapping[str, Any]) -> BioevalAcquisitionAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BioevalAcquisitionAuditReport.from_wire(value)


__all__ = [
    "BIOEVAL_ACQUISITION_SCHEMA",
    "BIOEVAL_ACQUISITION_KINDS",
    "MAX_BIOEVAL_ACQUISITION_ROWS",
    "MAX_BIOEVAL_ACQUISITION_INPUT_BYTES",
    "BioevalAcquisitionObligationArgs",
    "BioevalAcquisitionActionArgs",
    "BioevalAcquisitionReferencePolicyArgs",
    "BioevalAcquisitionAuditArgs",
    "BioevalAcquisitionAuditReport",
    "bioeval_acquisition_audit_report",
]
