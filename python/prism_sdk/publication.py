"""Typed publication-readiness projections for BioAtlas and public-hub gates."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text, _tool_payload
from .errors import ArgumentError


def _publication_bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


@dataclass(frozen=True)
class PublicationTargetReport:
    """One explicit publication target and its complete bounded blocker list."""

    raw: dict[str, Any]
    target: str
    eligible: bool
    blockers: tuple[str, ...]
    notes: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PublicationTargetReport":
        raw = _route_mapping("publication target", value)
        return cls(
            raw=raw,
            target=_route_text("publication target name", raw.get("target")),
            eligible=_publication_bool("publication target eligible", raw.get("eligible")),
            blockers=_route_strings("publication target blockers", raw.get("blockers", [])),
            notes=_route_strings("publication target notes", raw.get("notes", [])),
        )

    @property
    def ready(self) -> bool:
        return self.eligible and not self.blockers

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class PublicationReleaseRequestReport:
    """Explicit publication request state; absent requests cannot become implicit passes."""

    raw: dict[str, Any]
    present: bool
    ready: bool
    fail_closed: bool | None
    no_implicit_release: bool
    request_id: str | None
    targets: tuple[PublicationTargetReport, ...]
    reason: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PublicationReleaseRequestReport":
        raw = _route_mapping("publication release request", value)
        present = _publication_bool("publication release request present", raw.get("present"))
        ready = _publication_bool("publication release request ready", raw.get("ready"))
        raw_targets = raw.get("targets", [])
        if not isinstance(raw_targets, Sequence) or isinstance(raw_targets, (str, bytes)):
            raise ArgumentError("publication release request targets must be an array")
        targets = tuple(PublicationTargetReport.from_wire(target) for target in raw_targets)
        names = tuple(target.target for target in targets)
        if present and not targets:
            raise ArgumentError("present publication requests require targets")
        if not present and targets:
            raise ArgumentError("absent publication requests cannot contain targets")
        if len(names) != len(set(names)):
            raise ArgumentError("publication release request targets must be unique")
        if present and ready != all(target.eligible for target in targets):
            raise ArgumentError("publication release readiness does not reconcile with targets")
        raw_fail_closed = raw.get("fail_closed")
        fail_closed = None if raw_fail_closed is None else _publication_bool(
            "publication release request fail_closed", raw_fail_closed
        )
        if fail_closed is not None and fail_closed == ready:
            raise ArgumentError("publication fail_closed must be the inverse of ready")
        raw_id = raw.get("id")
        request_id = None if raw_id is None else _route_text("publication request id", raw_id)
        raw_reason = raw.get("reason")
        reason = None if raw_reason is None else _route_text("publication request reason", raw_reason)
        return cls(
            raw=raw,
            present=present,
            ready=ready,
            fail_closed=fail_closed,
            no_implicit_release=_publication_bool(
                "publication no_implicit_release", raw.get("no_implicit_release")
            ),
            request_id=request_id,
            targets=targets,
            reason=reason,
        )

    @property
    def blockers(self) -> tuple[str, ...]:
        return tuple(blocker for target in self.targets for blocker in target.blockers)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class PublicationCrossLayerReport:
    """Cross-layer gates connecting atlas, evidence, scores, and leaderboard state."""

    raw: dict[str, Any]
    numeric_score_requires_evidence_audit: bool
    numeric_score_evidence_ready: bool
    atlas_aggregation_ready: bool
    leaderboard_ranked_count: int
    leaderboard_unranked_count: int
    unranked_leaderboard_entries_remain_visible: bool
    withheld_scores_are_not_zeroes: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PublicationCrossLayerReport":
        raw = _route_mapping("publication cross layer", value)
        return cls(
            raw=raw,
            numeric_score_requires_evidence_audit=_publication_bool(
                "publication numeric_score_requires_evidence_audit",
                raw.get("numeric_score_requires_evidence_audit"),
            ),
            numeric_score_evidence_ready=_publication_bool(
                "publication numeric_score_evidence_ready", raw.get("numeric_score_evidence_ready")
            ),
            atlas_aggregation_ready=_publication_bool(
                "publication atlas_aggregation_ready", raw.get("atlas_aggregation_ready")
            ),
            leaderboard_ranked_count=_route_count(
                "publication leaderboard_ranked_count", raw.get("leaderboard_ranked_count")
            ),
            leaderboard_unranked_count=_route_count(
                "publication leaderboard_unranked_count", raw.get("leaderboard_unranked_count")
            ),
            unranked_leaderboard_entries_remain_visible=_publication_bool(
                "publication unranked_leaderboard_entries_remain_visible",
                raw.get("unranked_leaderboard_entries_remain_visible"),
            ),
            withheld_scores_are_not_zeroes=_publication_bool(
                "publication withheld_scores_are_not_zeroes",
                raw.get("withheld_scores_are_not_zeroes"),
            ),
        )

    @property
    def fully_ranked(self) -> bool:
        return self.leaderboard_unranked_count == 0

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class BioAtlasPublicationAuditReport:
    """Typed atlas/publication gates with optional evidence, card, and leaderboard evidence."""

    raw: dict[str, Any]
    workflow: str
    atlas: dict[str, Any]
    evidence_audit: dict[str, Any] | None
    card: dict[str, Any] | None
    leaderboard: dict[str, Any] | None
    release_request: PublicationReleaseRequestReport
    cross_layer: PublicationCrossLayerReport
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioAtlasPublicationAuditReport":
        raw = _route_mapping("BioAtlas publication audit report", value)
        if raw.get("ok") is False:
            raise ArgumentError("BioAtlas publication audit report is not successful")
        if raw.get("workflow") != "bioatlas_publication_audit":
            raise ArgumentError("BioAtlas publication audit workflow is invalid")
        return cls(
            raw=raw,
            workflow=_route_text("BioAtlas publication workflow", raw.get("workflow")),
            atlas=_route_mapping("BioAtlas atlas report", raw.get("atlas", {})),
            evidence_audit=(
                None
                if raw.get("evidence_audit") is None
                else _route_mapping("BioAtlas evidence audit", raw.get("evidence_audit"))
            ),
            card=None if raw.get("card") is None else _route_mapping("BioAtlas card", raw.get("card")),
            leaderboard=(
                None
                if raw.get("leaderboard") is None
                else _route_mapping("BioAtlas leaderboard", raw.get("leaderboard"))
            ),
            release_request=PublicationReleaseRequestReport.from_wire(
                raw.get("release_request", {})
            ),
            cross_layer=PublicationCrossLayerReport.from_wire(raw.get("cross_layer", {})),
            guarantees=_route_strings("BioAtlas guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("BioAtlas limitations", raw.get("limitations", [])),
        )

    @property
    def ready_for_requested_publication(self) -> bool:
        return self.release_request.present and self.release_request.ready

    @property
    def has_evidence_audit(self) -> bool:
        return self.evidence_audit is not None

    @property
    def has_leaderboard(self) -> bool:
        return self.leaderboard is not None

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioatlas_publication_audit_report(
    value: Mapping[str, Any],
) -> BioAtlasPublicationAuditReport:
    """Parse a direct publication audit result or an HTTP tool envelope."""

    return BioAtlasPublicationAuditReport.from_wire(
        _tool_payload(value, "bioatlas_publication_audit")
    )


__all__ = [
    "BioAtlasPublicationAuditReport",
    "PublicationCrossLayerReport",
    "PublicationReleaseRequestReport",
    "PublicationTargetReport",
    "bioatlas_publication_audit_report",
]
