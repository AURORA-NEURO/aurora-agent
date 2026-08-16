"""Typed SDK projections for the deeper OncoWorlds contracts.

These helpers keep model-system transport, methylation classification, radiogenomic evaluation,
and clonal-history reasoning separate.  They validate only the top-level request/result shape and
cross-field evidence invariants; the Rust ``bioprism-oncoworlds`` crate remains authoritative for
identity, fidelity, calibration, split safety, scope transport, and cellular-fraction arithmetic.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


METHYLATION_DIVERGENCES = frozenset({"agree", "both_unclassifiable", "version_conditioned"})
METHYLATION_CLASSIFY_SCHEMA = "bioprism-mcp/oncoworlds-methylation-classify/0.1"
METHYLATION_COMPARE_SCHEMA = "bioprism-mcp/oncoworlds-methylation-compare/0.1"
METHYLATION_OUTCOME_KINDS = frozenset({"classified", "unclassifiable", "refused"})
METHYLATION_REFUSAL_KINDS = frozenset({"undeclared_threshold", "score_out_of_range", "uncalibrated_cross_version", "circular_copy_number", "circular_label_use", "unclassifiable"})
ONCOWORLDS_CLONAL_SCHEMA = "bioprism-mcp/oncoworlds-clonal-history-check/0.1"
ONCOWORLDS_CLONAL_REFUSAL_KINDS = frozenset({"fractions_exceed_whole", "child_exceeds_parent", "cyclic", "unknown_subclone", "ambiguous", "unsupported_directionality"})
ONCOWORLDS_CLONAL_UNIQUE_STATUSES = frozenset({"unique", "ambiguous", "refused"})
ONCOWORLDS_CLONAL_EVIDENCE_SCHEMA = "bioprism-mcp/oncoworlds-clonal-evidence-check/0.1"
ONCOWORLDS_CLONAL_EVIDENCE_OUTCOME_KINDS = frozenset({"report"})
ONCOWORLDS_CLONAL_EVIDENCE_REFUSAL_KINDS = frozenset({"undeclared_sensitivity", "no_region_sampled", "not_an_absence", "copy_number_unknown", "ambiguous", "unsupported_directionality"})
ONCOWORLDS_RADIOGENOMIC_SCHEMA = "bioprism-mcp/oncoworlds-radiogenomic-check/0.1"
ONCOWORLDS_RADIOGENOMIC_TARGETS = frozenset({"association", "mechanism"})
ONCOWORLDS_RADIOGENOMIC_SPLIT_UNITS = frozenset({"image", "imaging_series", "specimen", "participant", "site"})
ONCOWORLDS_RADIOGENOMIC_FEATURE_PROVENANCE = frozenset({"fitted_on_training_split_only", "fitted_on_all_data"})
ONCOWORLDS_RADIOGENOMIC_REFUSAL_KINDS = frozenset({"undeclared_loss", "unstated_assumption", "leaky_split", "unstratified_claim", "specimen_scoped_target", "post_hoc_cohort_selection"})
ONCOWORLDS_RADIOGENOMIC_OUTCOME_KINDS = frozenset({"supported", "refused"})
ONCOWORLDS_MODEL_SCHEMA = "bioprism-mcp/oncoworlds-model-transport/0.1"
ONCOWORLDS_MODEL_OUTCOME_KINDS = frozenset({"supported", "refused"})
ONCOWORLDS_MODEL_REFUSAL_KINDS = frozenset({"unverified_model_identity", "unmeasured_fidelity", "unmodelled_establishment_selection", "technical_replicates_as_biological", "undeclared_loss", "unstated_assumption"})
ONCOWORLDS_MODEL_FIDELITY_AXES = frozenset({"genomic", "epigenetic", "transcriptomic", "phenotypic", "histologic"})
ONCOWORLDS_ERA_SCHEMA = "bioprism-mcp/oncoworlds-era-shift-check/0.1"
ONCOWORLDS_ERA_OUTCOME_KINDS = frozenset({"comparable", "refused"})
ONCOWORLDS_ERA_REFUSAL_KINDS = frozenset({"unmapped_classification_change", "incomplete_mapping", "resource_absence_read_as_biology", "descriptor_used_as_mechanism"})
ONCOWORLDS_EQUITY_SCHEMA = "bioprism-mcp/oncoworlds-equity-check/0.1"
ONCOWORLDS_EQUITY_OUTCOME_KINDS = frozenset({"equity_report", "refused"})
ONCOWORLDS_EQUITY_REFUSAL_KINDS = frozenset({"pooled_score_only", "unquantified_subgroup", "empty_subgroup"})
ONCOWORLDS_ENTITY_SCHEMA = "bioprism-mcp/oncoworlds-entity-world-check/0.1"
ONCOWORLDS_ENTITY_OUTCOME_KINDS = frozenset({"report"})
ONCOWORLDS_ENTITY_REFUSAL_KINDS = frozenset({"unmodelled_provenance_selection", "mechanism_collapse", "macro_score_without_counts", "undeclared_cluster", "competing_event_as_censoring"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _optional_mapping(name: str, value: Any) -> dict[str, Any] | None:
    return None if value is None else _route_mapping(name, value)


def _number(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _payload(value: Mapping[str, Any], *, label: str, direct_keys: tuple[str, ...]) -> dict[str, Any]:
    raw = _route_mapping(f"{label} response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        return "ok" in candidate and any(key in candidate for key in direct_keys)

    if matches(raw):
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
            if isinstance(structured, Mapping) and matches(structured):
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
                if matches(decoded_mapping):
                    return decoded_mapping
    raise ArgumentError(f"response does not contain a {label} projection")


def _domain_refusal(raw: Mapping[str, Any], label: str) -> tuple[str, Any, str, bool, str | None]:
    stage = _route_text(f"{label} stage", raw.get("stage"))
    refusal = raw.get("refusal")
    if not isinstance(refusal, Mapping):
        raise ArgumentError(f"{label} refusal must retain its typed object")
    refusal_text = _route_text(f"{label} refusal_text", raw.get("refusal_text"))
    fail_closed = _bool(f"{label} fail_closed", raw.get("fail_closed"))
    if not fail_closed:
        raise ArgumentError(f"refused {label} results must be fail-closed")
    return stage, dict(refusal), refusal_text, True, _optional_text(f"{label} guarantee", raw.get("guarantee"))


def _object(name: str, value: Mapping[str, Any]) -> dict[str, Any]:
    return _route_mapping(name, value)


@dataclass(frozen=True)
class OncoWorldsEraShiftCheckArgs:
    left: Mapping[str, Any]
    right: Mapping[str, Any]
    mapping: Mapping[str, Any] | None = None
    assay_contexts: tuple[Mapping[str, Any], ...] = ()
    descriptor_checks: tuple[Mapping[str, Any], ...] = ()

    def __post_init__(self) -> None:
        object.__setattr__(self, "left", _object("left OncoWorlds cohort", self.left))
        object.__setattr__(self, "right", _object("right OncoWorlds cohort", self.right))
        object.__setattr__(self, "mapping", _optional_mapping("OncoWorlds entity mapping", self.mapping))
        assays = tuple(_object(f"site assay context[{index}]", value) for index, value in enumerate(self.assay_contexts))
        descriptors = tuple(_object(f"descriptor check[{index}]", value) for index, value in enumerate(self.descriptor_checks))
        if len(assays) > 100 or len(descriptors) > 100:
            raise ArgumentError("OncoWorlds era-shift evidence panels may contain at most 100 entries")
        object.__setattr__(self, "assay_contexts", assays)
        object.__setattr__(self, "descriptor_checks", descriptors)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsEraShiftCheckArgs":
        raw = _object("OncoWorlds era-shift arguments", value)
        assays = _array("OncoWorlds assay contexts", raw.get("assay_contexts", []))
        descriptors = _array("OncoWorlds descriptor checks", raw.get("descriptor_checks", []))
        return cls(raw.get("left"), raw.get("right"), raw.get("mapping"), assays, descriptors)

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "left": dict(self.left),
            "right": dict(self.right),
            "assay_contexts": [dict(value) for value in self.assay_contexts],
            "descriptor_checks": [dict(value) for value in self.descriptor_checks],
        }
        if self.mapping is not None:
            result["mapping"] = dict(self.mapping)
        return result


@dataclass(frozen=True)
class OncoWorldsEquityCheckArgs:
    pooled: Mapping[str, Any]

    def __post_init__(self) -> None:
        pooled = _object("OncoWorlds pooled score", self.pooled)
        subgroups = _array("OncoWorlds pooled subgroups", pooled.get("subgroups", []))
        if len(subgroups) > 10_000:
            raise ArgumentError("OncoWorlds equity subgroup panel may contain at most 10000 entries")
        object.__setattr__(self, "pooled", pooled)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsEquityCheckArgs":
        raw = _object("OncoWorlds equity arguments", value)
        return cls(raw.get("pooled"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"pooled": dict(self.pooled)}


@dataclass(frozen=True)
class OncoWorldsEntityWorldCheckArgs:
    provenance: Mapping[str, Any] | None = None
    alterations: Mapping[str, Any] | None = None
    benchmark: Mapping[str, Any] | None = None
    lesion_analysis: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        sections = {
            "provenance": _optional_mapping("entity-world provenance", self.provenance),
            "alterations": _optional_mapping("entity-world alterations", self.alterations),
            "benchmark": _optional_mapping("entity-world benchmark", self.benchmark),
            "lesion_analysis": _optional_mapping("entity-world lesion analysis", self.lesion_analysis),
        }
        if not any(value is not None for value in sections.values()):
            raise ArgumentError("at least one entity-world check section is required")
        object.__setattr__(self, "provenance", sections["provenance"])
        object.__setattr__(self, "alterations", sections["alterations"])
        object.__setattr__(self, "benchmark", sections["benchmark"])
        object.__setattr__(self, "lesion_analysis", sections["lesion_analysis"])

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsEntityWorldCheckArgs":
        raw = _object("OncoWorlds entity-world arguments", value)
        return cls(raw.get("provenance"), raw.get("alterations"), raw.get("benchmark"), raw.get("lesion_analysis"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for name in ("provenance", "alterations", "benchmark", "lesion_analysis"):
            value = getattr(self, name)
            if value is not None:
                result[name] = dict(value)
        return result


@dataclass(frozen=True)
class OncoShiftCohortProjection:
    raw: dict[str, Any]
    name: str
    site: str
    classification_version: str
    entities: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoShiftCohortProjection":
        raw = _object("OncoWorlds cohort projection", value)
        entities = _route_strings("OncoWorlds cohort entities", raw.get("entities", []))
        return cls(
            raw,
            _route_text("OncoWorlds cohort name", raw.get("name")),
            _route_text("OncoWorlds cohort site", raw.get("site")),
            _route_text("OncoWorlds cohort classification version", raw.get("classification_version")),
            entities,
        )


@dataclass(frozen=True)
class OncoAssayShiftProjection:
    raw: dict[str, Any]
    site: str
    assay: str
    availability: dict[str, Any]
    observation: dict[str, Any]
    negative_call_supported: bool
    negative_call_refusal_kind: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoAssayShiftProjection":
        raw = _object("OncoWorlds assay shift projection", value)
        refusal = _object("OncoWorlds assay negative-call refusal", raw.get("negative_call_refusal"))
        refusal_kind = _route_text("OncoWorlds assay refusal kind", raw.get("negative_call_refusal_kind", refusal.get("refusal")))
        if refusal_kind != "resource_absence_read_as_biology":
            raise ArgumentError("OncoWorlds assay projection must retain the resource-absence refusal")
        negative_call_supported = _bool("OncoWorlds negative-call support", raw.get("negative_call_supported"))
        if negative_call_supported:
            raise ArgumentError("OncoWorlds assay projection cannot support a negative call")
        return cls(
            raw,
            _route_text("OncoWorlds assay site", raw.get("site")),
            _route_text("OncoWorlds assay name", raw.get("assay")),
            _object("OncoWorlds assay availability", raw.get("availability")),
            _object("OncoWorlds assay observation", raw.get("observation")),
            negative_call_supported,
            refusal_kind,
        )


@dataclass(frozen=True)
class OncoDescriptorShiftProjection:
    raw: dict[str, Any]
    descriptor: str
    use: str
    administrative: bool
    allowed: bool
    refusal_kind: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoDescriptorShiftProjection":
        raw = _object("OncoWorlds descriptor shift projection", value)
        allowed = _bool("OncoWorlds descriptor allowed", raw.get("allowed"))
        refusal = raw.get("refusal")
        refusal_kind = None if refusal is None else _route_text("OncoWorlds descriptor refusal kind", raw.get("refusal_kind", _object("OncoWorlds descriptor refusal", refusal).get("refusal")))
        if allowed and refusal_kind is not None:
            raise ArgumentError("allowed descriptor checks cannot carry refusal evidence")
        if not allowed and refusal_kind != "descriptor_used_as_mechanism":
            raise ArgumentError("refused descriptor checks must retain descriptor_used_as_mechanism")
        return cls(
            raw,
            _route_text("OncoWorlds descriptor", raw.get("descriptor_label")),
            _route_text("OncoWorlds descriptor use", raw.get("use_label")),
            _bool("OncoWorlds descriptor administrative", raw.get("administrative")),
            allowed,
            refusal_kind,
        )


@dataclass(frozen=True)
class OncoWorldsModelTransportArgs:
    result: Mapping[str, Any]
    establishment: Mapping[str, Any]
    claimed_n: int
    transport: Mapping[str, Any]
    fidelity: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "result", _object("oncoworlds model result", self.result))
        object.__setattr__(self, "establishment", _object("oncoworlds establishment", self.establishment))
        if isinstance(self.claimed_n, bool) or not isinstance(self.claimed_n, int) or self.claimed_n < 0 or self.claimed_n > 1_000_000:
            raise ArgumentError("oncoworlds claimed_n must be an integer in 0..=1000000")
        object.__setattr__(self, "transport", _object("oncoworlds declared transport", self.transport))
        object.__setattr__(self, "fidelity", _optional_mapping("oncoworlds fidelity", self.fidelity))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsModelTransportArgs":
        raw = _object("oncoworlds model transport arguments", value)
        return cls(raw.get("result"), raw.get("establishment"), raw.get("claimed_n"), raw.get("transport"), raw.get("fidelity"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"result": dict(self.result), "establishment": dict(self.establishment), "claimed_n": self.claimed_n, "transport": dict(self.transport)}
        if self.fidelity is not None:
            result["fidelity"] = dict(self.fidelity)
        return result


@dataclass(frozen=True)
class OncoModelIdentityProjection:
    raw: dict[str, Any]
    model: str
    system: str
    source_specimen: str
    passage: int
    verified_against_source: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoModelIdentityProjection":
        raw = _object("model identity", value)
        return cls(
            raw,
            _route_text("model identity model", raw.get("model")),
            _route_text("model identity system", raw.get("system")),
            _route_text("model identity source specimen", raw.get("source_specimen")),
            _route_count("model identity passage", raw.get("passage")),
            _bool("model identity verification", raw.get("verified_against_source")),
        )


@dataclass(frozen=True)
class OncoModelFidelityProjection:
    raw: dict[str, Any]
    axis: str
    passage: int
    measured: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoModelFidelityProjection":
        raw = _object("model fidelity axis", value)
        axis = _route_text("model fidelity axis name", raw.get("axis"))
        if axis not in ONCOWORLDS_MODEL_FIDELITY_AXES:
            raise ArgumentError(f"unknown model fidelity axis: {axis!r}")
        return cls(raw, axis, _route_count("model fidelity passage", raw.get("passage")), _bool("model fidelity measured", raw.get("measured")))


@dataclass(frozen=True)
class OncoModelEstablishmentProjection:
    raw: dict[str, Any]
    attempted: int
    established: int
    selected: bool
    selection_modelled: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoModelEstablishmentProjection":
        raw = _object("model establishment", value)
        attempted = _route_count("model establishment attempted", raw.get("attempted"))
        established = _route_count("model establishment established", raw.get("established"))
        if established > attempted:
            raise ArgumentError("model establishment cannot exceed attempted specimens")
        selected = _bool("model establishment selected", raw.get("selected"))
        if selected != (established < attempted):
            raise ArgumentError("model establishment selected state does not reconcile with counts")
        return cls(raw, attempted, established, selected, _bool("model establishment selection modelled", raw.get("selection_modelled")))


@dataclass(frozen=True)
class OncoModelReplicateProjection:
    raw: dict[str, Any]
    technical_wells: int
    biological_replicates: int
    effective_biological_n: int
    claimed_n: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoModelReplicateProjection":
        raw = _object("model replicates", value)
        technical_wells = _route_count("model technical wells", raw.get("technical_wells"))
        biological_replicates = _route_count("model biological replicates", raw.get("biological_replicates"))
        effective_biological_n = _route_count("model effective biological n", raw.get("effective_biological_n"))
        if effective_biological_n != biological_replicates:
            raise ArgumentError("model effective biological n must equal biological replicates")
        return cls(raw, technical_wells, biological_replicates, effective_biological_n, _route_count("model claimed n", raw.get("claimed_n")))


@dataclass(frozen=True)
class OncoPatientRelevantClaimProjection:
    raw: dict[str, Any]
    result: dict[str, Any]
    cohort: dict[str, Any]
    transport: dict[str, Any]
    claimed_n: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoPatientRelevantClaimProjection":
        raw = _object("patient-relevant model claim", value)
        return cls(
            raw,
            _object("patient-relevant model result", raw.get("result")),
            _object("patient-relevant establishment cohort", raw.get("cohort")),
            _object("patient-relevant transport", raw.get("transport")),
            _route_count("patient-relevant claimed n", raw.get("claimed_n")),
        )


@dataclass(frozen=True)
class OncoWorldsMethylationClassifyArgs:
    classifier: Mapping[str, Any]
    scores: Mapping[str, Any]
    context: Mapping[str, Any]

    def __post_init__(self) -> None:
        object.__setattr__(self, "classifier", _object("methylation classifier", self.classifier))
        object.__setattr__(self, "scores", _object("methylation scores", self.scores))
        if len(self.scores) > 10_000:
            raise ArgumentError("methylation scores may contain at most 10000 classes")
        object.__setattr__(self, "context", _object("methylation sample context", self.context))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsMethylationClassifyArgs":
        raw = _object("methylation classification arguments", value)
        return cls(raw.get("classifier"), raw.get("scores"), raw.get("context"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"classifier": dict(self.classifier), "scores": dict(self.scores), "context": dict(self.context)}


@dataclass(frozen=True)
class OncoWorldsMethylationCompareArgs:
    left: Mapping[str, Any]
    right: Mapping[str, Any]

    def __post_init__(self) -> None:
        object.__setattr__(self, "left", _object("left methylation result", self.left))
        object.__setattr__(self, "right", _object("right methylation result", self.right))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsMethylationCompareArgs":
        raw = _object("methylation comparison arguments", value)
        return cls(raw.get("left"), raw.get("right"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"left": dict(self.left), "right": dict(self.right)}


@dataclass(frozen=True)
class OncoMethylationClassifierProjection:
    raw: dict[str, Any]
    name: str
    version: str
    reference_version: str
    reporting_threshold: int | None
    threshold_declared: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoMethylationClassifierProjection":
        raw = _object("methylation classifier projection", value)
        threshold_value = raw.get("reporting_threshold")
        threshold = None if threshold_value is None else _route_count("methylation reporting threshold", threshold_value)
        threshold_declared = threshold is not None
        return cls(
            raw,
            _route_text("methylation classifier name", raw.get("name")),
            _route_text("methylation classifier version", raw.get("version")),
            _route_text("methylation reference version", raw.get("reference_version")),
            threshold,
            threshold_declared,
        )


@dataclass(frozen=True)
class OncoMethylationOutcomeProjection:
    raw: dict[str, Any]
    kind: str
    class_label: str | None
    reason: dict[str, Any] | None
    nearest: dict[str, Any] | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoMethylationOutcomeProjection":
        raw = _object("methylation outcome projection", value)
        kind = _route_text("methylation outcome kind", raw.get("outcome"))
        if kind not in {"classified", "unclassifiable"}:
            raise ArgumentError(f"unknown methylation outcome: {kind!r}")
        class_label = None if raw.get("class") is None else _route_text("methylation class label", raw.get("class"))
        reason = None if raw.get("reason") is None else _object("methylation unclassifiable reason", raw.get("reason"))
        nearest = None if raw.get("nearest") is None else _object("methylation nearest class", raw.get("nearest"))
        if (kind == "classified") != (class_label is not None) or (kind == "classified" and (reason is not None or nearest is not None)):
            raise ArgumentError("methylation classified outcome does not reconcile with its fields")
        if kind == "unclassifiable" and reason is None:
            raise ArgumentError("methylation unclassifiable outcome must retain its reason")
        return cls(raw, kind, class_label, reason, nearest)


@dataclass(frozen=True)
class OncoMethylationDivergenceProjection:
    raw: dict[str, Any]
    kind: str
    under_left: str | None
    under_right: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoMethylationDivergenceProjection":
        raw = _object("methylation divergence projection", value)
        kind = _route_text("methylation divergence kind", raw.get("divergence"))
        if kind not in METHYLATION_DIVERGENCES:
            raise ArgumentError(f"unknown methylation divergence: {kind!r}")
        under_left = None if raw.get("under_left") is None else _route_text("methylation left conditioned class", raw.get("under_left"))
        under_right = None if raw.get("under_right") is None else _route_text("methylation right conditioned class", raw.get("under_right"))
        if kind == "agree" and under_left != under_right:
            raise ArgumentError("methylation agreement must carry the same conditioned class")
        if kind == "both_unclassifiable" and (under_left is not None or under_right is not None):
            raise ArgumentError("both-unclassifiable methylation divergence cannot carry class labels")
        return cls(raw, kind, under_left, under_right)


@dataclass(frozen=True)
class OncoWorldsRadiogenomicCheckArgs:
    claim: Mapping[str, Any]
    design: Mapping[str, Any]
    observation: Mapping[str, Any]
    transport: Mapping[str, Any]

    def __post_init__(self) -> None:
        for name, value in (("claim", self.claim), ("design", self.design), ("observation", self.observation), ("transport", self.transport)):
            object.__setattr__(self, name, _object(f"radiogenomic {name}", value))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsRadiogenomicCheckArgs":
        raw = _object("radiogenomic arguments", value)
        return cls(raw.get("claim"), raw.get("design"), raw.get("observation"), raw.get("transport"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"claim": dict(self.claim), "design": dict(self.design), "observation": dict(self.observation), "transport": dict(self.transport)}


@dataclass(frozen=True)
class OncoRadiogenomicDesignProjection:
    raw: dict[str, Any]
    split_unit: str
    feature_provenance: str
    feature_version: str
    external_cohort: dict[str, Any] | None
    strata: tuple[str, ...]
    mechanism_strata_present: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoRadiogenomicDesignProjection":
        raw = _object("radiogenomic design projection", value)
        split_unit = _route_text("radiogenomic split unit", raw.get("split_unit"))
        if split_unit not in ONCOWORLDS_RADIOGENOMIC_SPLIT_UNITS:
            raise ArgumentError(f"unknown radiogenomic split unit: {split_unit!r}")
        feature_provenance = _route_text("radiogenomic feature provenance", raw.get("feature_provenance"))
        if feature_provenance not in ONCOWORLDS_RADIOGENOMIC_FEATURE_PROVENANCE:
            raise ArgumentError(f"unknown radiogenomic feature provenance: {feature_provenance!r}")
        strata = _route_strings("radiogenomic strata", raw.get("strata"))
        mechanism_strata_present = _bool("radiogenomic mechanism strata presence", raw.get("mechanism_strata_present"))
        if mechanism_strata_present != all(stratum in strata for stratum in ("site", "scanner")):
            raise ArgumentError("radiogenomic mechanism strata presence does not reconcile with strata")
        return cls(
            raw,
            split_unit,
            feature_provenance,
            _route_text("radiogenomic feature version", raw.get("feature_version")),
            _optional_mapping("radiogenomic external cohort", raw.get("external_cohort")),
            strata,
            mechanism_strata_present,
        )


@dataclass(frozen=True)
class OncoRadiogenomicSupportedClaimProjection:
    raw: dict[str, Any]
    target: str
    statement: str
    label: dict[str, Any]
    strata: tuple[str, ...]
    transport: dict[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoRadiogenomicSupportedClaimProjection":
        raw = _object("supported radiogenomic claim", value)
        claim = _object("supported radiogenomic claim body", raw.get("claim"))
        target = _route_text("radiogenomic claim target", claim.get("target"))
        if target not in ONCOWORLDS_RADIOGENOMIC_TARGETS:
            raise ArgumentError(f"unknown radiogenomic claim target: {target!r}")
        return cls(
            raw,
            target,
            _route_text("radiogenomic claim statement", claim.get("statement")),
            _object("radiogenomic tumour label", raw.get("label")),
            _route_strings("supported radiogenomic strata", raw.get("strata")),
            _object("radiogenomic transport", raw.get("transport")),
        )


@dataclass(frozen=True)
class OncoWorldsClonalHistoryCheckArgs:
    population: Mapping[str, Any]
    candidates: tuple[Mapping[str, Any], ...]

    def __init__(self, population: Mapping[str, Any], candidates: Sequence[Mapping[str, Any]]) -> None:
        object.__setattr__(self, "population", _object("clonal-history population", population))
        if not isinstance(candidates, Sequence) or isinstance(candidates, (str, bytes)):
            raise ArgumentError("clonal-history candidates must be an array")
        normalized = tuple(_object(f"clonal-history candidates[{index}]", item) for index, item in enumerate(candidates))
        if len(normalized) > 10_000:
            raise ArgumentError("clonal-history candidates may contain at most 10000 histories")
        object.__setattr__(self, "candidates", normalized)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsClonalHistoryCheckArgs":
        raw = _object("clonal-history arguments", value)
        return cls(raw.get("population"), raw.get("candidates"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"population": dict(self.population), "candidates": [dict(item) for item in self.candidates]}


@dataclass(frozen=True)
class OncoClonalEvidenceCheckArgs:
    promotion: Mapping[str, Any] | None = None
    resistance: Mapping[str, Any] | None = None
    attribution: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        sections = {
            "promotion": _optional_mapping("clonal promotion", self.promotion),
            "resistance": _optional_mapping("clonal resistance", self.resistance),
            "attribution": _optional_mapping("clonal attribution", self.attribution),
        }
        if not any(value is not None for value in sections.values()):
            raise ArgumentError("at least one clonal evidence section is required")
        object.__setattr__(self, "promotion", sections["promotion"])
        object.__setattr__(self, "resistance", sections["resistance"])
        object.__setattr__(self, "attribution", sections["attribution"])

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoClonalEvidenceCheckArgs":
        raw = _object("clonal evidence arguments", value)
        return cls(raw.get("promotion"), raw.get("resistance"), raw.get("attribution"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for name in ("promotion", "resistance", "attribution"):
            value = getattr(self, name)
            if value is not None:
                result[name] = dict(value)
        return result


@dataclass(frozen=True)
class OncoClonalHistoryProjection:
    raw: dict[str, Any]
    edges: tuple[tuple[str, str], ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoClonalHistoryProjection":
        raw = _object("clonal history", value)
        edges: list[tuple[str, str]] = []
        for index, edge in enumerate(_array("clonal history edges", raw.get("edges", []))):
            if isinstance(edge, Mapping):
                parent = _route_text(f"clonal history edge[{index}].parent", edge.get("parent"))
                child = _route_text(f"clonal history edge[{index}].child", edge.get("child"))
            else:
                pair = _array(f"clonal history edge[{index}]")
                if len(pair) != 2:
                    raise ArgumentError("clonal history edges must be parent/child pairs")
                parent = _route_text(f"clonal history edge[{index}][0]", pair[0])
                child = _route_text(f"clonal history edge[{index}][1]", pair[1])
            edges.append((parent, child))
        if len(edges) != len(set(edges)):
            raise ArgumentError("clonal history edges must be unique")
        return cls(raw, tuple(edges))


@dataclass(frozen=True)
class OncoClonalRejectedHistoryProjection:
    raw: dict[str, Any]
    history: OncoClonalHistoryProjection
    refusal: dict[str, Any]
    refusal_kind: str
    refusal_text: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoClonalRejectedHistoryProjection":
        raw = _object("clonal rejected history", value)
        refusal = _object("clonal history refusal", raw.get("refusal"))
        refusal_kind = _route_text("clonal history refusal kind", raw.get("refusal_kind", refusal.get("refusal")))
        if refusal_kind not in ONCOWORLDS_CLONAL_REFUSAL_KINDS or refusal.get("refusal") != refusal_kind:
            raise ArgumentError("clonal history refusal kind does not reconcile with the typed refusal")
        return cls(raw, OncoClonalHistoryProjection.from_wire(raw.get("history")), refusal, refusal_kind, _optional_text("clonal history refusal_text", raw.get("refusal_text")))


@dataclass(frozen=True)
class OncoClonalUniqueHistoryProjection:
    raw: dict[str, Any]
    ok: bool
    status: str
    history: OncoClonalHistoryProjection | None
    refusal: dict[str, Any] | None
    refusal_text: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any], status: str) -> "OncoClonalUniqueHistoryProjection":
        raw = _object("clonal unique_history", value)
        ok = _bool("clonal unique_history ok", raw.get("ok"))
        if status not in ONCOWORLDS_CLONAL_UNIQUE_STATUSES:
            raise ArgumentError(f"unknown clonal unique-history status: {status!r}")
        if ok:
            if status != "unique" or raw.get("refusal") is not None:
                raise ArgumentError("unique clonal history status cannot carry refusal evidence")
            return cls(raw, True, status, OncoClonalHistoryProjection.from_wire(raw.get("history")), None, None)
        refusal = _object("clonal unique-history refusal", raw.get("refusal"))
        refusal_kind = _route_text("clonal unique-history refusal kind", refusal.get("refusal"))
        if refusal_kind not in ONCOWORLDS_CLONAL_REFUSAL_KINDS or status == "unique":
            raise ArgumentError("non-unique clonal history must retain a known refusal")
        return cls(raw, False, status, None, refusal, _optional_text("clonal unique-history refusal_text", raw.get("refusal_text")))


@dataclass(frozen=True)
class OncoWorldsModelTransportReport:
    raw: dict[str, Any]
    ok: bool
    model_statement: str | None
    effective_biological_n: int | None
    patient_relevant_claim: dict[str, Any] | None
    stage: str | None
    refusal: dict[str, Any] | None
    refusal_text: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    outcome_kind: str | None = None
    refusal_kind: str | None = None
    model_identity: OncoModelIdentityProjection | None = None
    effect: str | None = None
    rests_on: tuple[str, ...] = ()
    fidelity_axes: tuple[OncoModelFidelityProjection, ...] = ()
    establishment: OncoModelEstablishmentProjection | None = None
    replicates: OncoModelReplicateProjection | None = None
    transport_assumption_names: tuple[str, ...] = ()
    required_assumptions: tuple[str, ...] = ()
    patient_relevant_claim_record: OncoPatientRelevantClaimProjection | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsModelTransportReport":
        raw = _payload(value, label="oncoworlds model transport", direct_keys=("patient_relevant_claim", "refusal"))
        ok = _bool("oncoworlds model transport ok", raw.get("ok"))
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("model transport schema", schema_value)
        if schema is not None and schema != ONCOWORLDS_MODEL_SCHEMA:
            raise ArgumentError(f"unknown model transport schema: {schema!r}")
        patient_claim_value = raw.get("patient_relevant_claim")
        supported = _bool("model transport supported", raw.get("supported", ok and patient_claim_value is not None))
        if supported != ok:
            raise ArgumentError("model transport supported does not reconcile with transport success")
        outcome_kind_value = raw.get("outcome_kind")
        outcome_kind = "supported" if supported else "refused"
        if outcome_kind_value is not None:
            outcome_kind = _route_text("model transport outcome kind", outcome_kind_value)
            if outcome_kind not in ONCOWORLDS_MODEL_OUTCOME_KINDS or outcome_kind != ("supported" if supported else "refused"):
                raise ArgumentError("model transport outcome kind does not reconcile with support state")
        model_identity_value = raw.get("model_identity")
        model_identity = None if model_identity_value is None else OncoModelIdentityProjection.from_wire(model_identity_value)
        effect = None if raw.get("effect") is None else _route_text("model transport effect", raw.get("effect"))
        rests_on = _route_strings("model transport rests_on", raw.get("rests_on", []))
        if any(axis not in ONCOWORLDS_MODEL_FIDELITY_AXES for axis in rests_on):
            raise ArgumentError("model transport rests_on contains an unknown fidelity axis")
        fidelity_axes = tuple(OncoModelFidelityProjection.from_wire(item) for item in _array("model transport fidelity_axes", raw.get("fidelity_axes", [])))
        establishment_value = raw.get("establishment")
        establishment = None if establishment_value is None else OncoModelEstablishmentProjection.from_wire(establishment_value)
        replicates_value = raw.get("replicates")
        replicates = None if replicates_value is None else OncoModelReplicateProjection.from_wire(replicates_value)
        transport_assumption_names = _route_strings("model transport assumptions", raw.get("transport_assumption_names", []))
        required_assumptions = _route_strings("model transport required assumptions", raw.get("required_assumptions", []))
        patient_relevant_claim_record = None
        if isinstance(patient_claim_value, Mapping) and (schema is not None or "result" in patient_claim_value):
            patient_relevant_claim_record = OncoPatientRelevantClaimProjection.from_wire(patient_claim_value)
        if schema is not None:
            if "supported" not in raw or "outcome_kind" not in raw:
                raise ArgumentError("versioned model transport projections require support and outcome fields")
            if model_identity is None or effect is None or establishment is None or replicates is None:
                raise ArgumentError("versioned model transport projections require identity, replication, and establishment evidence")
            if "fidelity_axes" not in raw or "transport_assumption_names" not in raw or "required_assumptions" not in raw:
                raise ArgumentError("versioned model transport projections require fidelity and assumption accounting")
            if supported != (patient_relevant_claim_record is not None):
                raise ArgumentError("versioned model transport support does not reconcile with its claim record")
        refusal_value = raw.get("refusal")
        refusal_kind = None
        if refusal_value is not None:
            refusal_kind = _route_text("model transport refusal kind", _object("model transport refusal", refusal_value).get("refusal"))
            if refusal_kind not in ONCOWORLDS_MODEL_REFUSAL_KINDS:
                raise ArgumentError(f"unknown model transport refusal kind: {refusal_kind!r}")
        refusal_kind_value = raw.get("refusal_kind")
        if refusal_kind_value is not None and _route_text("model transport refusal_kind", refusal_kind_value) != refusal_kind:
            raise ArgumentError("model transport refusal_kind does not reconcile with typed refusal")
        if not ok:
            stage, refusal, refusal_text, fail_closed, guarantee = _domain_refusal(raw, "oncoworlds model transport")
            if schema is not None and refusal_kind is None:
                raise ArgumentError("versioned model transport refusals require refusal_kind")
            return cls(raw, False, _optional_text("model statement", raw.get("model_statement")), None, None, stage, refusal, refusal_text, fail_closed, guarantee, (), (), schema, outcome_kind, refusal_kind, model_identity, effect, rests_on, fidelity_axes, establishment, replicates, transport_assumption_names, required_assumptions, None)
        if raw.get("refusal") is not None or raw.get("stage") is not None or raw.get("fail_closed", False):
            raise ArgumentError("successful model transport cannot carry refusal evidence")
        return cls(raw, True, _route_text("model statement", raw.get("model_statement")), _route_count("effective biological n", raw.get("effective_biological_n")), _route_mapping("patient relevant claim", raw.get("patient_relevant_claim")), None, None, None, False, None, _route_strings("model transport guarantees", raw.get("guarantees")), _route_strings("model transport limitations", raw.get("limitations")), schema, outcome_kind, None, model_identity, effect, rests_on, fidelity_axes, establishment, replicates, transport_assumption_names, required_assumptions, patient_relevant_claim_record)

    @property
    def supported(self) -> bool:
        return self.ok and self.patient_relevant_claim is not None


@dataclass(frozen=True)
class OncoWorldsMethylationClassifyReport:
    raw: dict[str, Any]
    ok: bool
    classified: bool | None
    class_label: str | None
    report: dict[str, Any] | None
    stage: str | None
    refusal: dict[str, Any] | None
    refusal_text: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    outcome_kind: str | None = None
    refusal_kind: str | None = None
    classifier: OncoMethylationClassifierProjection | None = None
    classifier_threshold: int | None = None
    threshold_declared: bool = False
    qc: dict[str, Any] | None = None
    tumour_content: dict[str, Any] | None = None
    score_count: int = 0
    score_classes: tuple[str, ...] = ()
    caveat_count: int = 0
    nearest_present: bool = False
    outcome_record: OncoMethylationOutcomeProjection | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsMethylationClassifyReport":
        raw = _payload(value, label="oncoworlds methylation classification", direct_keys=("report", "refusal"))
        ok = _bool("methylation classification ok", raw.get("ok"))
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("methylation classification schema", schema_value)
        if schema is not None and schema != METHYLATION_CLASSIFY_SCHEMA:
            raise ArgumentError(f"unknown methylation classification schema: {schema!r}")
        outcome_kind_value = raw.get("outcome_kind")
        outcome_kind = "refused" if not ok else None
        if outcome_kind_value is not None:
            outcome_kind = _route_text("methylation classification outcome kind", outcome_kind_value)
            if outcome_kind not in METHYLATION_OUTCOME_KINDS or (not ok and outcome_kind != "refused"):
                raise ArgumentError("methylation classification outcome kind does not reconcile with transport state")
        classifier_value = raw.get("classifier")
        classifier = None if classifier_value is None else OncoMethylationClassifierProjection.from_wire(classifier_value)
        threshold_value = raw.get("classifier_threshold")
        classifier_threshold = None if threshold_value is None else _route_count("methylation classifier threshold", threshold_value)
        threshold_declared = _bool("methylation threshold declared", raw.get("threshold_declared", classifier_threshold is not None))
        if threshold_declared != (classifier_threshold is not None):
            raise ArgumentError("methylation threshold declaration does not reconcile with threshold")
        if classifier is not None and classifier.reporting_threshold != classifier_threshold:
            raise ArgumentError("methylation classifier threshold does not reconcile with classifier")
        qc = None if raw.get("qc") is None else _object("methylation qc", raw.get("qc"))
        tumour_content = None if raw.get("tumour_content") is None else _object("methylation tumour content", raw.get("tumour_content"))
        score_count = _route_count("methylation score count", raw.get("score_count", 0))
        score_classes = _route_strings("methylation score classes", raw.get("score_classes", []))
        if score_count != len(score_classes):
            raise ArgumentError("methylation score count does not reconcile with score classes")
        caveat_count_declared = "caveat_count" in raw
        caveat_count = _route_count("methylation caveat count", raw.get("caveat_count", 0))
        nearest_present = _bool("methylation nearest presence", raw.get("nearest_present", False))
        outcome_record = None
        report_value = raw.get("report")
        if report_value is not None:
            report = _object("methylation classification report", report_value)
            if isinstance(report.get("outcome"), Mapping):
                outcome_record = OncoMethylationOutcomeProjection.from_wire(report.get("outcome"))
                if outcome_kind is None:
                    outcome_kind = outcome_record.kind
                if outcome_kind != outcome_record.kind:
                    raise ArgumentError("methylation outcome kind does not reconcile with nested outcome")
                classified = _bool("methylation classified", raw.get("classified"))
                if classified != (outcome_record.kind == "classified"):
                    raise ArgumentError("methylation classified does not reconcile with nested outcome")
                label = None if raw.get("class") is None else _route_text("methylation class", raw.get("class"))
                if label != outcome_record.class_label:
                    raise ArgumentError("methylation class does not reconcile with nested outcome")
            caveats = report.get("caveats", [])
            if not isinstance(caveats, Sequence) or isinstance(caveats, (str, bytes)) or ((caveat_count_declared or schema is not None) and caveat_count != len(caveats)):
                raise ArgumentError("methylation caveat count does not reconcile with report")
            if outcome_record is not None and nearest_present != (outcome_record.nearest is not None):
                raise ArgumentError("methylation nearest presence does not reconcile with outcome")
        if schema is not None:
            if "outcome_kind" not in raw or classifier is None or "score_count" not in raw or "score_classes" not in raw:
                raise ArgumentError("versioned methylation classification requires classifier and score accounting")
        if not ok:
            stage, refusal, refusal_text, fail_closed, guarantee = _domain_refusal(raw, "methylation classification")
            refusal_kind_value = raw.get("refusal_kind")
            refusal_kind = _route_text("methylation refusal kind", refusal_kind_value) if refusal_kind_value is not None else _route_text("methylation refusal kind", refusal.get("refusal"))
            if refusal_kind not in METHYLATION_REFUSAL_KINDS or refusal_kind != refusal.get("refusal"):
                raise ArgumentError("methylation refusal kind does not reconcile with typed refusal")
            if schema is not None and refusal_kind_value is None:
                raise ArgumentError("versioned methylation refusals require refusal_kind")
            return cls(raw, False, None, None, None, stage, refusal, refusal_text, fail_closed, guarantee, (), (), schema, outcome_kind, refusal_kind, classifier, classifier_threshold, threshold_declared, qc, tumour_content, score_count, score_classes, caveat_count, nearest_present, None)
        if raw.get("refusal") is not None or raw.get("stage") is not None or raw.get("fail_closed", False):
            raise ArgumentError("successful methylation classifications cannot carry refusal evidence")
        classified = _bool("methylation classified", raw.get("classified"))
        label = _optional_text("methylation class", raw.get("class"))
        if classified != (label is not None):
            raise ArgumentError("methylation classified and class do not reconcile")
        return cls(raw, True, classified, label, _route_mapping("methylation report", raw.get("report")), None, None, None, False, None, _route_strings("methylation guarantees", raw.get("guarantees")), _route_strings("methylation limitations", raw.get("limitations")), schema, outcome_kind, None, classifier, classifier_threshold, threshold_declared, qc, tumour_content, score_count, score_classes, caveat_count, nearest_present, outcome_record)

    @property
    def unclassifiable(self) -> bool:
        return self.ok and self.classified is False


@dataclass(frozen=True)
class OncoWorldsMethylationCompareReport:
    raw: dict[str, Any]
    ok: bool
    comparison: dict[str, Any]
    left_classifier: dict[str, Any]
    right_classifier: dict[str, Any]
    divergence: str
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    divergence_kind: str | None = None
    classifier_changed: bool = False
    left_outcome_kind: str | None = None
    right_outcome_kind: str | None = None
    stable_evidence_count: int = 0
    divergence_record: OncoMethylationDivergenceProjection | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsMethylationCompareReport":
        raw = _payload(value, label="oncoworlds methylation comparison", direct_keys=("comparison",))
        if not _bool("methylation comparison ok", raw.get("ok")):
            raise ArgumentError("methylation comparison transport projection is not successful")
        comparison = _route_mapping("methylation comparison", raw.get("comparison"))
        divergence_record = OncoMethylationDivergenceProjection.from_wire(comparison.get("divergence"))
        divergence = divergence_record.kind
        divergence_kind_value = raw.get("divergence_kind")
        divergence_kind = divergence if divergence_kind_value is None else _route_text("methylation divergence_kind", divergence_kind_value)
        if divergence_kind != divergence:
            raise ArgumentError("methylation divergence kind does not reconcile with comparison")
        left_classifier = _route_mapping("left classifier", raw.get("left_classifier"))
        right_classifier = _route_mapping("right classifier", raw.get("right_classifier"))
        classifier_changed = _bool("methylation classifier changed", raw.get("classifier_changed", left_classifier != right_classifier))
        if classifier_changed != (left_classifier != right_classifier):
            raise ArgumentError("methylation classifier change does not reconcile with classifier records")
        left_outcome_kind = _optional_text("methylation left outcome kind", raw.get("left_outcome_kind"))
        right_outcome_kind = _optional_text("methylation right outcome kind", raw.get("right_outcome_kind"))
        stable_evidence = comparison.get("stable_evidence", [])
        if not isinstance(stable_evidence, Sequence) or isinstance(stable_evidence, (str, bytes)):
            raise ArgumentError("methylation stable evidence must be an array")
        stable_evidence_count = _route_count("methylation stable evidence count", raw.get("stable_evidence_count", len(stable_evidence)))
        if stable_evidence_count != len(stable_evidence):
            raise ArgumentError("methylation stable evidence count does not reconcile")
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("methylation comparison schema", schema_value)
        if schema is not None and schema != METHYLATION_COMPARE_SCHEMA:
            raise ArgumentError(f"unknown methylation comparison schema: {schema!r}")
        if schema is not None and ("divergence_kind" not in raw or "classifier_changed" not in raw or "stable_evidence_count" not in raw):
            raise ArgumentError("versioned methylation comparisons require divergence and evidence accounting")
        return cls(raw, True, comparison, left_classifier, right_classifier, divergence, _route_strings("methylation comparison guarantees", raw.get("guarantees")), _route_strings("methylation comparison limitations", raw.get("limitations")), schema, divergence_kind, classifier_changed, left_outcome_kind, right_outcome_kind, stable_evidence_count, divergence_record)

    @property
    def version_conditioned(self) -> bool:
        return self.divergence == "version_conditioned"


@dataclass(frozen=True)
class OncoWorldsRadiogenomicCheckReport:
    raw: dict[str, Any]
    ok: bool
    supported_claim: dict[str, Any] | None
    stage: str | None
    refusal: dict[str, Any] | None
    refusal_text: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    outcome_kind: str | None = None
    refusal_kind: str | None = None
    claim_target: str | None = None
    claim_statement: str | None = None
    design: OncoRadiogenomicDesignProjection | None = None
    transport_assumption_names: tuple[str, ...] = ()
    required_assumptions: tuple[str, ...] = ()
    supported_claim_record: OncoRadiogenomicSupportedClaimProjection | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsRadiogenomicCheckReport":
        raw = _payload(value, label="oncoworlds radiogenomic check", direct_keys=("supported_claim", "refusal"))
        ok = _bool("radiogenomic check ok", raw.get("ok"))
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("radiogenomic schema", schema_value)
        if schema is not None and schema != ONCOWORLDS_RADIOGENOMIC_SCHEMA:
            raise ArgumentError(f"unknown radiogenomic schema: {schema!r}")
        supported = _bool("radiogenomic supported", raw.get("supported", ok and raw.get("supported_claim") is not None))
        if supported != ok:
            raise ArgumentError("radiogenomic supported does not reconcile with transport success")
        outcome_kind_value = raw.get("outcome_kind")
        outcome_kind = "supported" if supported else "refused"
        if outcome_kind_value is not None:
            outcome_kind = _route_text("radiogenomic outcome kind", outcome_kind_value)
            if outcome_kind not in ONCOWORLDS_RADIOGENOMIC_OUTCOME_KINDS or outcome_kind != ("supported" if supported else "refused"):
                raise ArgumentError("radiogenomic outcome kind does not reconcile with support state")
        claim_target_value = raw.get("claim_target")
        claim_target = None if claim_target_value is None else _route_text("radiogenomic claim target", claim_target_value)
        if claim_target is not None and claim_target not in ONCOWORLDS_RADIOGENOMIC_TARGETS:
            raise ArgumentError(f"unknown radiogenomic claim target: {claim_target!r}")
        claim_statement = None if raw.get("claim_statement") is None else _route_text("radiogenomic claim statement", raw.get("claim_statement"))
        design_value = raw.get("design")
        design = None if design_value is None else OncoRadiogenomicDesignProjection.from_wire(design_value)
        transport_assumption_names = _route_strings("radiogenomic transport assumptions", raw.get("transport_assumption_names", []))
        required_assumptions = _route_strings("radiogenomic required assumptions", raw.get("required_assumptions", []))
        supported_claim_value = raw.get("supported_claim")
        supported_claim_record = None
        if isinstance(supported_claim_value, Mapping) and (schema is not None or "claim" in supported_claim_value):
            supported_claim_record = OncoRadiogenomicSupportedClaimProjection.from_wire(supported_claim_value)
        if schema is not None and supported != (supported_claim_record is not None):
            raise ArgumentError("radiogenomic support state does not reconcile with supported claim")
        if supported_claim_record is not None:
            if claim_target is not None and claim_target != supported_claim_record.target:
                raise ArgumentError("radiogenomic claim target does not reconcile with supported claim")
            if claim_statement is not None and claim_statement != supported_claim_record.statement:
                raise ArgumentError("radiogenomic claim statement does not reconcile with supported claim")
        refusal_value = raw.get("refusal")
        refusal_kind = None
        if refusal_value is not None:
            refusal_kind = _route_text("radiogenomic refusal kind", _object("radiogenomic refusal", refusal_value).get("refusal"))
            if refusal_kind not in ONCOWORLDS_RADIOGENOMIC_REFUSAL_KINDS:
                raise ArgumentError(f"unknown radiogenomic refusal kind: {refusal_kind!r}")
        refusal_kind_value = raw.get("refusal_kind")
        if refusal_kind_value is not None and _route_text("radiogenomic refusal_kind", refusal_kind_value) != refusal_kind:
            raise ArgumentError("radiogenomic refusal_kind does not reconcile with typed refusal")
        if schema is not None:
            if "supported" not in raw or "outcome_kind" not in raw:
                raise ArgumentError("versioned radiogenomic projections require support and outcome fields")
            if design is None or claim_target is None or claim_statement is None:
                raise ArgumentError("versioned radiogenomic projections require claim and design evidence")
            if "transport_assumption_names" not in raw or "required_assumptions" not in raw:
                raise ArgumentError("versioned radiogenomic projections require assumption accounting")
        if not ok:
            stage, refusal, refusal_text, fail_closed, guarantee = _domain_refusal(raw, "radiogenomic check")
            if schema is not None and refusal_kind is None:
                raise ArgumentError("versioned radiogenomic refusals require refusal_kind")
            return cls(raw, False, None, stage, refusal, refusal_text, fail_closed, guarantee, (), (), schema, outcome_kind, refusal_kind, claim_target, claim_statement, design, transport_assumption_names, required_assumptions, None)
        if raw.get("refusal") is not None or raw.get("stage") is not None or raw.get("fail_closed", False):
            raise ArgumentError("successful radiogenomic checks cannot carry refusal evidence")
        return cls(raw, True, _route_mapping("supported radiogenomic claim", raw.get("supported_claim")), None, None, None, False, None, _route_strings("radiogenomic guarantees", raw.get("guarantees")), _route_strings("radiogenomic limitations", raw.get("limitations")), schema, outcome_kind, None, claim_target, claim_statement, design, transport_assumption_names, required_assumptions, supported_claim_record)

    @property
    def supported(self) -> bool:
        return self.ok and self.supported_claim is not None


@dataclass(frozen=True)
class OncoWorldsClonalHistoryCheckReport:
    raw: dict[str, Any]
    ok: bool
    compatible_count: int
    rejected_count: int
    compatible: tuple[Any, ...]
    rejected: tuple[Any, ...]
    unique_history: dict[str, Any]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    candidate_count: int | None = None
    unique_status: str | None = None
    compatible_records: tuple[OncoClonalHistoryProjection, ...] = ()
    rejected_records: tuple[OncoClonalRejectedHistoryProjection, ...] = ()
    unique_record: OncoClonalUniqueHistoryProjection | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsClonalHistoryCheckReport":
        raw = _payload(value, label="oncoworlds clonal history check", direct_keys=("compatible", "unique_history"))
        if not _bool("clonal history check ok", raw.get("ok")):
            raise ArgumentError("clonal-history check transport projection is not successful")
        compatible = _array("clonal compatible histories", raw.get("compatible"))
        rejected = _array("clonal rejected histories", raw.get("rejected"))
        compatible_count = _route_count("clonal compatible_count", raw.get("compatible_count"))
        rejected_count = _route_count("clonal rejected_count", raw.get("rejected_count"))
        if compatible_count != len(compatible) or rejected_count != len(rejected):
            raise ArgumentError("clonal history counts do not reconcile with retained candidates")
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("clonal history schema", schema_value)
        if schema is not None and schema != ONCOWORLDS_CLONAL_SCHEMA:
            raise ArgumentError(f"unknown clonal history schema: {schema!r}")
        compatible_records = tuple(OncoClonalHistoryProjection.from_wire(item) for item in compatible)
        rejected_records_value = raw.get("rejected_records")
        if rejected_records_value is None:
            rejected_records = tuple(
                OncoClonalRejectedHistoryProjection.from_wire({
                    "history": pair[0],
                    "refusal": pair[1],
                    "refusal_kind": _route_mapping("clonal refusal fallback", pair[1]).get("refusal"),
                })
                for pair in (_array("clonal rejected pair", item) for item in rejected)
                if len(pair) == 2
            )
            if len(rejected_records) != rejected_count:
                raise ArgumentError("clonal rejected fallback records must be history/refusal pairs")
        else:
            rejected_records = tuple(OncoClonalRejectedHistoryProjection.from_wire(item) for item in _array("clonal rejected_records", rejected_records_value))
        if len(rejected_records) != rejected_count:
            raise ArgumentError("clonal rejected_records count does not reconcile")
        unique = _route_mapping("clonal unique_history", raw.get("unique_history"))
        if unique.get("ok") is not True and not isinstance(unique.get("refusal"), Mapping):
            raise ArgumentError("clonal unique_history must retain a typed ambiguity/refusal object")
        unique_status = _route_text("clonal unique_status", raw.get("unique_status", "unique" if unique.get("ok") is True else "ambiguous" if unique.get("refusal", {}).get("refusal") == "ambiguous" else "refused"))
        unique_record = OncoClonalUniqueHistoryProjection.from_wire(unique, unique_status)
        candidate_count = _route_count("clonal candidate_count", raw.get("candidate_count", compatible_count + rejected_count))
        if candidate_count != compatible_count + rejected_count:
            raise ArgumentError("clonal candidate_count does not reconcile")
        return cls(raw, True, compatible_count, rejected_count, compatible, rejected, unique, _route_strings("clonal guarantees", raw.get("guarantees")), _route_strings("clonal limitations", raw.get("limitations")), schema=schema, candidate_count=candidate_count, unique_status=unique_status, compatible_records=compatible_records, rejected_records=rejected_records, unique_record=unique_record)

    @property
    def unique(self) -> bool:
        return self.unique_history.get("ok") is True

    @property
    def ambiguous_or_refused(self) -> bool:
        return not self.unique


@dataclass(frozen=True)
class OncoClonalEvidenceCheckProjection:
    raw: dict[str, Any]
    section: str
    allowed: bool
    outcome_kind: str
    refusal_kind: str | None
    refusal: dict[str, Any] | None
    unique_explanation: str | None = None

    @classmethod
    def from_wire(cls, section: str, value: Mapping[str, Any]) -> "OncoClonalEvidenceCheckProjection":
        raw = _object(f"clonal evidence {section} check", value)
        allowed = _bool(f"clonal evidence {section} allowed", raw.get("allowed"))
        outcome_kind = _route_text(f"clonal evidence {section} outcome kind", raw.get("outcome_kind"))
        refusal_value = raw.get("refusal")
        refusal = None if refusal_value is None else _object(f"clonal evidence {section} refusal", refusal_value)
        refusal_kind_value = raw.get("refusal_kind")
        refusal_kind = None if refusal_kind_value is None else _route_text(f"clonal evidence {section} refusal kind", refusal_kind_value)
        if refusal is not None:
            typed_kind = _route_text(f"clonal evidence {section} typed refusal", refusal.get("refusal"))
            if refusal_kind != typed_kind or typed_kind not in ONCOWORLDS_CLONAL_EVIDENCE_REFUSAL_KINDS:
                raise ArgumentError(f"clonal evidence {section} refusal kind does not reconcile")
        if section == "promotion" and not allowed and refusal_kind not in {"undeclared_sensitivity", "no_region_sampled", "not_an_absence", "copy_number_unknown"}:
            raise ArgumentError("unexpected clonal promotion refusal kind")
        if section == "resistance" and not allowed and refusal_kind != "ambiguous":
            raise ArgumentError("unexpected clonal resistance refusal kind")
        if section == "attribution" and not allowed and refusal_kind != "unsupported_directionality":
            raise ArgumentError("unexpected clonal attribution refusal kind")
        if allowed and (refusal is not None or refusal_kind is not None):
            raise ArgumentError(f"allowed clonal evidence {section} checks cannot carry refusal evidence")
        if not allowed and refusal is None:
            raise ArgumentError(f"refused clonal evidence {section} checks must carry typed refusal evidence")
        unique_value = raw.get("unique_explanation")
        unique_explanation = None if unique_value is None else _route_text(f"clonal evidence {section} unique explanation", unique_value)
        return cls(raw, section, allowed, outcome_kind, refusal_kind, refusal, unique_explanation)


@dataclass(frozen=True)
class OncoWorldsClonalEvidenceCheckReport:
    raw: dict[str, Any]
    ok: bool
    all_admissible: bool
    check_count: int
    refusal_count: int
    checks: dict[str, OncoClonalEvidenceCheckProjection]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    outcome_kind: str | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsClonalEvidenceCheckReport":
        raw = _payload(value, label="oncoworlds clonal evidence check", direct_keys=("checks",))
        ok = _bool("clonal evidence check ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("clonal evidence check transport projection is not successful")
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("clonal evidence schema", schema_value)
        if schema is not None and schema != ONCOWORLDS_CLONAL_EVIDENCE_SCHEMA:
            raise ArgumentError(f"unknown clonal evidence schema: {schema!r}")
        outcome_kind = _route_text("clonal evidence outcome kind", raw.get("outcome_kind", "report"))
        if outcome_kind not in ONCOWORLDS_CLONAL_EVIDENCE_OUTCOME_KINDS:
            raise ArgumentError(f"unknown clonal evidence outcome kind: {outcome_kind!r}")
        raw_checks = _route_mapping("clonal evidence checks", raw.get("checks"))
        if not raw_checks or any(section not in {"promotion", "resistance", "attribution"} for section in raw_checks):
            raise ArgumentError("clonal evidence checks contain an unknown section")
        checks = {section: OncoClonalEvidenceCheckProjection.from_wire(section, check) for section, check in raw_checks.items()}
        check_count = _route_count("clonal evidence check count", raw.get("check_count", len(checks)))
        refusal_count = _route_count("clonal evidence refusal count", raw.get("refusal_count", sum(not check.allowed for check in checks.values())))
        if check_count != len(checks) or refusal_count != sum(not check.allowed for check in checks.values()):
            raise ArgumentError("clonal evidence counts do not reconcile")
        all_admissible = _bool("all clonal evidence checks admissible", raw.get("all_admissible", refusal_count == 0))
        if all_admissible != (refusal_count == 0):
            raise ArgumentError("clonal evidence admissibility does not reconcile")
        if schema is not None and any(field not in raw for field in ("check_count", "refusal_count", "all_admissible")):
            raise ArgumentError("versioned clonal evidence reports require section accounting")
        return cls(raw, True, all_admissible, check_count, refusal_count, checks, _route_strings("clonal evidence guarantees", raw.get("guarantees")), _route_strings("clonal evidence limitations", raw.get("limitations")), schema, outcome_kind)


@dataclass(frozen=True)
class OncoWorldsEraShiftCheckReport:
    raw: dict[str, Any]
    ok: bool
    comparable: bool
    evidence: dict[str, Any]
    left: OncoShiftCohortProjection
    right: OncoShiftCohortProjection
    mapping: dict[str, Any] | None
    assay_contexts: tuple[OncoAssayShiftProjection, ...]
    descriptor_checks: tuple[OncoDescriptorShiftProjection, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    outcome_kind: str | None = None
    refusal_kind: str | None = None
    refusal: dict[str, Any] | None = None
    refusal_text: str | None = None
    fail_closed: bool = False
    same_classification_version: bool = False
    mapping_declared: bool = False
    mapping_fate_count: int = 0
    mapping_versions_match: bool = False

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsEraShiftCheckReport":
        raw = _payload(value, label="OncoWorlds era-shift check", direct_keys=("evidence", "refusal"))
        ok = _bool("OncoWorlds era-shift check ok", raw.get("ok"))
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("OncoWorlds era-shift schema", schema_value)
        if schema is not None and schema != ONCOWORLDS_ERA_SCHEMA:
            raise ArgumentError(f"unknown OncoWorlds era-shift schema: {schema!r}")
        comparable = _bool("OncoWorlds cohorts comparable", raw.get("comparable", ok))
        if comparable != ok:
            raise ArgumentError("OncoWorlds comparability does not reconcile with transport state")
        outcome_kind_value = raw.get("outcome_kind")
        outcome_kind = "comparable" if ok else "refused"
        if outcome_kind_value is not None:
            outcome_kind = _route_text("OncoWorlds era-shift outcome kind", outcome_kind_value)
            if outcome_kind not in ONCOWORLDS_ERA_OUTCOME_KINDS or outcome_kind != ("comparable" if ok else "refused"):
                raise ArgumentError("OncoWorlds era-shift outcome kind does not reconcile")
        evidence = _route_mapping("OncoWorlds era-shift evidence", raw.get("evidence"))
        left = OncoShiftCohortProjection.from_wire(evidence.get("left"))
        right = OncoShiftCohortProjection.from_wire(evidence.get("right"))
        mapping = _optional_mapping("OncoWorlds era-shift mapping", evidence.get("mapping"))
        mapping_declared = _bool("OncoWorlds mapping declared", evidence.get("mapping_declared", mapping is not None))
        if mapping_declared != (mapping is not None):
            raise ArgumentError("OncoWorlds mapping declaration does not reconcile")
        mapping_fate_count = _route_count("OncoWorlds mapping fate count", evidence.get("mapping_fate_count", len(mapping.get("fates", {})) if mapping else 0))
        if mapping is not None and mapping_fate_count != len(mapping.get("fates", {})):
            raise ArgumentError("OncoWorlds mapping fate count does not reconcile")
        mapping_versions_match = _bool("OncoWorlds mapping version match", evidence.get("mapping_versions_match", False))
        same_version = _bool("OncoWorlds same classification version", evidence.get("same_classification_version", left.classification_version == right.classification_version))
        if same_version != (left.classification_version == right.classification_version):
            raise ArgumentError("OncoWorlds same-version evidence does not reconcile with cohorts")
        assays = tuple(OncoAssayShiftProjection.from_wire(item) for item in _array("OncoWorlds assay contexts", evidence.get("assay_contexts", [])))
        descriptors = tuple(OncoDescriptorShiftProjection.from_wire(item) for item in _array("OncoWorlds descriptor checks", evidence.get("descriptor_checks", [])))
        if _route_count("OncoWorlds assay context count", evidence.get("assay_context_count", len(assays))) != len(assays):
            raise ArgumentError("OncoWorlds assay context count does not reconcile")
        if _route_count("OncoWorlds descriptor check count", evidence.get("descriptor_check_count", len(descriptors))) != len(descriptors):
            raise ArgumentError("OncoWorlds descriptor check count does not reconcile")
        refusal_value = raw.get("refusal")
        refusal = None
        refusal_kind = None
        refusal_text = None
        fail_closed = False
        guarantees: tuple[str, ...] = ()
        limitations: tuple[str, ...] = ()
        if not ok:
            stage, refusal, refusal_text, fail_closed, guarantee = _domain_refusal(raw, "OncoWorlds era-shift check")
            if stage != "classification_era_comparability":
                raise ArgumentError("OncoWorlds era-shift refusal stage does not reconcile")
            refusal_kind = _route_text("OncoWorlds era-shift refusal kind", raw.get("refusal_kind", refusal.get("refusal")))
            if refusal_kind not in ONCOWORLDS_ERA_REFUSAL_KINDS or refusal_kind != refusal.get("refusal"):
                raise ArgumentError("OncoWorlds era-shift refusal kind does not reconcile")
            if schema is not None and raw.get("refusal_kind") is None:
                raise ArgumentError("versioned OncoWorlds era-shift refusals require refusal_kind")
            return cls(raw, False, False, evidence, left, right, mapping, assays, descriptors, (), (), schema, outcome_kind, refusal_kind, refusal, refusal_text, fail_closed, same_version, mapping_declared, mapping_fate_count, mapping_versions_match)
        if raw.get("refusal") is not None or raw.get("stage") is not None or raw.get("fail_closed", False):
            raise ArgumentError("successful OncoWorlds era-shift checks cannot carry refusal evidence")
        if schema is not None and ("outcome_kind" not in raw or "mapping_declared" not in evidence or "mapping_fate_count" not in evidence):
            raise ArgumentError("versioned OncoWorlds era-shift checks require mapping accounting")
        return cls(raw, True, True, evidence, left, right, mapping, assays, descriptors, _route_strings("OncoWorlds era-shift guarantees", raw.get("guarantees")), _route_strings("OncoWorlds era-shift limitations", raw.get("limitations")), schema, outcome_kind, None, None, None, False, same_version, mapping_declared, mapping_fate_count, mapping_versions_match)


@dataclass(frozen=True)
class OncoEquitySubgroupProjection:
    raw: dict[str, Any]
    subgroup: str
    n: int
    estimate: float
    interval: dict[str, Any] | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoEquitySubgroupProjection":
        raw = _object("OncoWorlds equity subgroup", value)
        interval_value = raw.get("interval")
        normalized_interval = None
        if interval_value is not None:
            interval = _object("OncoWorlds subgroup uncertainty interval", interval_value)
            low = _number("OncoWorlds subgroup interval low", interval.get("low"))
            high = _number("OncoWorlds subgroup interval high", interval.get("high"))
            if low > high:
                raise ArgumentError("OncoWorlds subgroup interval low must not exceed high")
            normalized_interval = dict(interval)
            normalized_interval["low"] = low
            normalized_interval["high"] = high
        return cls(raw, _route_text("OncoWorlds subgroup", raw.get("subgroup")), _route_count("OncoWorlds subgroup n", raw.get("n")), _number("OncoWorlds subgroup estimate", raw.get("estimate")), normalized_interval)


@dataclass(frozen=True)
class OncoWorldsEquityCheckReport:
    raw: dict[str, Any]
    ok: bool
    equity_supported: bool
    pooled_value: float
    subgroups: tuple[OncoEquitySubgroupProjection, ...]
    subgroup_count: int
    interval_count: int
    all_intervals_present: bool
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    outcome_kind: str | None = None
    refusal_kind: str | None = None
    refusal: dict[str, Any] | None = None
    refusal_text: str | None = None
    fail_closed: bool = False
    report: dict[str, Any] | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsEquityCheckReport":
        raw = _payload(value, label="OncoWorlds equity check", direct_keys=("subgroups", "refusal"))
        ok = _bool("OncoWorlds equity check ok", raw.get("ok"))
        supported = _bool("OncoWorlds equity support", raw.get("equity_supported", ok))
        if supported != ok:
            raise ArgumentError("OncoWorlds equity support does not reconcile with transport state")
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("OncoWorlds equity schema", schema_value)
        if schema is not None and schema != ONCOWORLDS_EQUITY_SCHEMA:
            raise ArgumentError(f"unknown OncoWorlds equity schema: {schema!r}")
        outcome_kind_value = raw.get("outcome_kind")
        outcome_kind = "equity_report" if ok else "refused"
        if outcome_kind_value is not None:
            outcome_kind = _route_text("OncoWorlds equity outcome kind", outcome_kind_value)
            if outcome_kind not in ONCOWORLDS_EQUITY_OUTCOME_KINDS or outcome_kind != ("equity_report" if ok else "refused"):
                raise ArgumentError("OncoWorlds equity outcome kind does not reconcile")
        pooled_value = _number("OncoWorlds pooled value", raw.get("pooled_value"))
        subgroups = tuple(OncoEquitySubgroupProjection.from_wire(item) for item in _array("OncoWorlds equity subgroups", raw.get("subgroups", [])))
        subgroup_count = _route_count("OncoWorlds subgroup count", raw.get("subgroup_count", len(subgroups)))
        interval_count = _route_count("OncoWorlds interval count", raw.get("interval_count", len(subgroups)))
        actual_interval_count = sum(item.interval is not None for item in subgroups)
        if subgroup_count != len(subgroups) or interval_count != actual_interval_count:
            raise ArgumentError("OncoWorlds equity counts do not reconcile with retained subgroups")
        all_intervals_present = _bool("OncoWorlds all intervals present", raw.get("all_intervals_present", interval_count == subgroup_count))
        if all_intervals_present != (interval_count == subgroup_count):
            raise ArgumentError("OncoWorlds interval completeness does not reconcile")
        refusal = None
        refusal_kind = None
        refusal_text = None
        fail_closed = False
        report = None if raw.get("report") is None else _object("OncoWorlds equity report", raw.get("report"))
        if not ok:
            stage, refusal, refusal_text, fail_closed, guarantee = _domain_refusal(raw, "OncoWorlds equity check")
            if stage != "equity_report":
                raise ArgumentError("OncoWorlds equity refusal stage does not reconcile")
            refusal_kind = _route_text("OncoWorlds equity refusal kind", raw.get("refusal_kind", refusal.get("refusal")))
            if refusal_kind not in ONCOWORLDS_EQUITY_REFUSAL_KINDS or refusal_kind != refusal.get("refusal"):
                raise ArgumentError("OncoWorlds equity refusal kind does not reconcile")
            if schema is not None and raw.get("refusal_kind") is None:
                raise ArgumentError("versioned OncoWorlds equity refusals require refusal_kind")
            return cls(raw, False, False, pooled_value, subgroups, subgroup_count, interval_count, all_intervals_present, (), (), schema, outcome_kind, refusal_kind, refusal, refusal_text, fail_closed, report)
        if raw.get("refusal") is not None or raw.get("stage") is not None or raw.get("fail_closed", False):
            raise ArgumentError("successful OncoWorlds equity checks cannot carry refusal evidence")
        if not all_intervals_present:
            raise ArgumentError("successful OncoWorlds equity checks require intervals for every subgroup")
        if schema is not None and ("outcome_kind" not in raw or "subgroup_count" not in raw or "interval_count" not in raw):
            raise ArgumentError("versioned OncoWorlds equity checks require subgroup accounting")
        return cls(raw, True, True, pooled_value, subgroups, subgroup_count, interval_count, all_intervals_present, _route_strings("OncoWorlds equity guarantees", raw.get("guarantees")), _route_strings("OncoWorlds equity limitations", raw.get("limitations")), schema, outcome_kind, None, None, None, False, report)


@dataclass(frozen=True)
class OncoEntityWorldCheckProjection:
    raw: dict[str, Any]
    section: str
    allowed: bool
    refusal_kind: str | None
    refusal: dict[str, Any] | None
    cluster_refusal_kind: str | None = None
    event_refusal_kind: str | None = None
    feasibility_kind: str | None = None

    @classmethod
    def from_wire(cls, section: str, value: Mapping[str, Any]) -> "OncoEntityWorldCheckProjection":
        raw = _object(f"OncoWorlds entity-world {section} check", value)
        allowed = _bool(f"OncoWorlds entity-world {section} allowed", raw.get("allowed"))
        refusal_value = raw.get("refusal")
        refusal = None if refusal_value is None else _object(f"OncoWorlds entity-world {section} refusal", refusal_value)
        refusal_kind_value = raw.get("refusal_kind")
        refusal_kind = None if refusal_kind_value is None else _route_text(f"OncoWorlds entity-world {section} refusal kind", refusal_kind_value)
        cluster_refusal_kind = None if raw.get("cluster_refusal_kind") is None else _route_text("OncoWorlds cluster refusal kind", raw.get("cluster_refusal_kind"))
        event_refusal_kind = None if raw.get("event_refusal_kind") is None else _route_text("OncoWorlds event refusal kind", raw.get("event_refusal_kind"))
        feasibility = raw.get("feasibility_kind")
        feasibility_kind = None if feasibility is None else _route_text("OncoWorlds feasibility kind", feasibility)
        if section == "provenance" and refusal_kind not in {None, "unmodelled_provenance_selection"}:
            raise ArgumentError("unexpected provenance refusal kind")
        if section == "alterations" and refusal_kind not in {None, "mechanism_collapse"}:
            raise ArgumentError("unexpected alteration refusal kind")
        if section == "benchmark" and refusal_kind not in {None, "macro_score_without_counts"}:
            raise ArgumentError("unexpected benchmark refusal kind")
        if section == "lesion_analysis":
            if cluster_refusal_kind not in {None, "undeclared_cluster"} or event_refusal_kind not in {None, "competing_event_as_censoring"}:
                raise ArgumentError("unexpected lesion-analysis refusal kind")
        if allowed and (refusal is not None or refusal_kind is not None or cluster_refusal_kind is not None or event_refusal_kind is not None):
            raise ArgumentError(f"allowed {section} checks cannot carry refusal evidence")
        if not allowed and refusal is None and cluster_refusal_kind is None and event_refusal_kind is None:
            raise ArgumentError(f"refused {section} checks must carry typed refusal evidence")
        return cls(raw, section, allowed, refusal_kind, refusal, cluster_refusal_kind, event_refusal_kind, feasibility_kind)


@dataclass(frozen=True)
class OncoWorldsEntityWorldCheckReport:
    raw: dict[str, Any]
    ok: bool
    all_admissible: bool
    check_count: int
    refusal_count: int
    checks: dict[str, OncoEntityWorldCheckProjection]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    outcome_kind: str | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsEntityWorldCheckReport":
        raw = _payload(value, label="OncoWorlds entity-world check", direct_keys=("checks",))
        ok = _bool("OncoWorlds entity-world check ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("entity-world check transport projection must be successful")
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("OncoWorlds entity-world schema", schema_value)
        if schema is not None and schema != ONCOWORLDS_ENTITY_SCHEMA:
            raise ArgumentError(f"unknown OncoWorlds entity-world schema: {schema!r}")
        outcome_kind = _route_text("OncoWorlds entity-world outcome kind", raw.get("outcome_kind", "report"))
        if outcome_kind not in ONCOWORLDS_ENTITY_OUTCOME_KINDS:
            raise ArgumentError(f"unknown OncoWorlds entity-world outcome kind: {outcome_kind!r}")
        raw_checks = _route_mapping("OncoWorlds entity-world checks", raw.get("checks"))
        checks = {section: OncoEntityWorldCheckProjection.from_wire(section, check) for section, check in raw_checks.items()}
        if not checks or any(section not in {"provenance", "alterations", "benchmark", "lesion_analysis"} for section in checks):
            raise ArgumentError("OncoWorlds entity-world checks contain an unknown section")
        check_count = _route_count("OncoWorlds entity-world check count", raw.get("check_count", len(checks)))
        refusal_count = _route_count("OncoWorlds entity-world refusal count", raw.get("refusal_count", sum(not check.allowed for check in checks.values())))
        if check_count != len(checks) or refusal_count != sum(not check.allowed for check in checks.values()):
            raise ArgumentError("OncoWorlds entity-world counts do not reconcile")
        all_admissible = _bool("OncoWorlds all entity-world checks admissible", raw.get("all_admissible", refusal_count == 0))
        if all_admissible != (refusal_count == 0):
            raise ArgumentError("OncoWorlds entity-world admissibility does not reconcile")
        if schema is not None and ("check_count" not in raw or "refusal_count" not in raw or "all_admissible" not in raw):
            raise ArgumentError("versioned OncoWorlds entity-world reports require section accounting")
        return cls(raw, True, all_admissible, check_count, refusal_count, checks, _route_strings("OncoWorlds entity-world guarantees", raw.get("guarantees")), _route_strings("OncoWorlds entity-world limitations", raw.get("limitations")), schema, outcome_kind)


def oncoworlds_model_transport_report(value: Mapping[str, Any]) -> OncoWorldsModelTransportReport:
    return OncoWorldsModelTransportReport.from_wire(value)


def oncoworlds_methylation_classify_report(value: Mapping[str, Any]) -> OncoWorldsMethylationClassifyReport:
    return OncoWorldsMethylationClassifyReport.from_wire(value)


def oncoworlds_methylation_compare_report(value: Mapping[str, Any]) -> OncoWorldsMethylationCompareReport:
    return OncoWorldsMethylationCompareReport.from_wire(value)


def oncoworlds_radiogenomic_check_report(value: Mapping[str, Any]) -> OncoWorldsRadiogenomicCheckReport:
    return OncoWorldsRadiogenomicCheckReport.from_wire(value)


def oncoworlds_clonal_history_check_report(value: Mapping[str, Any]) -> OncoWorldsClonalHistoryCheckReport:
    return OncoWorldsClonalHistoryCheckReport.from_wire(value)


def oncoworlds_clonal_evidence_check_report(value: Mapping[str, Any]) -> OncoWorldsClonalEvidenceCheckReport:
    return OncoWorldsClonalEvidenceCheckReport.from_wire(value)


def oncoworlds_era_shift_check_report(value: Mapping[str, Any]) -> OncoWorldsEraShiftCheckReport:
    return OncoWorldsEraShiftCheckReport.from_wire(value)


def oncoworlds_equity_check_report(value: Mapping[str, Any]) -> OncoWorldsEquityCheckReport:
    return OncoWorldsEquityCheckReport.from_wire(value)


def oncoworlds_entity_world_check_report(value: Mapping[str, Any]) -> OncoWorldsEntityWorldCheckReport:
    return OncoWorldsEntityWorldCheckReport.from_wire(value)


__all__ = [
    "METHYLATION_DIVERGENCES",
    "METHYLATION_CLASSIFY_SCHEMA",
    "METHYLATION_COMPARE_SCHEMA",
    "METHYLATION_OUTCOME_KINDS",
    "METHYLATION_REFUSAL_KINDS",
    "ONCOWORLDS_CLONAL_REFUSAL_KINDS",
    "ONCOWORLDS_CLONAL_SCHEMA",
    "ONCOWORLDS_CLONAL_UNIQUE_STATUSES",
    "ONCOWORLDS_CLONAL_EVIDENCE_SCHEMA",
    "ONCOWORLDS_CLONAL_EVIDENCE_OUTCOME_KINDS",
    "ONCOWORLDS_CLONAL_EVIDENCE_REFUSAL_KINDS",
    "ONCOWORLDS_RADIOGENOMIC_FEATURE_PROVENANCE",
    "ONCOWORLDS_RADIOGENOMIC_OUTCOME_KINDS",
    "ONCOWORLDS_RADIOGENOMIC_REFUSAL_KINDS",
    "ONCOWORLDS_RADIOGENOMIC_SCHEMA",
    "ONCOWORLDS_RADIOGENOMIC_SPLIT_UNITS",
    "ONCOWORLDS_RADIOGENOMIC_TARGETS",
    "ONCOWORLDS_MODEL_FIDELITY_AXES",
    "ONCOWORLDS_MODEL_OUTCOME_KINDS",
    "ONCOWORLDS_MODEL_REFUSAL_KINDS",
    "ONCOWORLDS_MODEL_SCHEMA",
    "ONCOWORLDS_ERA_OUTCOME_KINDS",
    "ONCOWORLDS_ERA_REFUSAL_KINDS",
    "ONCOWORLDS_ERA_SCHEMA",
    "ONCOWORLDS_EQUITY_OUTCOME_KINDS",
    "ONCOWORLDS_EQUITY_REFUSAL_KINDS",
    "ONCOWORLDS_EQUITY_SCHEMA",
    "ONCOWORLDS_ENTITY_OUTCOME_KINDS",
    "ONCOWORLDS_ENTITY_REFUSAL_KINDS",
    "ONCOWORLDS_ENTITY_SCHEMA",
    "OncoClonalHistoryProjection",
    "OncoClonalRejectedHistoryProjection",
    "OncoClonalUniqueHistoryProjection",
    "OncoWorldsClonalHistoryCheckArgs",
    "OncoWorldsClonalHistoryCheckReport",
    "OncoClonalEvidenceCheckArgs",
    "OncoClonalEvidenceCheckProjection",
    "OncoWorldsClonalEvidenceCheckReport",
    "OncoWorldsMethylationClassifyArgs",
    "OncoWorldsMethylationClassifyReport",
    "OncoWorldsMethylationCompareArgs",
    "OncoWorldsMethylationCompareReport",
    "OncoMethylationClassifierProjection",
    "OncoMethylationDivergenceProjection",
    "OncoMethylationOutcomeProjection",
    "OncoWorldsModelTransportArgs",
    "OncoWorldsModelTransportReport",
    "OncoModelEstablishmentProjection",
    "OncoModelFidelityProjection",
    "OncoModelIdentityProjection",
    "OncoModelReplicateProjection",
    "OncoPatientRelevantClaimProjection",
    "OncoRadiogenomicDesignProjection",
    "OncoRadiogenomicSupportedClaimProjection",
    "OncoWorldsRadiogenomicCheckArgs",
    "OncoWorldsRadiogenomicCheckReport",
    "OncoWorldsEraShiftCheckArgs",
    "OncoWorldsEraShiftCheckReport",
    "OncoShiftCohortProjection",
    "OncoAssayShiftProjection",
    "OncoDescriptorShiftProjection",
    "OncoWorldsEquityCheckArgs",
    "OncoEquitySubgroupProjection",
    "OncoWorldsEquityCheckReport",
    "OncoWorldsEntityWorldCheckArgs",
    "OncoEntityWorldCheckProjection",
    "OncoWorldsEntityWorldCheckReport",
    "oncoworlds_clonal_history_check_report",
    "oncoworlds_clonal_evidence_check_report",
    "oncoworlds_era_shift_check_report",
    "oncoworlds_equity_check_report",
    "oncoworlds_entity_world_check_report",
    "oncoworlds_methylation_classify_report",
    "oncoworlds_methylation_compare_report",
    "oncoworlds_model_transport_report",
    "oncoworlds_radiogenomic_check_report",
]
