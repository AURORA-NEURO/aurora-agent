"""Typed projections for the public BioAtlas card renderer.

Cards are intentionally stateful publication objects rather than score dictionaries.  Rust
derives publication state from moderation history, access, verification, withdrawal, dispute,
supersession, and reproduction evidence; it starts every card with a tagged withheld score and
only attaches a number after both the disclosure gate and available-state gate pass.  This module
validates and exposes that shape without reimplementing those predicates in Python.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


HUB_CARD_SCHEMA = "bioprism-mcp/hub-card/0.1"
HUB_CARD_MAX_INPUT_BYTES = 20_000_000
HUB_CARD_STATES = frozenset({"available", "unavailable", "controlled", "stale", "under-review", "disputed", "withdrawn", "non-reproducible", "not-comparable"})
HUB_CARD_SCORE_DISPLAYS = frozenset({"published", "withheld"})
HUB_CARD_LABELS = frozenset({"held_out", "computed_before_disclosure", "disclosed_pack"})
HUB_CARD_VERIFICATION = frozenset({"self-reported", "reproduced", "verified", "prospectively-validated"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("hub card response", value)
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
                            raise ArgumentError(f"hub card response text is not JSON: {error}") from error
                        if isinstance(decoded, Mapping):
                            candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("ok") is not None and isinstance(candidate.get("card"), Mapping) and "score" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain a hub card projection")


@dataclass(frozen=True)
class HubCardRenderArgs:
    """Serialized moderation/card inputs accepted by the hub renderer."""

    moderation: dict[str, Any]
    submission: str
    version: str = "bioatlas-card/0.1"
    score: dict[str, Any] | None = None
    pack: str | None = None
    computed_at: int | None = None
    acknowledges_disclosure: bool = False
    disclosure: dict[str, Any] | None = None
    not_comparable: dict[str, Any] | None = None

    def __init__(self, moderation: Mapping[str, Any], submission: str, version: str = "bioatlas-card/0.1", score: Mapping[str, Any] | None = None, pack: str | None = None, computed_at: int | None = None, acknowledges_disclosure: bool = False, disclosure: Mapping[str, Any] | None = None, not_comparable: Mapping[str, Any] | None = None) -> None:
        normalized_moderation = _route_mapping("hub card moderation", moderation)
        normalized_submission = _route_text("hub card submission", submission)
        normalized_version = _route_text("hub card version", version)
        normalized_score = None if score is None else _route_mapping("hub card score", score)
        normalized_pack = _optional_text("hub card pack", pack)
        normalized_at = None if computed_at is None else _integer("hub card computed_at", computed_at)
        normalized_acknowledges = _bool("hub card acknowledges_disclosure", acknowledges_disclosure)
        normalized_disclosure = None if disclosure is None else _route_mapping("hub card disclosure", disclosure)
        normalized_not_comparable = None if not_comparable is None else _route_mapping("hub card not_comparable", not_comparable)
        arguments = {"moderation": normalized_moderation, "submission": normalized_submission, "version": normalized_version, "score": normalized_score, "pack": normalized_pack, "computed_at": normalized_at, "acknowledges_disclosure": normalized_acknowledges, "disclosure": normalized_disclosure, "not_comparable": normalized_not_comparable}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"hub card arguments are not JSON serializable: {error}") from error
        if len(encoded) > HUB_CARD_MAX_INPUT_BYTES:
            raise ArgumentError("hub card input exceeds the 20 MB safety bound")
        object.__setattr__(self, "moderation", normalized_moderation)
        object.__setattr__(self, "submission", normalized_submission)
        object.__setattr__(self, "version", normalized_version)
        object.__setattr__(self, "score", normalized_score)
        object.__setattr__(self, "pack", normalized_pack)
        object.__setattr__(self, "computed_at", normalized_at)
        object.__setattr__(self, "acknowledges_disclosure", normalized_acknowledges)
        object.__setattr__(self, "disclosure", normalized_disclosure)
        object.__setattr__(self, "not_comparable", normalized_not_comparable)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubCardRenderArgs":
        raw = _route_mapping("hub card arguments", value)
        return cls(raw.get("moderation"), raw.get("submission"), raw.get("version", "bioatlas-card/0.1"), raw.get("score"), raw.get("pack"), raw.get("computed_at"), raw.get("acknowledges_disclosure", False), raw.get("disclosure"), raw.get("not_comparable"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"moderation": dict(self.moderation), "submission": self.submission, "version": self.version, "acknowledges_disclosure": self.acknowledges_disclosure}
        for name, value in (("score", self.score), ("pack", self.pack), ("computed_at", self.computed_at), ("disclosure", self.disclosure), ("not_comparable", self.not_comparable)):
            if value is not None:
                result[name] = dict(value) if isinstance(value, Mapping) else value
        return result


@dataclass(frozen=True)
class HubCardLabelReport:
    raw: dict[str, Any]
    kind: str
    disclosed_at: int | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubCardLabelReport":
        raw = _route_mapping("hub card label", value)
        kind = _route_text("hub card label", raw.get("label"))
        if kind not in HUB_CARD_LABELS:
            raise ArgumentError(f"unknown hub card headline label {kind!r}")
        disclosed_at = None if raw.get("disclosed_at") is None else _integer("hub card disclosed_at", raw.get("disclosed_at"))
        if kind != "held_out" and disclosed_at is None:
            raise ArgumentError("disclosed hub card labels must retain disclosed_at")
        return cls(raw, kind, disclosed_at)


@dataclass(frozen=True)
class HubCardScoreReport:
    raw: dict[str, Any]
    display: str
    score: dict[str, Any] | None
    label: HubCardLabelReport | None
    state: str | None
    why: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubCardScoreReport":
        raw = _route_mapping("hub card score display", value)
        display = _route_text("hub card score display tag", raw.get("display"))
        if display not in HUB_CARD_SCORE_DISPLAYS:
            raise ArgumentError(f"unknown hub card score display {display!r}")
        if display == "published":
            score = _route_mapping("hub card published score", raw.get("score"))
            label = HubCardLabelReport.from_wire(raw.get("label"))
            if raw.get("state") is not None or raw.get("why") is not None:
                raise ArgumentError("published hub card scores cannot retain withheld fields")
            return cls(raw, display, score, label, None, None)
        state = _route_text("hub card withheld state", raw.get("state"))
        if state not in HUB_CARD_STATES:
            raise ArgumentError(f"unknown hub card withheld state {state!r}")
        return cls(raw, display, None, None, state, _route_text("hub card withheld reason", raw.get("why")))

    @property
    def numeric_value(self) -> float | None:
        if self.score is None or self.score.get("value") is None:
            return None
        value = self.score.get("value")
        if not isinstance(value, (int, float)) or isinstance(value, bool):
            raise ArgumentError("hub card score value must be numeric")
        return float(value)


@dataclass(frozen=True)
class HubCardObjectReport:
    raw: dict[str, Any]
    resource_type: str
    resource_id: str
    version: str
    submission: str
    scope: Any
    provenance: tuple[str, ...]
    access: str
    state: str
    verification: str
    score: HubCardScoreReport
    non_claims: tuple[Any, ...]
    attributions: tuple[Any, ...]
    limitations: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubCardObjectReport":
        raw = _route_mapping("hub card object", value)
        state = _route_text("hub card state", raw.get("state"))
        if state not in HUB_CARD_STATES:
            raise ArgumentError(f"unknown hub card state {state!r}")
        verification = _route_text("hub card verification", raw.get("verification"))
        if verification not in HUB_CARD_VERIFICATION:
            raise ArgumentError(f"unknown hub card verification {verification!r}")
        return cls(raw, _route_text("hub card resource_type", raw.get("resource_type")), _route_text("hub card resource_id", raw.get("resource_id")), _route_text("hub card version", raw.get("version")), _route_text("hub card submission", raw.get("submission")), raw.get("scope"), _route_strings("hub card provenance", raw.get("provenance", [])), _route_text("hub card access", raw.get("access")), state, verification, HubCardScoreReport.from_wire(raw.get("score")), tuple(raw.get("non_claims", [])), tuple(raw.get("attributions", [])), _route_text("hub card limitations", raw.get("limitations")))

    @property
    def score_is_withheld(self) -> bool:
        return self.score.display == "withheld"

    @property
    def is_numeric(self) -> bool:
        return self.score.numeric_value is not None


@dataclass(frozen=True)
class HubCardAttachmentReport:
    raw: dict[str, Any]
    attached: bool
    pack: str | None
    computed_at: int | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubCardAttachmentReport":
        raw = _route_mapping("hub card attachment", value)
        attached = _bool("hub card attachment attached", raw.get("attached"))
        pack = _optional_text("hub card attachment pack", raw.get("pack"))
        computed_at = None if raw.get("computed_at") is None else _integer("hub card attachment computed_at", raw.get("computed_at"))
        if attached and (pack is None or computed_at is None):
            raise ArgumentError("attached hub card scores must retain pack and computed_at")
        if not attached and (pack is not None or computed_at is not None):
            raise ArgumentError("unattached hub card scores cannot retain pack or computed_at")
        return cls(raw, attached, pack, computed_at)


@dataclass(frozen=True)
class HubCardRenderReport:
    raw: dict[str, Any]
    ok: bool
    schema: str
    card: HubCardObjectReport
    score: HubCardAttachmentReport | None
    moderation_state: str | None
    verification: str | None
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubCardRenderReport":
        raw = _payload(value)
        ok = _bool("hub card ok", raw.get("ok"))
        refusal = _optional_text("hub card refusal", raw.get("refusal"))
        fail_closed = _bool("hub card fail_closed", raw.get("fail_closed", False))
        if not ok and (refusal is None or not fail_closed):
            raise ArgumentError("failed hub card renders must be fail-closed")
        score_raw = raw.get("score")
        attachment = None if score_raw is None else HubCardAttachmentReport.from_wire(score_raw)
        if not ok and score_raw is not None:
            raise ArgumentError("failed hub card renders must withhold the score attachment")
        return cls(raw, ok, _route_text("hub card schema", raw.get("schema")), HubCardObjectReport.from_wire(raw.get("card")), attachment, _optional_text("hub card moderation_state", raw.get("moderation_state")), _optional_text("hub card verification", raw.get("verification")), _optional_text("hub card stage", raw.get("stage")), refusal, fail_closed, _route_strings("hub card guarantees", raw.get("guarantees", [])))

    @property
    def score_withheld(self) -> bool:
        return self.card.score_is_withheld or self.score is None or not self.score.attached

    @property
    def numeric_score_exposed(self) -> bool:
        return self.ok and self.card.is_numeric and self.score is not None and self.score.attached

    @property
    def publication_state(self) -> str:
        return self.card.state

    @property
    def state_gate_is_visible(self) -> bool:
        return any("available publication state" in item for item in self.guarantees)

    @property
    def withholding_is_not_zero(self) -> bool:
        return any("never uses zero or blank" in item for item in self.guarantees)

    @property
    def renderer_is_not_a_publisher(self) -> bool:
        return any("does not render HTML" in item and "publish" in item for item in self.guarantees)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def hub_card_render(value: Mapping[str, Any]) -> HubCardRenderReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return HubCardRenderReport.from_wire(value)


__all__ = [
    "HUB_CARD_SCHEMA",
    "HUB_CARD_MAX_INPUT_BYTES",
    "HUB_CARD_STATES",
    "HUB_CARD_SCORE_DISPLAYS",
    "HUB_CARD_LABELS",
    "HUB_CARD_VERIFICATION",
    "HubCardRenderArgs",
    "HubCardLabelReport",
    "HubCardScoreReport",
    "HubCardObjectReport",
    "HubCardAttachmentReport",
    "HubCardRenderReport",
    "hub_card_render",
]
