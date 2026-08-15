"""Typed epistemic provenance and world-claim boundary reports.

The world-factory kernel distinguishes observed, semi-synthetic, and mechanistic evidence.  The
SDK must preserve that ladder and the difference between a grounded claim and a structured refusal;
it must never turn a simulator result into biological support or turn a refusal into a transport
failure with no inspectable reason.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


WORLD_RUNGS = frozenset({"observed", "semi_synthetic", "mechanistic"})
WORLD_CLAIM_KINDS = frozenset({
    "the_world_as_built",
    "detecting_injected_structure",
    "simulator_behaviour",
    "biology",
})
WORLD_SELECTION_KINDS = frozenset({"consecutive", "convenience", "enriched", "undeclared"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _array_of_mappings(name: str, value: Any) -> tuple[dict[str, Any], ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array of objects")
    return tuple(_route_mapping(f"{name}[{index}]", item) for index, item in enumerate(value))


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    required = ("ok", "supported", "claim", "provenance")
    raw = _route_mapping("world claim response", value)
    if all(key in raw for key in required):
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
            if isinstance(structured, Mapping) and all(key in structured for key in required):
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
                    raise ArgumentError(f"world claim response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping("decoded world claim response", decoded)
                if all(key in decoded_mapping for key in required):
                    return decoded_mapping
    raise ArgumentError("response does not contain a world claim projection")


@dataclass(frozen=True)
class WorldSelectionReport:
    raw: dict[str, Any]
    kind: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorldSelectionReport":
        raw = _route_mapping("world selection", value)
        kind = _route_text("world selection kind", raw.get("selection"))
        if kind not in WORLD_SELECTION_KINDS:
            raise ArgumentError(f"unknown world selection kind: {kind!r}")
        required = {
            "consecutive": ("criterion",),
            "convenience": ("because",),
            "enriched": ("for_what",),
            "undeclared": (),
        }[kind]
        for field_name in required:
            _route_text(f"world selection {field_name}", raw.get(field_name))
        return cls(raw, kind)


@dataclass(frozen=True)
class WorldProvenanceReport:
    raw: dict[str, Any]
    top: str
    stands_on: tuple[str, ...]
    assumptions: tuple[str, ...]
    unsupported_counterfactuals: tuple[str, ...]
    selection: WorldSelectionReport

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorldProvenanceReport":
        raw = _route_mapping("world provenance", value)
        top = _route_text("world provenance top", raw.get("top"))
        if top not in WORLD_RUNGS:
            raise ArgumentError(f"unknown world provenance top rung: {top!r}")
        stands_on = _route_strings("world provenance stands_on", raw.get("stands_on"))
        if not stands_on or any(rung not in WORLD_RUNGS for rung in stands_on):
            raise ArgumentError("world provenance stands_on contains an unknown or empty rung set")
        if top not in stands_on:
            raise ArgumentError("world provenance top rung must be present in stands_on")
        return cls(
            raw,
            top,
            stands_on,
            _route_strings("world provenance assumptions", raw.get("assumptions")),
            _route_strings(
                "world provenance unsupported_counterfactuals",
                raw.get("unsupported_counterfactuals"),
            ),
            WorldSelectionReport.from_wire(raw.get("selection")),
        )

    @property
    def observed_only(self) -> bool:
        return self.stands_on == ("observed",)

    @property
    def construction_distance(self) -> int:
        return max({"observed": 0, "semi_synthetic": 1, "mechanistic": 2}[rung] for rung in self.stands_on)


@dataclass(frozen=True)
class WorldClaimReport:
    raw: dict[str, Any]
    kind: str
    quantity: str
    counterfactual: str | None
    population: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorldClaimReport":
        raw = _route_mapping("world claim", value)
        kind = _route_text("world claim kind", raw.get("kind"))
        if kind not in WORLD_CLAIM_KINDS:
            raise ArgumentError(f"unknown world claim kind: {kind!r}")
        return cls(
            raw,
            kind,
            _route_text("world claim quantity", raw.get("quantity")),
            _optional_text("world claim counterfactual", raw.get("counterfactual")),
            _optional_text("world claim population", raw.get("population")),
        )


@dataclass(frozen=True)
class GroundedWorldClaimReport:
    raw: dict[str, Any]
    claim: WorldClaimReport
    stands_on: tuple[str, ...]
    furthest_from_observation: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "GroundedWorldClaimReport":
        raw = _route_mapping("grounded world claim", value)
        stands_on = _route_strings("grounded world claim stands_on", raw.get("stands_on"))
        if not stands_on or any(rung not in WORLD_RUNGS for rung in stands_on):
            raise ArgumentError("grounded world claim stands_on contains an unknown or empty rung set")
        furthest = _route_text("grounded world claim furthest_from_observation", raw.get("furthest_from_observation"))
        if furthest not in WORLD_RUNGS:
            raise ArgumentError(f"unknown grounded world claim furthest rung: {furthest!r}")
        return cls(raw, WorldClaimReport.from_wire(raw.get("claim")), stands_on, furthest)


@dataclass(frozen=True)
class WorldClaimCheckReport:
    """Success or structured refusal for one provenance-limited claim."""

    raw: dict[str, Any]
    ok: bool
    supported: bool
    claim: WorldClaimReport
    provenance: WorldProvenanceReport
    grounded: GroundedWorldClaimReport | None
    caveat: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorldClaimCheckReport":
        raw = _payload(value)
        ok = _bool("world claim ok", raw.get("ok"))
        supported = _bool("world claim supported", raw.get("supported"))
        if ok != supported:
            raise ArgumentError("world claim ok and supported must have parity")
        claim = WorldClaimReport.from_wire(raw.get("claim"))
        provenance = WorldProvenanceReport.from_wire(raw.get("provenance"))
        grounded_value = raw.get("grounded")
        grounded = None if grounded_value is None else GroundedWorldClaimReport.from_wire(grounded_value)
        caveat = _optional_text("world claim caveat", raw.get("caveat"))
        refusal = _optional_text("world claim refusal", raw.get("refusal"))
        if supported:
            if grounded is None or caveat is None or refusal is not None:
                raise ArgumentError("supported world claims require grounded evidence and a caveat")
            if grounded.claim.raw != claim.raw:
                raise ArgumentError("grounded world claim does not preserve the requested claim")
            fail_closed = False if raw.get("fail_closed") is None else _bool("world claim fail_closed", raw.get("fail_closed"))
            if fail_closed:
                raise ArgumentError("supported world claims cannot be fail-closed refusals")
        else:
            if grounded is not None or caveat is not None or refusal is None:
                raise ArgumentError("refused world claims require a refusal and no grounded claim")
            fail_closed = _bool("world claim fail_closed", raw.get("fail_closed"))
            if not fail_closed:
                raise ArgumentError("refused world claims must remain fail-closed")
        return cls(raw, ok, supported, claim, provenance, grounded, caveat, refusal, fail_closed)

    @property
    def refused(self) -> bool:
        return not self.supported


def world_claim_check_report(value: Mapping[str, Any]) -> WorldClaimCheckReport:
    """Parse direct MCP or HTTP world-claim output, including structured refusals."""

    return WorldClaimCheckReport.from_wire(value)


__all__ = [
    "WORLD_CLAIM_KINDS",
    "WORLD_RUNGS",
    "WORLD_SELECTION_KINDS",
    "GroundedWorldClaimReport",
    "WorldClaimCheckReport",
    "WorldClaimReport",
    "WorldProvenanceReport",
    "WorldSelectionReport",
    "world_claim_check_report",
]
