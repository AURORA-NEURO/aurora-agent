"""Typed pre-analytic fault application contracts.

Pre-analytic mutation is intentionally not an ordinary success/failure boolean.  A successful
application must preserve the biological digest while carrying an observable QC or measurability
signature; response availability, family false-positive validation, and detectability are separate
evidence channels.  Structured refusals remain inspectable and fail closed.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


PREANALYTIC_STAGES = frozenset({"collection", "preservation", "protocol", "instrument", "batch", "qc", "processing"})
PREANALYTIC_RESPONSES = frozenset({"detect", "correct", "abstain", "select_confirmatory"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ArgumentError(f"{name} must be an integer")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _mapping_array(name: str, value: Any) -> tuple[dict[str, Any], ...]:
    return tuple(_route_mapping(f"{name}[{index}]", item) for index, item in enumerate(_array(name, value)))


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("pre-analytic response", value)
    required = ("ok", "applied", "mutation")
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
                    raise ArgumentError(f"pre-analytic response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping("decoded pre-analytic response", decoded)
                if all(key in decoded_mapping for key in required):
                    return decoded_mapping
    raise ArgumentError("response does not contain a pre-analytic projection")


@dataclass(frozen=True)
class PreanalyticApplyArgs:
    specimen: Mapping[str, Any]
    mutation: Mapping[str, Any]
    available_actions: tuple[str, ...] | None = None
    family: tuple[Mapping[str, Any], ...] | None = None
    family_name: str | None = None
    qc_field: str | None = None
    alert_at: int | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PreanalyticApplyArgs":
        raw = _route_mapping("pre-analytic arguments", value)
        available = raw.get("available_actions")
        family = raw.get("family")
        return cls(
            raw.get("specimen"),
            raw.get("mutation"),
            None if available is None else tuple(_route_text(f"available_actions[{index}]", item) for index, item in enumerate(_array("available_actions", available))),
            None if family is None else _mapping_array("pre-analytic family", family),
            _optional_text("pre-analytic family_name", raw.get("family_name")),
            _optional_text("pre-analytic qc_field", raw.get("qc_field")),
            raw.get("alert_at"),
        )

    def __post_init__(self) -> None:
        specimen = _route_mapping("pre-analytic specimen", self.specimen)
        mutation = _route_mapping("pre-analytic mutation", self.mutation)
        edits = mutation.get("edits", [])
        if len(_array("pre-analytic mutation edits", edits)) > 100:
            raise ArgumentError("pre-analytic mutation may contain at most 100 edits")
        if self.family is not None and len(self.family) > 100:
            raise ArgumentError("pre-analytic family may contain at most 100 mutations")
        if self.alert_at is not None and (isinstance(self.alert_at, bool) or not isinstance(self.alert_at, int) or self.alert_at < 0):
            raise ArgumentError("pre-analytic alert_at must be a non-negative integer")
        object.__setattr__(self, "specimen", specimen)
        object.__setattr__(self, "mutation", mutation)

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"specimen": dict(self.specimen), "mutation": dict(self.mutation)}
        if self.available_actions is not None:
            result["available_actions"] = list(self.available_actions)
        if self.family is not None:
            result["family"] = [dict(item) for item in self.family]
        if self.family_name is not None:
            result["family_name"] = self.family_name
        if self.qc_field is not None:
            result["qc_field"] = self.qc_field
        if self.alert_at is not None:
            result["alert_at"] = self.alert_at
        return result


@dataclass(frozen=True)
class PreanalyticResponseCheckReport:
    raw: dict[str, Any]
    ok: bool
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PreanalyticResponseCheckReport":
        raw = _route_mapping("pre-analytic response check", value)
        ok = _bool("pre-analytic response check ok", raw.get("ok"))
        refusal = _optional_text("pre-analytic response check refusal", raw.get("refusal"))
        if ok:
            if refusal is not None or raw.get("fail_closed", False):
                raise ArgumentError("successful pre-analytic response checks cannot be refusals")
            fail_closed = False
        else:
            if refusal is None:
                raise ArgumentError("refused pre-analytic response checks require a refusal")
            fail_closed = _bool("pre-analytic response check fail_closed", raw.get("fail_closed"))
            if not fail_closed:
                raise ArgumentError("refused pre-analytic response checks must fail closed")
        return cls(raw, ok, refusal, fail_closed)


@dataclass(frozen=True)
class PreanalyticFamilyValidationReport:
    raw: dict[str, Any]
    ok: bool
    family: str
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PreanalyticFamilyValidationReport":
        raw = _route_mapping("pre-analytic family validation", value)
        ok = _bool("pre-analytic family validation ok", raw.get("ok"))
        family = _route_text("pre-analytic family validation family", raw.get("family"))
        refusal = _optional_text("pre-analytic family validation refusal", raw.get("refusal"))
        if ok:
            if refusal is not None or raw.get("fail_closed", False):
                raise ArgumentError("successful pre-analytic family validation cannot be a refusal")
            fail_closed = False
        else:
            if refusal is None:
                raise ArgumentError("refused pre-analytic family validation requires a refusal")
            fail_closed = _bool("pre-analytic family validation fail_closed", raw.get("fail_closed"))
            if not fail_closed:
                raise ArgumentError("refused pre-analytic family validation must fail closed")
        return cls(raw, ok, family, refusal, fail_closed)


@dataclass(frozen=True)
class PreanalyticDetectabilityReport:
    raw: dict[str, Any]
    qc_field: str
    alert_at: int
    intensity: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PreanalyticDetectabilityReport":
        raw = _route_mapping("pre-analytic detectability", value)
        alert_at = _route_count("pre-analytic detectability alert_at", raw.get("alert_at"))
        intensity = _route_count("pre-analytic detectability intensity", raw.get("intensity"))
        if intensity > 10_000:
            raise ArgumentError("pre-analytic detectability intensity must be at most 10000")
        return cls(raw, _route_text("pre-analytic detectability qc_field", raw.get("qc_field")), alert_at, intensity)


@dataclass(frozen=True)
class PreanalyticFaultedReport:
    raw: dict[str, Any]
    mutation: str
    specimen: Mapping[str, Any]
    qc_signature: Mapping[str, int]
    measurability_lost: Mapping[str, int]
    stage: str
    has_signature: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PreanalyticFaultedReport":
        raw = _route_mapping("pre-analytic faulted specimen", value)
        stage = _route_text("pre-analytic faulted stage", raw.get("stage"))
        if stage not in PREANALYTIC_STAGES:
            raise ArgumentError(f"unknown pre-analytic stage: {stage!r}")
        specimen = _route_mapping("pre-analytic faulted specimen record", raw.get("specimen"))
        qc_raw = _route_mapping("pre-analytic QC signature", raw.get("qc_signature"))
        meas_raw = _route_mapping("pre-analytic measurability loss", raw.get("measurability_lost"))
        qc: dict[str, int] = {}
        for key, value_item in qc_raw.items():
            qc[_route_text("pre-analytic QC field", key)] = _integer("pre-analytic QC delta", value_item)
        meas: dict[str, int] = {}
        for key, value_item in meas_raw.items():
            meas[_route_text("pre-analytic measurability axis", key)] = _route_count("pre-analytic measurability loss", value_item)
        has_signature = _bool("pre-analytic has_signature", raw.get("has_signature", bool(qc or meas)))
        if has_signature != bool(qc or meas):
            raise ArgumentError("pre-analytic has_signature does not reconcile with fault signature")
        return cls(raw, _route_text("pre-analytic faulted mutation", raw.get("mutation")), specimen, qc, meas, stage, has_signature)


@dataclass(frozen=True)
class PreanalyticApplyReport:
    raw: dict[str, Any]
    ok: bool
    applied: bool
    mutation: Mapping[str, Any]
    stage: str | None
    faulted: PreanalyticFaultedReport | None
    biology_digest_before: str
    biology_digest_after: str | None
    biology_unchanged: bool | None
    specimen_digest_before: str
    specimen_digest_after: str | None
    has_signature: bool | None
    response_check: PreanalyticResponseCheckReport | None
    family_validation: PreanalyticFamilyValidationReport | None
    detectability: PreanalyticDetectabilityReport | None
    refusal: str | None
    fail_closed: bool
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PreanalyticApplyReport":
        raw = _payload(value)
        ok = _bool("pre-analytic ok", raw.get("ok"))
        applied = _bool("pre-analytic applied", raw.get("applied"))
        if ok != applied:
            raise ArgumentError("pre-analytic ok and applied must have parity")
        mutation = _route_mapping("pre-analytic returned mutation", raw.get("mutation"))
        refusal = _optional_text("pre-analytic refusal", raw.get("refusal"))
        fail_closed_value = raw.get("fail_closed", False)
        fail_closed = _bool("pre-analytic fail_closed", fail_closed_value)
        response_value = raw.get("response_check")
        family_value = raw.get("family_validation")
        detectability_value = raw.get("detectability")
        response = None if response_value is None else PreanalyticResponseCheckReport.from_wire(response_value)
        family = None if family_value is None else PreanalyticFamilyValidationReport.from_wire(family_value)
        detectability = None if detectability_value is None else PreanalyticDetectabilityReport.from_wire(detectability_value)
        biology_before = _route_text("pre-analytic biology_digest_before", raw.get("biology_digest_before"))
        specimen_before = _route_text("pre-analytic specimen_digest_before", raw.get("specimen_digest_before"))
        if applied:
            if refusal is not None or fail_closed:
                raise ArgumentError("applied pre-analytic reports cannot carry fail-closed refusal evidence")
            faulted = PreanalyticFaultedReport.from_wire(raw.get("faulted"))
            stage = _route_text("pre-analytic stage", raw.get("stage"))
            if stage != faulted.stage:
                raise ArgumentError("pre-analytic stage does not reconcile with faulted record")
            after_biology = _route_text("pre-analytic biology_digest_after", raw.get("biology_digest_after"))
            biology_unchanged = _bool("pre-analytic biology_unchanged", raw.get("biology_unchanged"))
            if biology_unchanged != (biology_before == after_biology):
                raise ArgumentError("pre-analytic biology_unchanged does not reconcile with digests")
            after_specimen = _route_text("pre-analytic specimen_digest_after", raw.get("specimen_digest_after"))
            has_signature = _bool("pre-analytic has_signature", raw.get("has_signature"))
            if has_signature != faulted.has_signature:
                raise ArgumentError("pre-analytic has_signature does not reconcile with faulted specimen")
            return cls(raw, True, True, mutation, stage, faulted, biology_before, after_biology, biology_unchanged, specimen_before, after_specimen, has_signature, response, family, detectability, None, False, _route_strings("pre-analytic guarantees", raw.get("guarantees")), _route_strings("pre-analytic limitations", raw.get("limitations")))
        if refusal is None or not fail_closed:
            raise ArgumentError("refused pre-analytic reports require a fail-closed refusal")
        if raw.get("faulted") is not None or raw.get("biology_digest_after") is not None or raw.get("specimen_digest_after") is not None:
            raise ArgumentError("refused pre-analytic reports cannot claim an admitted post-state")
        return cls(raw, False, False, mutation, None, None, biology_before, None, None, specimen_before, None, None, response, family, detectability, refusal, True, (), ())

    @property
    def refused(self) -> bool:
        return not self.applied

    @property
    def biology_preserved(self) -> bool:
        return self.biology_unchanged is True


def preanalytic_apply_report(value: Mapping[str, Any]) -> PreanalyticApplyReport:
    """Parse direct MCP or HTTP pre-analytic output, including structured refusals."""

    return PreanalyticApplyReport.from_wire(value)


__all__ = [
    "PREANALYTIC_RESPONSES",
    "PREANALYTIC_STAGES",
    "PreanalyticApplyArgs",
    "PreanalyticApplyReport",
    "PreanalyticDetectabilityReport",
    "PreanalyticFamilyValidationReport",
    "PreanalyticFaultedReport",
    "PreanalyticResponseCheckReport",
    "preanalytic_apply_report",
]
