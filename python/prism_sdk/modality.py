"""Typed boundary for modality claim eligibility and analysis-unit safeguards."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .capability import _route_mapping, _route_text
from .errors import ArgumentError


MODALITY_SUPPORT_SCHEMA = "bioprism-mcp/modality-support-check/0.1"
MODALITY_SUPPORT_OUTCOME_KINDS = frozenset({"supported", "refused"})
MODALITIES = frozenset({
    "epigenomics", "bulk_transcriptomics", "single_cell", "spatial", "proteomics",
    "metabolomics", "functional_screen", "protein_structure", "pharmacology", "microbiome",
    "microscopy", "digital_pathology", "clinical_ehr", "trials_and_rwe", "literature",
    "model_organism", "neuro_oncology_connector",
})
MODALITY_CLAIMS = frozenset({
    "population_average", "absolute_abundance_change", "cell_identity", "cell_composition",
    "cell_intrinsic_change", "spatial_localization", "cell_communication", "protein_activity",
    "flux_rate", "gene_dependency", "causal_effect_of_perturbation", "binding_affinity",
    "exposure_at_site", "host_mechanism", "subject_level_outcome", "treatment_effect",
    "temporal_order", "cross_species_equivalence", "published_claim_support", "dataset_content",
})
MODALITY_RESOLUTIONS = frozenset({"population", "cell", "location", "molecule", "subject", "timepoint", "perturbation"})


def _object(name: str, value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be an object")
    return dict(value)


@dataclass(frozen=True)
class ModalitySupportCheckArgs:
    modality: str
    claim: str
    descriptor: Mapping[str, Any] | None = None
    counted_unit: str | None = None

    def __post_init__(self) -> None:
        modality = _route_text("modality", self.modality)
        claim = _route_text("modality claim", self.claim)
        if modality not in MODALITIES:
            raise ArgumentError(f"unknown modality: {modality!r}")
        if claim not in MODALITY_CLAIMS:
            raise ArgumentError(f"unknown modality claim: {claim!r}")
        descriptor = None if self.descriptor is None else _object("modality descriptor", self.descriptor)
        counted_unit = None if self.counted_unit is None else _route_text("counted analysis unit", self.counted_unit)
        if counted_unit is not None and counted_unit not in MODALITY_RESOLUTIONS:
            raise ArgumentError(f"unknown counted analysis unit: {counted_unit!r}")
        object.__setattr__(self, "modality", modality)
        object.__setattr__(self, "claim", claim)
        object.__setattr__(self, "descriptor", descriptor)
        object.__setattr__(self, "counted_unit", counted_unit)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ModalitySupportCheckArgs":
        raw = _object("modality support arguments", value)
        return cls(raw.get("modality"), raw.get("claim"), raw.get("descriptor"), raw.get("counted_unit"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"modality": self.modality, "claim": self.claim}
        if self.descriptor is not None:
            result["descriptor"] = dict(self.descriptor)
        if self.counted_unit is not None:
            result["counted_unit"] = self.counted_unit
        return result


@dataclass(frozen=True)
class ModalitySupportCheckReport:
    raw: dict[str, Any]
    ok: bool
    outcome_kind: str
    modality: str
    claim: str
    supported: bool
    support: dict[str, Any]
    analysis_unit: dict[str, Any]
    descriptor: dict[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ModalitySupportCheckReport":
        raw = _object("modality support report", value)
        if raw.get("ok") is not True:
            raise ArgumentError("modality support report transport projection is not successful")
        if raw.get("schema") != MODALITY_SUPPORT_SCHEMA:
            raise ArgumentError(f"unknown modality support schema: {raw.get('schema')!r}")
        outcome_kind = _route_text("modality support outcome kind", raw.get("outcome_kind"))
        if outcome_kind not in MODALITY_SUPPORT_OUTCOME_KINDS:
            raise ArgumentError(f"unknown modality support outcome kind: {outcome_kind!r}")
        modality = _route_text("modality support modality", raw.get("modality"))
        claim = _route_text("modality support claim", raw.get("claim"))
        supported = raw.get("supported")
        if not isinstance(supported, bool):
            raise ArgumentError("modality support supported must be a boolean")
        if (outcome_kind == "supported") != supported:
            raise ArgumentError("modality support outcome and supported flag do not reconcile")
        support = _route_mapping("modality support evidence", raw.get("support"))
        if support.get("supported") != supported:
            raise ArgumentError("modality support evidence does not reconcile with the top-level result")
        if not supported and support.get("refusal") is None:
            raise ArgumentError("refused modality support must retain a typed refusal")
        analysis_unit = _route_mapping("modality analysis-unit evidence", raw.get("analysis_unit"))
        admissible = analysis_unit.get("admissible")
        if admissible is not None and not isinstance(admissible, bool):
            raise ArgumentError("modality analysis-unit admissible must be boolean or null")
        descriptor = _route_mapping("modality descriptor evidence", raw.get("descriptor"))
        return cls(raw, True, outcome_kind, modality, claim, supported, support, analysis_unit, descriptor)


def modality_support_check_report(value: Mapping[str, Any]) -> ModalitySupportCheckReport:
    return ModalitySupportCheckReport.from_wire(value)


__all__ = [
    "MODALITY_SUPPORT_SCHEMA",
    "MODALITY_SUPPORT_OUTCOME_KINDS",
    "MODALITIES",
    "MODALITY_CLAIMS",
    "MODALITY_RESOLUTIONS",
    "ModalitySupportCheckArgs",
    "ModalitySupportCheckReport",
    "modality_support_check_report",
]
