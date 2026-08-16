"""Typed claim-evidence grounding audits for the bioevaluation kernel."""

from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import datetime
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BIOEVAL_GROUNDING_SCHEMA = "bioprism-mcp/bioeval-grounding-audit/0.1"
BIOEVAL_GROUNDING_EDGE_KINDS = frozenset({"supports", "contradicts", "adjacent"})
BIOEVAL_GROUNDING_LOCATORS = frozenset({"resolved", "not_checked", "unresolvable"})
MAX_BIOEVAL_GROUNDING_ROWS = 4096
MAX_BIOEVAL_GROUNDING_INPUT_BYTES = 20_000_000
MAX_BIOEVAL_GROUNDING_OUTPUT_ITEMS = 1000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _identifier(name: str, value: Any) -> str:
    identifier = _route_text(name, value)
    if not identifier.strip() or len(identifier.encode("utf-8")) > 256:
        raise ArgumentError(f"{name} must contain 1 to 256 UTF-8 bytes")
    return identifier


def _timestamp(name: str, value: Any) -> str:
    timestamp = _route_text(name, value)
    if not timestamp.strip() or len(timestamp.encode("utf-8")) > 128:
        raise ArgumentError(f"{name} must be non-empty RFC-3339 text")
    try:
        datetime.fromisoformat(timestamp.replace("Z", "+00:00"))
    except ValueError as error:
        raise ArgumentError(f"{name} must be valid RFC-3339 text") from error
    return timestamp


def _locator(name: str, value: Mapping[str, Any] | None) -> dict[str, Any]:
    raw = {"locator": "not_checked"} if value is None else _route_mapping(name, value)
    state = _route_text(f"{name}.locator", raw.get("locator"))
    if state not in BIOEVAL_GROUNDING_LOCATORS:
        raise ArgumentError(f"{name}.locator is not recognized")
    if state == "resolved" and not _identifier(f"{name}.digest", raw.get("digest")):
        raise ArgumentError(f"{name}.digest must be non-empty when locator is resolved")
    if state == "unresolvable" and not _route_text(f"{name}.detail", raw.get("detail")).strip():
        raise ArgumentError(f"{name}.detail must be non-empty when locator is unresolvable")
    return dict(raw)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("bioeval grounding response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BIOEVAL_GROUNDING_SCHEMA and isinstance(candidate.get("census"), Mapping)
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
                        raise ArgumentError(f"bioeval grounding response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a bioeval grounding projection")


@dataclass(frozen=True)
class BioevalGroundingClaimArgs:
    id: str

    def __post_init__(self) -> None:
        object.__setattr__(self, "id", _identifier("bioeval grounding claim id", self.id))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalGroundingClaimArgs":
        raw = _route_mapping("bioeval grounding claim", value)
        return cls(_identifier("bioeval grounding claim id", raw.get("id")))

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id}


@dataclass(frozen=True)
class BioevalGroundingEvidenceArgs:
    id: str
    last_modified: str
    lineage: tuple[str, ...] = ()
    locator_status: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        identifier = _identifier("bioeval grounding evidence id", self.id)
        last_modified = _timestamp("bioeval grounding evidence last_modified", self.last_modified)
        lineage = tuple(_identifier(f"bioeval grounding lineage[{index}]", item) for index, item in enumerate(self.lineage))
        if len(lineage) > MAX_BIOEVAL_GROUNDING_ROWS:
            raise ArgumentError("bioeval grounding evidence lineage is bounded at 4096 entries")
        locator_status = _locator("bioeval grounding locator_status", self.locator_status)
        object.__setattr__(self, "id", identifier)
        object.__setattr__(self, "last_modified", last_modified)
        object.__setattr__(self, "lineage", lineage)
        object.__setattr__(self, "locator_status", locator_status)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalGroundingEvidenceArgs":
        raw = _route_mapping("bioeval grounding evidence", value)
        return cls(
            _identifier("bioeval grounding evidence id", raw.get("id")),
            _timestamp("bioeval grounding evidence last_modified", raw.get("last_modified")),
            tuple(_identifier(f"bioeval grounding lineage[{index}]", item) for index, item in enumerate(_array("bioeval grounding evidence lineage", raw.get("lineage", [])))),
            None if raw.get("locator_status") is None else _route_mapping("bioeval grounding locator_status", raw.get("locator_status")),
        )

    def to_wire(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "last_modified": self.last_modified,
            "lineage": list(self.lineage),
            "locator_status": dict(self.locator_status or {"locator": "not_checked"}),
        }


