"""Typed leaderboard and composed BioAtlas publication-audit projections.

The public hub has several independent gates: comparability, moderation acceptance, verification
floor, evidence scale, disclosure eligibility, atlas coverage, evidence-conditioned claims, and an
explicit release request.  This module keeps the result of each gate visible.  It deliberately
does not turn a ranked count into scientific truth or a ready release target into network
publication.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError
from .hub_card import HubCardLabelReport, HubCardRenderReport, hub_card_render


HUB_LEADERBOARD_SCHEMA = "bioprism-mcp/hub-leaderboard/0.1"
BIOATLAS_PUBLICATION_SCHEMA = "bioprism-mcp/bioatlas-publication-audit/0.1"
HUB_LEADERBOARD_MAX_ENTRIES = 2_000
BIOATLAS_PUBLICATION_MAX_INPUT_BYTES = 20_000_000
BIOATLAS_PUBLICATION_MAX_ITEMS = 1_000
BIOATLAS_PUBLICATION_MAX_TARGETS = 16
HUB_UNRANKABLE_REASONS = frozenset({"not_comparable", "not_published", "below_verification_floor", "ineligible"})
BIOATLAS_RELEASE_TARGETS = frozenset({"atlas_profile", "atlas_aggregation", "evidence_claims", "card_render", "numeric_card_score", "leaderboard", "ranked_leaderboard"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _envelope(value: Mapping[str, Any], description: str, schema: str, required: tuple[str, ...]) -> dict[str, Any]:
    raw = _route_mapping(description, value)
    candidates: list[Mapping[str, Any]] = [raw]

    def add_container(container: Any) -> None:
        if not isinstance(container, Mapping):
            return
        candidates.append(container)
        nested = container.get("result")
        if isinstance(nested, Mapping):
            candidates.append(nested)
            structured = nested.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = nested.get("content")
            if isinstance(content, list):
                for block in content:
                    if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                        try:
                            decoded = json.loads(block["text"])
                        except json.JSONDecodeError as error:
                            raise ArgumentError(f"{description} text is not JSON: {error}") from error
                        if isinstance(decoded, Mapping):
                            candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("schema") == schema and all(field in candidate for field in required):
            return dict(candidate)
    raise ArgumentError(f"response does not contain a {description} projection")


def _bool_route(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


@dataclass(frozen=True)
class HubLeaderboardRenderArgs:
    board: dict[str, Any]
    entries: tuple[Any, ...]
    moderation: dict[str, Any]
    disclosure: dict[str, Any]
    include_details: bool = False

    def __init__(self, board: Mapping[str, Any], entries: Sequence[Any], moderation: Mapping[str, Any], disclosure: Mapping[str, Any], include_details: bool = False) -> None:
        normalized_entries = _sequence("hub leaderboard entries", entries)
        if len(normalized_entries) > HUB_LEADERBOARD_MAX_ENTRIES:
            raise ArgumentError("hub leaderboard entries must contain at most 2000 rows")
        normalized_board = _route_mapping("hub leaderboard board", board)
        normalized_moderation = _route_mapping("hub leaderboard moderation", moderation)
        normalized_disclosure = _route_mapping("hub leaderboard disclosure", disclosure)
        normalized_details = _bool("hub leaderboard include_details", include_details)
        object.__setattr__(self, "board", normalized_board)
        object.__setattr__(self, "entries", normalized_entries)
        object.__setattr__(self, "moderation", normalized_moderation)
        object.__setattr__(self, "disclosure", normalized_disclosure)
        object.__setattr__(self, "include_details", normalized_details)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubLeaderboardRenderArgs":
        raw = _route_mapping("hub leaderboard arguments", value)
        return cls(raw.get("board"), raw.get("entries"), raw.get("moderation"), raw.get("disclosure"), raw.get("include_details", False))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"board": dict(self.board), "entries": list(self.entries), "moderation": dict(self.moderation), "disclosure": dict(self.disclosure), "include_details": self.include_details}


@dataclass(frozen=True)
class HubUnrankableReasonReport:
    raw: dict[str, Any]
    kind: str
    differences: tuple[dict[str, Any], ...]
    state: str | None
    has_verification: str | None
    verification_floor: str | None
    detail: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubUnrankableReasonReport":
        raw = _route_mapping("hub unrankable reason", value)
        kind = _route_text("hub unrankable reason tag", raw.get("reason"))
        if kind not in HUB_UNRANKABLE_REASONS:
            raise ArgumentError(f"unknown hub unrankable reason {kind!r}")
        differences = tuple(_route_mapping("hub condition difference", item) for item in _sequence("hub condition differences", raw.get("differences", [])))
        state = _optional_text("hub unrankable publication state", raw.get("state"))
        has = _optional_text("hub unrankable verification", raw.get("has"))
        floor = _optional_text("hub verification floor", raw.get("floor"))
        detail = _optional_text("hub ineligible detail", raw.get("detail"))
        return cls(raw, kind, differences, state, has, floor, detail)


@dataclass(frozen=True)
class HubRankedEntryReport:
    raw: dict[str, Any]
    rank: int
    entry: dict[str, Any]
    verification: str
    label: HubCardLabelReport

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubRankedEntryReport":
        raw = _route_mapping("hub ranked entry", value)
        return cls(raw, _integer("hub ranked entry rank", raw.get("rank")), _route_mapping("hub ranked entry payload", raw.get("entry")), _route_text("hub ranked verification", raw.get("verification")), HubCardLabelReport.from_wire(raw.get("label")))


@dataclass(frozen=True)
class HubUnrankedEntryReport:
    raw: dict[str, Any]
    entry: dict[str, Any]
    reason: HubUnrankableReasonReport

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubUnrankedEntryReport":
        raw = _route_mapping("hub unranked entry", value)
        return cls(raw, _route_mapping("hub unranked entry payload", raw.get("entry")), HubUnrankableReasonReport.from_wire(raw.get("reason")))


@dataclass(frozen=True)
class HubRankedBoardReport:
    raw: dict[str, Any]
    board: str
    conditions: dict[str, Any]
    ranked: tuple[HubRankedEntryReport, ...]
    unranked: tuple[HubUnrankedEntryReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubRankedBoardReport":
        raw = _route_mapping("hub ranked board", value)
        return cls(raw, _route_text("hub ranked board id", raw.get("board")), _route_mapping("hub ranked board conditions", raw.get("conditions")), tuple(HubRankedEntryReport.from_wire(item) for item in _sequence("hub ranked rows", raw.get("ranked", []))), tuple(HubUnrankedEntryReport.from_wire(item) for item in _sequence("hub unranked rows", raw.get("unranked", []))))


@dataclass(frozen=True)
class HubLeaderboardRenderReport:
    raw: dict[str, Any]
    ok: bool
    schema: str
    board: str
    ranked_count: int
    unranked_count: int
    leader_count: int
    headline: str
    rendered: HubRankedBoardReport | None
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubLeaderboardRenderReport":
        raw = _envelope(value, "hub leaderboard", HUB_LEADERBOARD_SCHEMA, ("ok", "ranked_count", "unranked_count", "headline", "rendered"))
        rendered_raw = raw.get("rendered")
        rendered = None if rendered_raw is None else HubRankedBoardReport.from_wire(rendered_raw)
        ranked_count = _integer("hub leaderboard ranked_count", raw.get("ranked_count"))
        unranked_count = _integer("hub leaderboard unranked_count", raw.get("unranked_count"))
        if rendered is not None and (ranked_count != len(rendered.ranked) or unranked_count != len(rendered.unranked)):
            raise ArgumentError("hub leaderboard counts do not reconcile with rendered details")
        leader_count = _integer("hub leaderboard leader_count", raw.get("leader_count"))
        if leader_count > ranked_count:
            raise ArgumentError("hub leaderboard leaders cannot exceed ranked entries")
        return cls(raw, _bool_route("hub leaderboard ok", raw.get("ok")), _route_text("hub leaderboard schema", raw.get("schema")), _route_text("hub leaderboard board", raw.get("board")), ranked_count, unranked_count, leader_count, _route_text("hub leaderboard headline", raw.get("headline")), rendered, _route_strings("hub leaderboard guarantees", raw.get("guarantees", [])))

    @property
    def details_omitted(self) -> bool:
        return self.rendered is None

    @property
    def has_unranked_entries(self) -> bool:
        return self.unranked_count > 0

    @property
    def all_unranked_reasons_are_typed(self) -> bool:
        return self.rendered is not None and all(row.reason.kind in HUB_UNRANKABLE_REASONS for row in self.rendered.unranked)

    @property
    def headline_has_nonclaims(self) -> bool:
        return "no clinical validity" in self.headline and "outside the stated conditions" in self.headline

    @property
    def rankability_is_gated(self) -> bool:
        return any("checked before an entry is rankable" in item for item in self.guarantees)

    @property
    def unranked_entries_remain_visible(self) -> bool:
        return any("remain visible as unranked reasons" in item for item in self.guarantees)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def hub_leaderboard_render(value: Mapping[str, Any]) -> HubLeaderboardRenderReport:
    return HubLeaderboardRenderReport.from_wire(value)


@dataclass(frozen=True)
class BioAtlasPublicationAuditArgs:
    atlas: dict[str, Any]
    weighting: dict[str, Any] | None = None
    evidence_audit: dict[str, Any] | None = None
    card: dict[str, Any] | None = None
    leaderboard: dict[str, Any] | None = None
    release_request: dict[str, Any] | None = None
    max_items: int = 100

    def __init__(self, atlas: Mapping[str, Any], weighting: Mapping[str, Any] | None = None, evidence_audit: Mapping[str, Any] | None = None, card: Mapping[str, Any] | None = None, leaderboard: Mapping[str, Any] | None = None, release_request: Mapping[str, Any] | None = None, max_items: int = 100) -> None:
        normalized_max = _integer("BioAtlas publication max_items", max_items)
        if not 1 <= normalized_max <= BIOATLAS_PUBLICATION_MAX_ITEMS:
            raise ArgumentError("BioAtlas publication max_items must be between 1 and 1000")
        normalized_atlas = _route_mapping("BioAtlas publication atlas", atlas)
        normalized_weighting = None if weighting is None else _route_mapping("BioAtlas weighting", weighting)
        normalized_evidence = None if evidence_audit is None else _route_mapping("BioAtlas evidence audit", evidence_audit)
        normalized_card = None if card is None else _route_mapping("BioAtlas card", card)
        normalized_leaderboard = None if leaderboard is None else _route_mapping("BioAtlas leaderboard", leaderboard)
        normalized_release = None if release_request is None else _route_mapping("BioAtlas release request", release_request)
        arguments = {"atlas": normalized_atlas, "weighting": normalized_weighting, "evidence_audit": normalized_evidence, "card": normalized_card, "leaderboard": normalized_leaderboard, "release_request": normalized_release, "max_items": normalized_max}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"BioAtlas publication arguments are not JSON serializable: {error}") from error
        if len(encoded) > BIOATLAS_PUBLICATION_MAX_INPUT_BYTES:
            raise ArgumentError("BioAtlas publication input exceeds the 20 MB safety bound")
        object.__setattr__(self, "atlas", normalized_atlas)
        object.__setattr__(self, "weighting", normalized_weighting)
        object.__setattr__(self, "evidence_audit", normalized_evidence)
        object.__setattr__(self, "card", normalized_card)
        object.__setattr__(self, "leaderboard", normalized_leaderboard)
        object.__setattr__(self, "release_request", normalized_release)
        object.__setattr__(self, "max_items", normalized_max)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioAtlasPublicationAuditArgs":
        raw = _route_mapping("BioAtlas publication arguments", value)
        return cls(raw.get("atlas"), raw.get("weighting"), raw.get("evidence_audit"), raw.get("card"), raw.get("leaderboard"), raw.get("release_request"), raw.get("max_items", 100))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"atlas": dict(self.atlas), "max_items": self.max_items}
        for name, value in (("weighting", self.weighting), ("evidence_audit", self.evidence_audit), ("card", self.card), ("leaderboard", self.leaderboard), ("release_request", self.release_request)):
            if value is not None:
                result[name] = dict(value)
        return result


@dataclass(frozen=True)
class BioAtlasReleaseTargetReport:
    raw: dict[str, Any]
    target: str
    eligible: bool
    blockers: tuple[str, ...]
    notes: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioAtlasReleaseTargetReport":
        raw = _route_mapping("BioAtlas release target", value)
        target = _route_text("BioAtlas release target name", raw.get("target"))
        if target not in BIOATLAS_RELEASE_TARGETS:
            raise ArgumentError(f"unknown BioAtlas release target {target!r}")
        return cls(raw, target, _bool_route("BioAtlas release target eligible", raw.get("eligible")), _route_strings("BioAtlas release blockers", raw.get("blockers", [])), _route_strings("BioAtlas release notes", raw.get("notes", [])))


@dataclass(frozen=True)
class BioAtlasReleaseRequestReport:
    raw: dict[str, Any]
    present: bool
    request_id: str | None
    targets: tuple[BioAtlasReleaseTargetReport, ...]
    ready: bool
    fail_closed: bool
    no_implicit_release: bool
    reason: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioAtlasReleaseRequestReport":
        raw = _route_mapping("BioAtlas release request", value)
        present = _bool_route("BioAtlas release request present", raw.get("present"))
        targets = tuple(BioAtlasReleaseTargetReport.from_wire(item) for item in _sequence("BioAtlas release targets", raw.get("targets", [])))
        ready = _bool_route("BioAtlas release ready", raw.get("ready"))
        fail_closed = _bool_route("BioAtlas release fail_closed", raw.get("fail_closed", False))
        no_implicit = _bool_route("BioAtlas no_implicit_release", raw.get("no_implicit_release"))
        request_id = _optional_text("BioAtlas release id", raw.get("id"))
        reason = _optional_text("BioAtlas release reason", raw.get("reason"))
        if present and (request_id is None or not targets or ready != all(target.eligible for target in targets) or fail_closed == ready):
            raise ArgumentError("present BioAtlas release requests do not reconcile")
        if not present and (ready or request_id is not None or targets):
            raise ArgumentError("absent BioAtlas release requests cannot claim readiness or targets")
        return cls(raw, present, request_id, targets, ready, fail_closed, no_implicit, reason)


@dataclass(frozen=True)
class BioAtlasCrossLayerReport:
    raw: dict[str, Any]
    numeric_score_requires_evidence_audit: bool
    numeric_score_evidence_ready: bool
    atlas_aggregation_ready: bool
    leaderboard_ranked_count: int
    leaderboard_unranked_count: int
    unranked_leaderboard_entries_remain_visible: bool
    withheld_scores_are_not_zeroes: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioAtlasCrossLayerReport":
        raw = _route_mapping("BioAtlas cross-layer report", value)
        return cls(raw, _bool_route("BioAtlas numeric score evidence gate", raw.get("numeric_score_requires_evidence_audit")), _bool_route("BioAtlas numeric score evidence ready", raw.get("numeric_score_evidence_ready")), _bool_route("BioAtlas aggregation ready", raw.get("atlas_aggregation_ready")), _integer("BioAtlas ranked count", raw.get("leaderboard_ranked_count")), _integer("BioAtlas unranked count", raw.get("leaderboard_unranked_count")), _bool_route("BioAtlas unranked visibility", raw.get("unranked_leaderboard_entries_remain_visible")), _bool_route("BioAtlas withheld score nonzero", raw.get("withheld_scores_are_not_zeroes")))


@dataclass(frozen=True)
class BioAtlasPublicationAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str
    workflow: str
    atlas: dict[str, Any]
    evidence_audit: dict[str, Any] | None
    card: HubCardRenderReport | None
    leaderboard: HubLeaderboardRenderReport | None
    release_request: BioAtlasReleaseRequestReport
    cross_layer: BioAtlasCrossLayerReport
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioAtlasPublicationAuditReport":
        raw = _envelope(value, "BioAtlas publication audit", BIOATLAS_PUBLICATION_SCHEMA, ("ok", "workflow", "atlas", "release_request", "cross_layer"))
        evidence_raw = raw.get("evidence_audit")
        card_raw = raw.get("card")
        leaderboard_raw = raw.get("leaderboard")
        card = None if card_raw is None else hub_card_render(card_raw)
        leaderboard = None if leaderboard_raw is None else hub_leaderboard_render(leaderboard_raw)
        return cls(raw, _bool_route("BioAtlas publication ok", raw.get("ok")), _route_text("BioAtlas publication schema", raw.get("schema")), _route_text("BioAtlas publication workflow", raw.get("workflow")), _route_mapping("BioAtlas atlas result", raw.get("atlas")), None if evidence_raw is None else _route_mapping("BioAtlas evidence result", evidence_raw), card, leaderboard, BioAtlasReleaseRequestReport.from_wire(raw.get("release_request")), BioAtlasCrossLayerReport.from_wire(raw.get("cross_layer")), _route_strings("BioAtlas publication guarantees", raw.get("guarantees", [])), _route_strings("BioAtlas publication limitations", raw.get("limitations", [])))

    @property
    def explicit_release_requested(self) -> bool:
        return self.release_request.present

    @property
    def release_ready(self) -> bool:
        return self.release_request.ready

    @property
    def numeric_score_is_conditioned(self) -> bool:
        return self.cross_layer.numeric_score_requires_evidence_audit and self.cross_layer.numeric_score_evidence_ready

    @property
    def unranked_entries_remain_visible(self) -> bool:
        return self.cross_layer.unranked_leaderboard_entries_remain_visible

    @property
    def gates_are_separate(self) -> bool:
        return any("remain distinct gates" in item for item in self.guarantees)

    @property
    def score_withholding_is_explicit(self) -> bool:
        return self.cross_layer.withheld_scores_are_not_zeroes

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioatlas_publication_audit(value: Mapping[str, Any]) -> BioAtlasPublicationAuditReport:
    return BioAtlasPublicationAuditReport.from_wire(value)


__all__ = [
    "HUB_LEADERBOARD_SCHEMA",
    "BIOATLAS_PUBLICATION_SCHEMA",
    "HUB_LEADERBOARD_MAX_ENTRIES",
    "BIOATLAS_PUBLICATION_MAX_INPUT_BYTES",
    "BIOATLAS_PUBLICATION_MAX_ITEMS",
    "BIOATLAS_PUBLICATION_MAX_TARGETS",
    "HUB_UNRANKABLE_REASONS",
    "BIOATLAS_RELEASE_TARGETS",
    "HubLeaderboardRenderArgs",
    "HubUnrankableReasonReport",
    "HubRankedEntryReport",
    "HubUnrankedEntryReport",
    "HubRankedBoardReport",
    "HubLeaderboardRenderReport",
    "hub_leaderboard_render",
    "BioAtlasPublicationAuditArgs",
    "BioAtlasReleaseTargetReport",
    "BioAtlasReleaseRequestReport",
    "BioAtlasCrossLayerReport",
    "BioAtlasPublicationAuditReport",
    "bioatlas_publication_audit",
]
