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

from .capability import _route_count, _route_mapping, _route_strings, _route_text
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


@dataclass(frozen=True)
class ObservedWorldDeclareArgs:
    """Pinned observed-world declaration request."""

    id: str
    sources: tuple[Mapping[str, Any], ...]
    design: Mapping[str, Any]
    outcome_labels: tuple[str, ...]

    def __init__(
        self,
        id: str,
        sources: Sequence[Mapping[str, Any]],
        design: Mapping[str, Any],
        outcome_labels: Sequence[str],
    ) -> None:
        world_id = _route_text("observed world id", id)
        normalized_sources = _array_of_mappings("observed world sources", sources)
        source_names = tuple(_route_text("observed world source name", source.get("name")) for source in normalized_sources)
        if len(source_names) != len(set(source_names)):
            raise ArgumentError("observed world sources must have unique names")
        normalized_design = _route_mapping("observed world design", design)
        if not isinstance(outcome_labels, Sequence) or isinstance(outcome_labels, (str, bytes)):
            raise ArgumentError("observed world outcome_labels must be an array of strings")
        labels = tuple(_route_text(f"observed world outcome_labels[{index}]", label) for index, label in enumerate(outcome_labels))
        if len(labels) != len(set(labels)):
            raise ArgumentError("observed world outcome_labels must be unique")
        object.__setattr__(self, "id", world_id)
        object.__setattr__(self, "sources", normalized_sources)
        object.__setattr__(self, "design", normalized_design)
        object.__setattr__(self, "outcome_labels", labels)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ObservedWorldDeclareArgs":
        raw = _route_mapping("observed world declaration arguments", value)
        return cls(raw.get("id"), raw.get("sources"), raw.get("design"), raw.get("outcome_labels"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "sources": [dict(source) for source in self.sources],
            "design": dict(self.design),
            "outcome_labels": list(self.outcome_labels),
        }


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


def _payload_for_keys(
    value: Mapping[str, Any],
    required: tuple[str, ...],
    label: str,
) -> dict[str, Any]:
    raw = _route_mapping(f"{label} response", value)
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
                    raise ArgumentError(f"{label} response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping(f"decoded {label} response", decoded)
                if all(key in decoded_mapping for key in required):
                    return decoded_mapping
    raise ArgumentError(f"response does not contain a {label} projection")


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
class WorldSourceReport:
    raw: dict[str, Any]
    name: str
    version: str | None
    access_kind: str
    access_policy: str | None
    embedded: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorldSourceReport":
        raw = _route_mapping("observed world source", value)
        access = _route_mapping("observed world source access", raw.get("access"))
        access_kind = _route_text("observed world source access kind", access.get("access"))
        if access_kind not in {"public", "controlled"}:
            raise ArgumentError(f"unknown observed world access kind: {access_kind!r}")
        policy = access.get("policy")
        access_policy = None if policy is None else _route_text("observed world source access policy", policy)
        if access_kind == "controlled" and access_policy is None:
            raise ArgumentError("controlled observed world sources require an access policy")
        if access_kind == "public" and access_policy is not None:
            raise ArgumentError("public observed world sources cannot carry a control policy")
        embedded = raw.get("embedded", False)
        if not isinstance(embedded, bool):
            raise ArgumentError("observed world source embedded must be a boolean")
        return cls(
            raw,
            _route_text("observed world source name", raw.get("name")),
            _optional_text("observed world source version", raw.get("version")),
            access_kind,
            access_policy,
            embedded,
        )

    @property
    def controlled(self) -> bool:
        return self.access_kind == "controlled"


@dataclass(frozen=True)
class WorldStratumReport:
    raw: dict[str, Any]
    name: str
    count: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorldStratumReport":
        raw = _route_mapping("observed world stratum", value)
        return cls(
            raw,
            _route_text("observed world stratum name", raw.get("name")),
            _route_count("observed world stratum count", raw.get("count")),
        )


@dataclass(frozen=True)
class WorldStudyDesignReport:
    raw: dict[str, Any]
    cohort_size: int
    strata: tuple[WorldStratumReport, ...]
    selection: WorldSelectionReport
    stands_for_population: str | None
    unsupported_counterfactuals: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorldStudyDesignReport":
        raw = _route_mapping("observed world study design", value)
        strata = tuple(
            WorldStratumReport.from_wire(item)
            for item in _array_of_mappings("observed world design strata", raw.get("strata", []))
        )
        cohort_size = _route_count("observed world cohort_size", raw.get("cohort_size"))
        if strata and sum(stratum.count for stratum in strata) != cohort_size:
            raise ArgumentError("observed world strata do not reconcile with cohort_size")
        return cls(
            raw,
            cohort_size,
            strata,
            WorldSelectionReport.from_wire(raw.get("selection")),
            _optional_text("observed world stands_for_population", raw.get("stands_for_population")),
            _route_strings(
                "observed world design unsupported_counterfactuals",
                raw.get("unsupported_counterfactuals", []),
            ),
        )


@dataclass(frozen=True)
class ObservedWorldReport:
    raw: dict[str, Any]
    id: str
    sources: tuple[WorldSourceReport, ...]
    design: WorldStudyDesignReport
    outcome_labels: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ObservedWorldReport":
        raw = _route_mapping("observed world", value)
        sources = tuple(
            WorldSourceReport.from_wire(item)
            for item in _array_of_mappings("observed world sources", raw.get("sources"))
        )
        names = tuple(source.name for source in sources)
        if len(names) != len(set(names)):
            raise ArgumentError("observed world sources must have unique names")
        return cls(
            raw,
            _route_text("observed world id", raw.get("id")),
            sources,
            WorldStudyDesignReport.from_wire(raw.get("design")),
            _route_strings("observed world outcome_labels", raw.get("outcome_labels")),
        )

    @property
    def controlled_sources(self) -> tuple[str, ...]:
        return tuple(source.name for source in self.sources if source.controlled)


@dataclass(frozen=True)
class ObservedWorldDeclareReport:
    raw: dict[str, Any]
    ok: bool
    world: ObservedWorldReport
    provenance: WorldProvenanceReport
    world_id: str
    source_count: int
    controlled_sources: tuple[str, ...]
    outcome_label_count: int
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ObservedWorldDeclareReport":
        raw = _payload_for_keys(
            value,
            (
                "ok",
                "world",
                "provenance",
                "world_id",
                "source_count",
                "controlled_sources",
                "outcome_label_count",
                "guarantees",
            ),
            "observed world declaration",
        )
        if not _bool("observed world declaration ok", raw.get("ok")):
            raise ArgumentError("observed world declaration is not successful")
        world = ObservedWorldReport.from_wire(raw.get("world"))
        provenance = WorldProvenanceReport.from_wire(raw.get("provenance"))
        world_id = _route_text("observed world declaration world_id", raw.get("world_id"))
        source_count = _route_count("observed world declaration source_count", raw.get("source_count"))
        controlled_sources = _route_strings("observed world declaration controlled_sources", raw.get("controlled_sources"))
        outcome_label_count = _route_count("observed world declaration outcome_label_count", raw.get("outcome_label_count"))
        if world_id != world.id or source_count != len(world.sources) or outcome_label_count != len(world.outcome_labels):
            raise ArgumentError("observed world declaration counts do not reconcile with world")
        if controlled_sources != world.controlled_sources:
            raise ArgumentError("observed world declaration controlled_sources do not reconcile")
        if provenance.top != "observed" or provenance.stands_on != ("observed",):
            raise ArgumentError("observed world declaration provenance must be observed-only")
        if provenance.selection.kind != world.design.selection.kind:
            raise ArgumentError("observed world provenance selection does not match the study design")
        if provenance.unsupported_counterfactuals != world.design.unsupported_counterfactuals:
            raise ArgumentError("observed world provenance counterfactuals do not match the study design")
        return cls(
            raw,
            True,
            world,
            provenance,
            world_id,
            source_count,
            controlled_sources,
            outcome_label_count,
            _route_strings("observed world declaration guarantees", raw.get("guarantees")),
        )


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


def observed_world_declare_report(value: Mapping[str, Any]) -> ObservedWorldDeclareReport:
    """Parse direct MCP or HTTP observed-world declaration output."""

    return ObservedWorldDeclareReport.from_wire(value)


__all__ = [
    "WORLD_CLAIM_KINDS",
    "WORLD_RUNGS",
    "WORLD_SELECTION_KINDS",
    "GroundedWorldClaimReport",
    "ObservedWorldDeclareArgs",
    "ObservedWorldDeclareReport",
    "ObservedWorldReport",
    "WorldClaimCheckReport",
    "WorldClaimReport",
    "WorldProvenanceReport",
    "WorldSelectionReport",
    "WorldSourceReport",
    "WorldStratumReport",
    "WorldStudyDesignReport",
    "observed_world_declare_report",
    "world_claim_check_report",
]
