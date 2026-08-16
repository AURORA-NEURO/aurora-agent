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

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsModelTransportReport":
        raw = _payload(value, label="oncoworlds model transport", direct_keys=("patient_relevant_claim", "refusal"))
        ok = _bool("oncoworlds model transport ok", raw.get("ok"))
        if not ok:
            stage, refusal, refusal_text, fail_closed, guarantee = _domain_refusal(raw, "oncoworlds model transport")
            return cls(raw, False, _optional_text("model statement", raw.get("model_statement")), None, None, stage, refusal, refusal_text, fail_closed, guarantee, (), ())
        if raw.get("refusal") is not None or raw.get("stage") is not None or raw.get("fail_closed", False):
            raise ArgumentError("successful model transport cannot carry refusal evidence")
        return cls(raw, True, _route_text("model statement", raw.get("model_statement")), _route_count("effective biological n", raw.get("effective_biological_n")), _route_mapping("patient relevant claim", raw.get("patient_relevant_claim")), None, None, None, False, None, _route_strings("model transport guarantees", raw.get("guarantees")), _route_strings("model transport limitations", raw.get("limitations")))

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

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldsRadiogenomicCheckReport":
        raw = _payload(value, label="oncoworlds radiogenomic check", direct_keys=("supported_claim", "refusal"))
        ok = _bool("radiogenomic check ok", raw.get("ok"))
        if not ok:
            stage, refusal, refusal_text, fail_closed, guarantee = _domain_refusal(raw, "radiogenomic check")
            return cls(raw, False, None, stage, refusal, refusal_text, fail_closed, guarantee, (), ())
        if raw.get("refusal") is not None or raw.get("stage") is not None or raw.get("fail_closed", False):
            raise ArgumentError("successful radiogenomic checks cannot carry refusal evidence")
        return cls(raw, True, _route_mapping("supported radiogenomic claim", raw.get("supported_claim")), None, None, None, False, None, _route_strings("radiogenomic guarantees", raw.get("guarantees")), _route_strings("radiogenomic limitations", raw.get("limitations")))

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
    "OncoWorldsRadiogenomicCheckArgs",
    "OncoWorldsRadiogenomicCheckReport",
    "oncoworlds_clonal_history_check_report",
    "oncoworlds_methylation_classify_report",
    "oncoworlds_methylation_compare_report",
    "oncoworlds_model_transport_report",
    "oncoworlds_radiogenomic_check_report",
]