@dataclass(frozen=True)
class BioevalGroundingEdgeArgs:
    claim: str
    evidence: str
    kind: str

    def __post_init__(self) -> None:
        claim = _identifier("bioeval grounding edge claim", self.claim)
        evidence = _identifier("bioeval grounding edge evidence", self.evidence)
        kind = _route_text("bioeval grounding edge kind", self.kind)
        if kind not in BIOEVAL_GROUNDING_EDGE_KINDS:
            raise ArgumentError("bioeval grounding edge kind must be supports, contradicts, or adjacent")
        object.__setattr__(self, "claim", claim)
        object.__setattr__(self, "evidence", evidence)
        object.__setattr__(self, "kind", kind)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalGroundingEdgeArgs":
        raw = _route_mapping("bioeval grounding edge", value)
        return cls(
            _identifier("bioeval grounding edge claim", raw.get("claim")),
            _identifier("bioeval grounding edge evidence", raw.get("evidence")),
            _route_text("bioeval grounding edge kind", raw.get("kind")),
        )

    def to_wire(self) -> dict[str, Any]:
        return {"claim": self.claim, "evidence": self.evidence, "kind": self.kind}


@dataclass(frozen=True)
class BioevalGroundingAuditArgs:
    claims: tuple[BioevalGroundingClaimArgs, ...]
    evidence: tuple[BioevalGroundingEvidenceArgs, ...]
    edges: tuple[BioevalGroundingEdgeArgs, ...]
    stale_against: str | None = None
    max_items: int = 100

    def __post_init__(self) -> None:
        claims = tuple(item if isinstance(item, BioevalGroundingClaimArgs) else BioevalGroundingClaimArgs.from_wire(item) for item in self.claims)
        evidence = tuple(item if isinstance(item, BioevalGroundingEvidenceArgs) else BioevalGroundingEvidenceArgs.from_wire(item) for item in self.evidence)
        edges = tuple(item if isinstance(item, BioevalGroundingEdgeArgs) else BioevalGroundingEdgeArgs.from_wire(item) for item in self.edges)
        if any(len(items) > MAX_BIOEVAL_GROUNDING_ROWS for items in (claims, evidence, edges)):
            raise ArgumentError("bioeval grounding claims, evidence, and edges are each bounded at 4096 rows")
        if len({item.id for item in claims}) != len(claims):
            raise ArgumentError("bioeval grounding claim ids must be unique")
        if len({item.id for item in evidence}) != len(evidence):
            raise ArgumentError("bioeval grounding evidence ids must be unique")
        if self.stale_against is not None:
            object.__setattr__(self, "stale_against", _timestamp("bioeval grounding stale_against", self.stale_against))
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_BIOEVAL_GROUNDING_OUTPUT_ITEMS:
            raise ArgumentError("bioeval grounding max_items must be between 1 and 1000")
        object.__setattr__(self, "claims", claims)
        object.__setattr__(self, "evidence", evidence)
        object.__setattr__(self, "edges", edges)
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_BIOEVAL_GROUNDING_INPUT_BYTES:
            raise ArgumentError("bioeval grounding input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalGroundingAuditArgs":
        raw = _route_mapping("bioeval grounding arguments", value)
        return cls(
            tuple(BioevalGroundingClaimArgs.from_wire(item) for item in _array("bioeval grounding claims", raw.get("claims"))),
            tuple(BioevalGroundingEvidenceArgs.from_wire(item) for item in _array("bioeval grounding evidence", raw.get("evidence"))),
            tuple(BioevalGroundingEdgeArgs.from_wire(item) for item in _array("bioeval grounding edges", raw.get("edges"))),
            None if raw.get("stale_against") is None else _timestamp("bioeval grounding stale_against", raw.get("stale_against")),
            raw.get("max_items", 100),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "claims": [item.to_wire() for item in self.claims],
            "evidence": [item.to_wire() for item in self.evidence],
            "edges": [item.to_wire() for item in self.edges],
            "max_items": self.max_items,
        }
        if self.stale_against is not None:
            result["stale_against"] = self.stale_against
        return result


@dataclass(frozen=True)
class BioevalGroundingAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    claims: Mapping[str, Any] | None
    evidence: Mapping[str, Any] | None
    edges: Mapping[str, Any] | None
    census: Mapping[str, Any] | None
    graph: Mapping[str, Any] | None
    locator_census: Mapping[str, Any] | None
    staleness: Mapping[str, Any] | None
    findings: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalGroundingAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("bioeval grounding refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), None, None, None, None, None, None, None, None, _route_text("bioeval grounding refusal stage", raw.get("stage")), _route_text("bioeval grounding refusal", raw.get("refusal")), _route_strings("bioeval grounding refusal guarantees", raw.get("guarantees", [])), _route_strings("bioeval grounding refusal limitations", raw.get("limitations", [])), True)
        if raw.get("ok") is not True or raw.get("schema") != BIOEVAL_GROUNDING_SCHEMA:
            raise ArgumentError("bioeval grounding projection has an invalid schema")
        return cls(
            raw,
            True,
            BIOEVAL_GROUNDING_SCHEMA,
            _route_text("bioeval grounding workflow", raw.get("workflow")),
            _route_mapping("bioeval grounding claims projection", raw.get("claims")),
            _route_mapping("bioeval grounding evidence projection", raw.get("evidence")),
            _route_mapping("bioeval grounding edges projection", raw.get("edges")),
            _route_mapping("bioeval grounding census", raw.get("census")),
            _route_mapping("bioeval grounding graph", raw.get("graph")),
            _route_mapping("bioeval grounding locator census", raw.get("locator_census")),
            _route_mapping("bioeval grounding staleness", raw.get("staleness")),
            _route_mapping("bioeval grounding findings", raw.get("findings")),
            None,
            None,
            _route_strings("bioeval grounding guarantees", raw.get("guarantees", [])),
            _route_strings("bioeval grounding limitations", raw.get("limitations", [])),
            False,
        )

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def fully_grounded(self) -> bool | None:
        return None if self.census is None else self.census.get("fully_grounded")

    @property
    def contested_claims(self) -> tuple[str, ...]:
        if self.findings is None:
            return ()
        rows = self.findings.get("contested_claims")
        if not isinstance(rows, Mapping):
            return ()
        values = rows.get("ids", [])
        return tuple(value for value in values if isinstance(value, str)) if isinstance(values, Sequence) and not isinstance(values, (str, bytes)) else ()

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioeval_grounding_audit_report(value: Mapping[str, Any]) -> BioevalGroundingAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BioevalGroundingAuditReport.from_wire(value)


__all__ = [
    "BIOEVAL_GROUNDING_SCHEMA",
    "BIOEVAL_GROUNDING_EDGE_KINDS",
    "BIOEVAL_GROUNDING_LOCATORS",
    "MAX_BIOEVAL_GROUNDING_ROWS",
    "MAX_BIOEVAL_GROUNDING_INPUT_BYTES",
    "MAX_BIOEVAL_GROUNDING_OUTPUT_ITEMS",
    "BioevalGroundingClaimArgs",
    "BioevalGroundingEvidenceArgs",
    "BioevalGroundingEdgeArgs",
    "BioevalGroundingAuditArgs",
    "BioevalGroundingAuditReport",
    "bioeval_grounding_audit_report",
]
