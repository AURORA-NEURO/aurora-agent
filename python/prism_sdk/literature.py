"""Typed boundary for binding literature claims without laundering source evidence."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .capability import _route_mapping, _route_text
from .errors import ArgumentError


LITERATURE_BIND_SCHEMA = "bioprism-mcp/literature-bind-check/0.1"
LITERATURE_BIND_OUTCOME_KINDS = frozenset({"bound", "citable", "cite_refused", "refused"})
LITERATURE_BINDING_REFUSAL_KINDS = frozenset({"citation_laundering", "unstated_population", "population_mismatch", "temporal_leakage", "retracted_source"})
LITERATURE_CLAIM_KINDS = frozenset({"population_average", "absolute_abundance_change", "cell_identity", "cell_composition", "cell_intrinsic_change", "spatial_localization", "cell_communication", "protein_activity", "flux_rate", "gene_dependency", "causal_effect_of_perturbation", "binding_affinity", "exposure_at_site", "host_mechanism", "subject_level_outcome", "treatment_effect", "temporal_order", "cross_species_equivalence", "published_claim_support", "dataset_content"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _object(name: str, value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be an object")
    return dict(value)


@dataclass(frozen=True)
class LiteratureBindCheckArgs:
    claim: Mapping[str, Any]
    target: Mapping[str, Any]
    at_tier: str
    horizon: Mapping[str, Any]
    flag_warrant: str | None = None
    claim_kind: str | None = None

    def __post_init__(self) -> None:
        claim = _object("literature claim", self.claim)
        target = _object("literature target", self.target)
        horizon = _object("literature horizon", self.horizon)
        at_tier = _route_text("literature citation tier", self.at_tier)
        if at_tier not in {"primary", "review", "guideline", "database"}:
            raise ArgumentError(f"unknown literature citation tier: {at_tier!r}")
        flag_warrant = None if self.flag_warrant is None else _route_text("literature flag warrant", self.flag_warrant)
        if flag_warrant is not None and not flag_warrant.strip():
            raise ArgumentError("literature flag warrant must not be empty")
        claim_kind = None if self.claim_kind is None else _route_text("literature claim kind", self.claim_kind)
        if claim_kind is not None and claim_kind not in LITERATURE_CLAIM_KINDS:
            raise ArgumentError(f"unknown literature claim kind: {claim_kind!r}")
        object.__setattr__(self, "claim", claim)
        object.__setattr__(self, "target", target)
        object.__setattr__(self, "at_tier", at_tier)
        object.__setattr__(self, "horizon", horizon)
        object.__setattr__(self, "flag_warrant", flag_warrant)
        object.__setattr__(self, "claim_kind", claim_kind)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LiteratureBindCheckArgs":
        raw = _object("literature binding arguments", value)
        return cls(raw.get("claim"), raw.get("target"), raw.get("at_tier"), raw.get("horizon"), raw.get("flag_warrant"), raw.get("claim_kind"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "claim": dict(self.claim),
            "target": dict(self.target),
            "at_tier": self.at_tier,
            "horizon": dict(self.horizon),
        }
        if self.flag_warrant is not None:
            result["flag_warrant"] = self.flag_warrant
        if self.claim_kind is not None:
            result["claim_kind"] = self.claim_kind
        return result


@dataclass(frozen=True)
class LiteratureBindCheckReport:
    raw: dict[str, Any]
    ok: bool
    outcome_kind: str
    bound: bool
    citable: bool | None
    evidence: dict[str, Any]
    refusal_kind: str | None = None
    citation_refusal_kind: str | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LiteratureBindCheckReport":
        raw = _object("literature binding report", value)
        ok = _bool("literature binding report ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("literature binding report transport projection is not successful")
        schema = raw.get("schema")
        if schema != LITERATURE_BIND_SCHEMA:
            raise ArgumentError(f"unknown literature binding schema: {schema!r}")
        outcome_kind = _route_text("literature binding outcome kind", raw.get("outcome_kind"))
        if outcome_kind not in LITERATURE_BIND_OUTCOME_KINDS:
            raise ArgumentError(f"unknown literature binding outcome kind: {outcome_kind!r}")
        bound = _bool("literature binding bound", raw.get("bound"))
        citable_value = raw.get("citable")
        citable = None if citable_value is None else _bool("literature binding citable", citable_value)
        evidence = _route_mapping("literature binding evidence", raw.get("evidence"))
        if evidence.get("outcome_kind") != outcome_kind or evidence.get("bound") != bound or evidence.get("citable") != citable:
            raise ArgumentError("literature binding top-level and evidence projections do not reconcile")
        refusal_kind_value = evidence.get("refusal_kind")
        refusal_kind = None if refusal_kind_value is None else _route_text("literature binding refusal kind", refusal_kind_value)
        if not bound:
            if outcome_kind != "refused" or refusal_kind not in LITERATURE_BINDING_REFUSAL_KINDS:
                raise ArgumentError("refused literature bindings must retain a typed binding refusal")
        elif refusal_kind is not None:
            raise ArgumentError("bound literature claims cannot carry a binding refusal")
        citation_refusal_kind_value = evidence.get("citation_refusal_kind")
        citation_refusal_kind = None if citation_refusal_kind_value is None else _route_text("literature citation refusal kind", citation_refusal_kind_value)
        if outcome_kind == "citable" and (not bound or citable is not True or evidence.get("citation") is None):
            raise ArgumentError("citable literature reports require a bound claim and citation record")
        if outcome_kind == "cite_refused" and (not bound or citable is not False or citation_refusal_kind is None):
            raise ArgumentError("cite-refused literature reports require a bound claim and citation refusal")
        if outcome_kind == "bound" and (not bound or citable is not None):
            raise ArgumentError("bound literature reports cannot claim citation status that was not requested")
        return cls(raw, True, outcome_kind, bound, citable, evidence, refusal_kind, citation_refusal_kind)


def literature_bind_check_report(value: Mapping[str, Any]) -> LiteratureBindCheckReport:
    return LiteratureBindCheckReport.from_wire(value)


__all__ = [
    "LITERATURE_BIND_SCHEMA",
    "LITERATURE_BIND_OUTCOME_KINDS",
    "LITERATURE_BINDING_REFUSAL_KINDS",
    "LITERATURE_CLAIM_KINDS",
    "LiteratureBindCheckArgs",
    "LiteratureBindCheckReport",
    "literature_bind_check_report",
]
