"""Typed bioethics, research-boundary, and release-gate projections.

The bioethics crates deliberately expose asymmetric states: review required is not an exemption,
an external referral is not an executed action, an assessed empty misuse set is not an unassessed
set, and complete validation evidence is not the same thing as a caller asserting verification.
These SDK types retain those states across MCP and HTTP transports while leaving the policy
decisions in Rust.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


ENGAGEMENT_KINDS = frozenset(
    {
        "identifiable_or_coded_data",
        "interaction_with_participants",
        "prospective_data_collection",
        "expert_performance_study",
        "clinical_workflow_observation",
        "secondary_research",
    }
)
RETURN_OF_RESULTS = frozenset({"not_returned", "aggregate_to_participants", "individual_findings"})
MISUSE_SURFACES = frozenset(
    {
        "sequence_design",
        "pathogen_relevant_analysis",
        "experimental_execution_automation",
        "screening_evasion",
        "toxin_or_virulence_optimisation",
        "sensitive_literature_synthesis",
    }
)
WITHHOLD_SCOPES = frozenset({"exploit_detail", "existence"})
VALIDATION_EVIDENCE_KINDS = frozenset(
    {
        "requirements_and_risk_file",
        "design_review",
        "unit_and_conformance_tests",
        "scientific_validation",
        "security_and_privacy_review",
        "independent_reproduction",
        "change_control",
    }
)
REPRESENTATION_COVERAGE = frozenset({"measured", "unmeasured", "suppressed_small_group"})


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


def _refusal(raw: Mapping[str, Any], label: str) -> tuple[str, str, str | None]:
    stage = _route_text(f"{label} stage", raw.get("stage"))
    refusal = _route_text(f"{label} refusal", raw.get("refusal"))
    if not _bool(f"{label} fail_closed", raw.get("fail_closed")):
        raise ArgumentError(f"refused {label} results must be fail-closed")
    return stage, refusal, _optional_text(f"{label} guarantee", raw.get("guarantee"))


def _object(name: str, value: Mapping[str, Any]) -> dict[str, Any]:
    return _route_mapping(name, value)


@dataclass(frozen=True)
class BioethicsActionReviewArgs:
    plan: Mapping[str, Any]
    boundary: Mapping[str, Any] | None = None
    authorisation: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "plan", _object("bioethics action plan", self.plan))
        object.__setattr__(self, "boundary", _optional_mapping("bioethics action boundary", self.boundary))
        object.__setattr__(self, "authorisation", _optional_mapping("bioethics action authorisation", self.authorisation))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioethicsActionReviewArgs":
        raw = _object("bioethics action arguments", value)
        return cls(raw.get("plan"), raw.get("boundary"), raw.get("authorisation"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"plan": dict(self.plan)}
        for key, value in (("boundary", self.boundary), ("authorisation", self.authorisation)):
            if value is not None:
                result[key] = dict(value)
        return result


@dataclass(frozen=True)
class HumanSubjectScreenArgs:
    study: Mapping[str, Any]
    consent: Mapping[str, Any] | None = None
    at: str | None = None
    boundary: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "study", _object("human-subject study", self.study))
        object.__setattr__(self, "consent", _optional_mapping("human-subject consent", self.consent))
        if (self.consent is None) != (self.at is None):
            raise ArgumentError("human-subject consent and at must be supplied together")
        object.__setattr__(self, "at", _optional_text("human-subject consent at", self.at))
        object.__setattr__(self, "boundary", _optional_mapping("human-subject boundary", self.boundary))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HumanSubjectScreenArgs":
        raw = _object("human-subject arguments", value)
        return cls(raw.get("study"), raw.get("consent"), raw.get("at"), raw.get("boundary"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"study": dict(self.study)}
        for key, value in (("consent", self.consent), ("at", self.at), ("boundary", self.boundary)):
            if value is not None:
                result[key] = dict(value) if isinstance(value, Mapping) else value
        return result


@dataclass(frozen=True)
class BioethicsDualUseReviewArgs:
    release: Mapping[str, Any]
    risk: Mapping[str, Any]
    withhold: str | None = None
    finding: str | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "release", _object("dual-use release", self.release))
        object.__setattr__(self, "risk", _object("dual-use risk assessment", self.risk))
        normalized_withhold = _optional_text("dual-use withhold", self.withhold)
        if normalized_withhold is not None and normalized_withhold not in WITHHOLD_SCOPES:
            raise ArgumentError(f"unknown dual-use withholding scope: {normalized_withhold!r}")
        object.__setattr__(self, "withhold", normalized_withhold)
        object.__setattr__(self, "finding", _optional_text("dual-use finding", self.finding))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioethicsDualUseReviewArgs":
        raw = _object("dual-use arguments", value)
        return cls(raw.get("release"), raw.get("risk"), raw.get("withhold"), raw.get("finding"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"release": dict(self.release), "risk": dict(self.risk)}
        if self.withhold is not None:
            result["withhold"] = self.withhold
        if self.finding is not None:
            result["finding"] = self.finding
        return result


@dataclass(frozen=True)
class BioethicsValidationCheckArgs:
    dossier: Mapping[str, Any]

    def __post_init__(self) -> None:
        object.__setattr__(self, "dossier", _object("validation dossier", self.dossier))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioethicsValidationCheckArgs":
        raw = _object("validation arguments", value)
        return cls(raw.get("dossier"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"dossier": dict(self.dossier)}


@dataclass(frozen=True)
class BioethicsRepresentationAuditArgs:
    subject: str
    observations: tuple[Mapping[str, Any], ...]
    attribution: Mapping[str, Any] | None = None

    def __init__(self, subject: str, observations: Sequence[Mapping[str, Any]], attribution: Mapping[str, Any] | None = None) -> None:
        normalized_subject = _route_text("representation subject", subject)
        if not isinstance(observations, Sequence) or isinstance(observations, (str, bytes)):
            raise ArgumentError("representation observations must be an array")
        normalized_observations = tuple(_object(f"representation observations[{index}]", item) for index, item in enumerate(observations))
        if len(normalized_observations) > 10_000:
            raise ArgumentError("representation observations may contain at most 10000 strata")
        object.__setattr__(self, "subject", normalized_subject)
        object.__setattr__(self, "observations", normalized_observations)
        object.__setattr__(self, "attribution", _optional_mapping("representation attribution", attribution))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioethicsRepresentationAuditArgs":
        raw = _object("representation arguments", value)
        return cls(raw.get("subject"), raw.get("observations"), raw.get("attribution"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"subject": self.subject, "observations": [dict(item) for item in self.observations]}
        if self.attribution is not None:
            result["attribution"] = dict(self.attribution)
        return result


@dataclass(frozen=True)
class BioethicsActionReviewReport:
    raw: dict[str, Any]
    ok: bool
    subject: str | None
    declared_use: str | None
    permitted_uses: tuple[str, ...]
    disposition: dict[str, Any] | None
    physical_step_count: int | None
    in_silico_step_count: int | None
    requires_external_authorisation: bool | None
    referral: dict[str, Any] | None
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioethicsActionReviewReport":
        raw = _payload(value, label="bioethics action review", direct_keys=("subject", "refusal"))
        ok = _bool("bioethics action review ok", raw.get("ok"))
        fail_closed = _bool("bioethics action review fail_closed", raw.get("fail_closed", False))
        if not ok:
            stage, refusal, guarantee = _refusal(raw, "bioethics action review")
            return cls(raw, False, None, None, (), None, None, None, None, None, stage, refusal, True, guarantee, ())
        if fail_closed or raw.get("refusal") is not None or raw.get("stage") is not None:
            raise ArgumentError("successful bioethics action reviews cannot carry refusal evidence")
        referral = _route_mapping("bioethics action referral", raw.get("referral"))
        if referral.get("executes_physical_action") is not False:
            raise ArgumentError("bioethics action review must never execute a physical action")
        return cls(
            raw,
            True,
            _route_text("bioethics action subject", raw.get("subject")),
            _route_text("bioethics action declared_use", raw.get("declared_use")),
            _route_strings("bioethics permitted uses", raw.get("permitted_uses")),
            _route_mapping("bioethics action disposition", raw.get("disposition")),
            _route_count("bioethics physical_step_count", raw.get("physical_step_count")),
            _route_count("bioethics in_silico_step_count", raw.get("in_silico_step_count")),
            _bool("bioethics requires_external_authorisation", raw.get("requires_external_authorisation")),
            referral,
            None,
            None,
            False,
            None,
            _route_strings("bioethics action guarantees", raw.get("guarantees")),
        )

    @property
    def physical_execution_reachable(self) -> bool:
        return False

    @property
    def referral_status(self) -> str | None:
        return None if self.referral is None else _optional_text("bioethics referral status", self.referral.get("status"))

    @property
    def referral_ready(self) -> bool:
        return self.referral_status == "referred"


@dataclass(frozen=True)
class HumanSubjectScreenReport:
    raw: dict[str, Any]
    ok: bool
    subject: str
    determination: dict[str, Any]
    requires_institutional_review: bool
    triggers: tuple[Any, ...]
    consent: dict[str, Any]
    return_of_results: dict[str, Any]
    clearance_issued: bool
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HumanSubjectScreenReport":
        raw = _payload(value, label="human-subject screen", direct_keys=("determination", "consent"))
        if not _bool("human-subject screen ok", raw.get("ok")):
            raise ArgumentError("human-subject screening transport projection is not successful")
        determination = _route_mapping("human-subject determination", raw.get("determination"))
        consent = _route_mapping("human-subject consent", raw.get("consent"))
        return_of_results = _route_mapping("human-subject return_of_results", raw.get("return_of_results"))
        clearance = _bool("human-subject clearance_issued", raw.get("clearance_issued"))
        if clearance:
            raise ArgumentError("human-subject screening cannot issue institutional clearance")
        determination_kind = _optional_text("human-subject determination kind", determination.get("determination"))
        if determination_kind not in {"review_required", "undetermined"}:
            raise ArgumentError(f"unknown human-subject determination: {determination_kind!r}")
        return cls(
            raw,
            True,
            _route_text("human-subject subject", raw.get("subject")),
            determination,
            _bool("human-subject requires_institutional_review", raw.get("requires_institutional_review")),
            _array("human-subject triggers", raw.get("triggers")),
            consent,
            return_of_results,
            False,
            _route_strings("human-subject guarantees", raw.get("guarantees")),
        )

    @property
    def review_required(self) -> bool:
        return self.determination.get("determination") == "review_required"

    @property
    def undetermined(self) -> bool:
        return self.determination.get("determination") == "undetermined"

    @property
    def consent_status(self) -> str | None:
        return _optional_text("human-subject consent status", self.consent.get("status"))

    @property
    def return_of_results_status(self) -> str | None:
        return _optional_text("human-subject return-of-results status", self.return_of_results.get("status"))


@dataclass(frozen=True)
class BioethicsDualUseReviewReport:
    raw: dict[str, Any]
    ok: bool
    subject: str | None
    surfaces: tuple[str, ...]
    assessor: str | None
    sensitive_category: str | None
    decision: dict[str, Any] | None
    referral: dict[str, Any] | None
    withholding: dict[str, Any] | None
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioethicsDualUseReviewReport":
        raw = _payload(value, label="bioethics dual-use review", direct_keys=("subject", "refusal"))
        ok = _bool("bioethics dual-use review ok", raw.get("ok"))
        fail_closed = _bool("bioethics dual-use review fail_closed", raw.get("fail_closed", False))
        if not ok:
            stage, refusal, guarantee = _refusal(raw, "bioethics dual-use review")
            return cls(raw, False, None, (), None, None, None, None, None, stage, refusal, True, guarantee, ())
        if fail_closed or raw.get("refusal") is not None or raw.get("stage") is not None:
            raise ArgumentError("successful bioethics dual-use reviews cannot carry refusal evidence")
        surfaces = _route_strings("dual-use surfaces", raw.get("surfaces"))
        if any(surface not in MISUSE_SURFACES for surface in surfaces):
            raise ArgumentError("dual-use surfaces contains an unknown misuse surface")
        referral = _route_mapping("dual-use referral", raw.get("referral"))
        return cls(
            raw,
            True,
            _route_text("dual-use subject", raw.get("subject")),
            surfaces,
            _route_text("dual-use assessor", raw.get("assessor")),
            _route_text("dual-use sensitive_category", raw.get("sensitive_category")),
            _route_mapping("dual-use decision", raw.get("decision")),
            referral,
            _route_mapping("dual-use withholding", raw.get("withholding")),
            None,
            None,
            False,
            None,
            _route_strings("dual-use guarantees", raw.get("guarantees")),
        )

    @property
    def misuse_assessed(self) -> bool:
        return self.ok

    @property
    def risk_gate_reached(self) -> bool:
        return self.ok

    @property
    def withheld_exploit_detail_only(self) -> bool:
        if self.withholding is None:
            return True
        scope = self.withholding.get("scope")
        return scope in (None, "exploit_detail")


@dataclass(frozen=True)
class BioethicsValidationCheckReport:
    raw: dict[str, Any]
    ok: bool
    subject: str
    author: str
    maturity: str
    missing: tuple[str, ...]
    missing_count: int
    verification: dict[str, Any]
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioethicsValidationCheckReport":
        raw = _payload(value, label="bioethics validation check", direct_keys=("verification", "missing"))
        if not _bool("bioethics validation ok", raw.get("ok")):
            raise ArgumentError("bioethics validation transport projection is not successful")
        missing = _route_strings("validation missing", raw.get("missing"))
        if any(kind not in VALIDATION_EVIDENCE_KINDS for kind in missing):
            raise ArgumentError("validation missing contains an unknown evidence kind")
        missing_count = _route_count("validation missing_count", raw.get("missing_count"))
        if missing_count != len(missing):
            raise ArgumentError("validation missing_count does not reconcile with missing evidence")
        verification = _route_mapping("validation verification", raw.get("verification"))
        status = _route_text("validation verification status", verification.get("status"))
        if status not in {"verified", "refused"}:
            raise ArgumentError(f"unknown validation verification status: {status!r}")
        if status == "verified" and missing:
            raise ArgumentError("validation cannot be verified while evidence is missing")
        if status == "refused" and not _bool("validation refusal fail_closed", verification.get("fail_closed")):
            raise ArgumentError("validation refusal must be fail-closed")
        maturity = _route_text("validation maturity", raw.get("maturity"))
        if maturity not in {"experimental", "verified"}:
            raise ArgumentError(f"unknown validation maturity: {maturity!r}")
        return cls(
            raw,
            True,
            _route_text("validation subject", raw.get("subject")),
            _route_text("validation author", raw.get("author")),
            maturity,
            missing,
            missing_count,
            verification,
            _route_strings("validation guarantees", raw.get("guarantees")),
        )

    @property
    def verified(self) -> bool:
        return self.verification.get("status") == "verified"

    @property
    def verification_refused(self) -> bool:
        return not self.verified


@dataclass(frozen=True)
class BioethicsRepresentationAuditReport:
    raw: dict[str, Any]
    ok: bool
    summary: dict[str, Any] | None
    measured_count: int | None
    unmeasured_count: int | None
    suppressed_count: int | None
    complete: bool | None
    incomplete_axes: tuple[str, ...]
    attribution: dict[str, Any] | None
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioethicsRepresentationAuditReport":
        raw = _payload(value, label="bioethics representation audit", direct_keys=("summary", "refusal"))
        ok = _bool("representation audit ok", raw.get("ok"))
        fail_closed = _bool("representation audit fail_closed", raw.get("fail_closed", False))
        if not ok:
            stage, refusal, _ = _refusal(raw, "representation audit")
            return cls(raw, False, None, None, None, None, None, (), None, stage, refusal, True, ())
        if fail_closed or raw.get("refusal") is not None or raw.get("stage") is not None:
            raise ArgumentError("successful representation audits cannot carry refusal evidence")
        summary = _route_mapping("representation summary", raw.get("summary"))
        for field in ("measured", "unmeasured", "suppressed"):
            if not isinstance(summary.get(field), Sequence) or isinstance(summary.get(field), (str, bytes)):
                raise ArgumentError(f"representation summary must retain {field} strata")
        measured_count = _route_count("representation measured_count", raw.get("measured_count"))
        unmeasured_count = _route_count("representation unmeasured_count", raw.get("unmeasured_count"))
        suppressed_count = _route_count("representation suppressed_count", raw.get("suppressed_count"))
        if (measured_count, unmeasured_count, suppressed_count) != (len(summary["measured"]), len(summary["unmeasured"]), len(summary["suppressed"])):
            raise ArgumentError("representation counts do not reconcile with retained strata")
        return cls(
            raw,
            True,
            summary,
            measured_count,
            unmeasured_count,
            suppressed_count,
            _bool("representation complete", raw.get("complete")),
            _route_strings("representation incomplete_axes", raw.get("incomplete_axes")),
            _optional_mapping("representation attribution", raw.get("attribution")),
            None,
            None,
            False,
            _route_strings("representation guarantees", raw.get("guarantees")),
        )

    @property
    def coverage_preserved(self) -> bool:
        return self.ok and self.summary is not None and all(key in self.summary for key in ("measured", "unmeasured", "suppressed"))

    @property
    def incomplete(self) -> bool:
        return self.ok and not bool(self.complete)


def bioethics_action_review_report(value: Mapping[str, Any]) -> BioethicsActionReviewReport:
    return BioethicsActionReviewReport.from_wire(value)


def human_subject_screen_report(value: Mapping[str, Any]) -> HumanSubjectScreenReport:
    return HumanSubjectScreenReport.from_wire(value)


def bioethics_dual_use_review_report(value: Mapping[str, Any]) -> BioethicsDualUseReviewReport:
    return BioethicsDualUseReviewReport.from_wire(value)


def bioethics_validation_check_report(value: Mapping[str, Any]) -> BioethicsValidationCheckReport:
    return BioethicsValidationCheckReport.from_wire(value)


def bioethics_representation_audit_report(value: Mapping[str, Any]) -> BioethicsRepresentationAuditReport:
    return BioethicsRepresentationAuditReport.from_wire(value)


__all__ = [
    "ENGAGEMENT_KINDS",
    "MISUSE_SURFACES",
    "RETURN_OF_RESULTS",
    "VALIDATION_EVIDENCE_KINDS",
    "WITHHOLD_SCOPES",
    "BioethicsActionReviewArgs",
    "BioethicsActionReviewReport",
    "BioethicsDualUseReviewArgs",
    "BioethicsDualUseReviewReport",
    "BioethicsRepresentationAuditArgs",
    "BioethicsRepresentationAuditReport",
    "BioethicsValidationCheckArgs",
    "BioethicsValidationCheckReport",
    "HumanSubjectScreenArgs",
    "HumanSubjectScreenReport",
    "bioethics_action_review_report",
    "bioethics_dual_use_review_report",
    "bioethics_representation_audit_report",
    "bioethics_validation_check_report",
    "human_subject_screen_report",
]
