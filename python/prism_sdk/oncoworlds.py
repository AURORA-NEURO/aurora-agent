"""Typed SDK projections for the deeper OncoWorlds contracts.

These helpers keep model-system transport, methylation classification, radiogenomic evaluation,
and clonal-history reasoning separate.  They validate only the top-level request/result shape and
cross-field evidence invariants; the Rust ``bioprism-oncoworlds`` crate remains authoritative for
identity, fidelity, calibration, split safety, scope transport, and cellular-fraction arithmetic.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


METHYLATION_DIVERGENCES = frozenset({"agree", "both_unclassifiable", "version_conditioned"})
ONCOWORLDS_CLONAL_SCHEMA = "bioprism-mcp/oncoworlds-clonal-history-check/0.1"
ONCOWORLDS_CLONAL_REFUSAL_KINDS = frozenset({"fractions_exceed_whole", "child_exceeds_parent", "cyclic", "unknown_subclone", "ambiguous", "unsupported_directionality"})
ONCOWORLDS_CLONAL_UNIQUE_STATUSES = frozenset({"unique", "ambiguous", "refused"})
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

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsMethylationClassifyReport":
        raw = _payload(value, label="oncoworlds methylation classification", direct_keys=("report", "refusal"))
        ok = _bool("methylation classification ok", raw.get("ok"))
        if not ok:
            stage, refusal, refusal_text, fail_closed, guarantee = _domain_refusal(raw, "methylation classification")
            return cls(raw, False, None, None, None, stage, refusal, refusal_text, fail_closed, guarantee, (), ())
        if raw.get("refusal") is not None or raw.get("stage") is not None or raw.get("fail_closed", False):
            raise ArgumentError("successful methylation classifications cannot carry refusal evidence")
        classified = _bool("methylation classified", raw.get("classified"))
        label = _optional_text("methylation class", raw.get("class"))
        if classified != (label is not None):
            raise ArgumentError("methylation classified and class do not reconcile")
        return cls(raw, True, classified, label, _route_mapping("methylation report", raw.get("report")), None, None, None, False, None, _route_strings("methylation guarantees", raw.get("guarantees")), _route_strings("methylation limitations", raw.get("limitations")))

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

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsMethylationCompareReport":
        raw = _payload(value, label="oncoworlds methylation comparison", direct_keys=("comparison",))
        if not _bool("methylation comparison ok", raw.get("ok")):
            raise ArgumentError("methylation comparison transport projection is not successful")
        comparison = _route_mapping("methylation comparison", raw.get("comparison"))
        divergence = _route_text("methylation divergence", _route_mapping("methylation divergence object", comparison.get("divergence")).get("divergence"))
        if divergence not in METHYLATION_DIVERGENCES:
            raise ArgumentError(f"unknown methylation divergence: {divergence!r}")
        return cls(raw, True, comparison, _route_mapping("left classifier", raw.get("left_classifier")), _route_mapping("right classifier", raw.get("right_classifier")), divergence, _route_strings("methylation comparison guarantees", raw.get("guarantees")), _route_strings("methylation comparison limitations", raw.get("limitations")))

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


__all__ = [
    "METHYLATION_DIVERGENCES",
    "ONCOWORLDS_CLONAL_REFUSAL_KINDS",
    "ONCOWORLDS_CLONAL_SCHEMA",
    "ONCOWORLDS_CLONAL_UNIQUE_STATUSES",
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
    "OncoClonalHistoryProjection",
    "OncoClonalRejectedHistoryProjection",
    "OncoClonalUniqueHistoryProjection",
    "OncoWorldsClonalHistoryCheckArgs",
    "OncoWorldsClonalHistoryCheckReport",
    "OncoWorldsMethylationClassifyArgs",
    "OncoWorldsMethylationClassifyReport",
    "OncoWorldsMethylationCompareArgs",
    "OncoWorldsMethylationCompareReport",
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
    "oncoworlds_clonal_history_check_report",
    "oncoworlds_methylation_classify_report",
    "oncoworlds_methylation_compare_report",
    "oncoworlds_model_transport_report",
    "oncoworlds_radiogenomic_check_report",
]
