"""Typed safety-release and research-only medical-boundary contracts.

The safety kernel intentionally does not classify content or provide runtime security.  It evaluates
reviewer-supplied labels and preserves the kernel's fail-closed distinctions: unrated is not low,
one high non-mitigating dimension is conditioned, two block, and clinical output is never admitted.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


SAFETY_RISK_DIMENSIONS = (
    "capability_uplift",
    "actionability",
    "scale",
    "expertise_reduction",
    "target_specificity",
    "reversibility",
    "detectability",
    "available_safeguards",
    "legitimate_scientific_value",
)
SAFETY_MITIGATING_DIMENSIONS = frozenset({"detectability", "available_safeguards", "legitimate_scientific_value"})
SAFETY_RATINGS = frozenset({"low", "moderate", "high"})
SAFETY_CATEGORIES = frozenset({
    "cyber_exploitation",
    "biological_design",
    "surveillance_and_privacy_invasion",
    "fraud",
    "harmful_physical_automation",
    "clinical_misuse",
})
SAFETY_RESEARCH_USES = frozenset({
    "workflow_reproducibility",
    "data_quality_checks",
    "paper_data_code_linkage",
    "imaging_and_omics_metadata_reasoning",
    "tool_use",
    "provenance",
    "evidence_synthesis",
    "uncertainty_reporting",
    "benchmark_methodology",
})
SAFETY_PROHIBITED_OUTPUTS = frozenset({
    "personalised_clinical_recommendation",
    "urgency_classification",
    "treatment_selection",
    "prognosis_as_patient_advice",
    "clinician_review_bypass",
})
SAFETY_GATE_DECISIONS = frozenset({"cleared", "conditioned", "blocked"})
SAFETY_CONDITION_CONTROLS = ("gated reviewer access", "non-executable release form")
SAFETY_GATE_RULE = "zero high non-mitigating dimensions clears; one conditions release; two or more block; any unrated dimension refuses the gate"


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    if value is None:
        return None
    return _route_text(name, value)


def _payload(value: Mapping[str, Any], required: tuple[str, ...], label: str) -> dict[str, Any]:
    raw = _route_mapping(f"{label} response", value)
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
                    raise ArgumentError(f"{label} response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping(f"decoded {label} response", decoded)
                if all(key in decoded_mapping for key in required):
                    return decoded_mapping
    raise ArgumentError(f"response does not contain a {label} projection")


@dataclass(frozen=True)
class RiskAssessmentRequest:
    """Reviewer-labelled dual-use assessment; omitted ratings remain unrated."""

    subject: str
    ratings: Mapping[str, str] = field(default_factory=dict)
    category: str | None = None

    def __post_init__(self) -> None:
        _route_text("risk assessment subject", self.subject)
        if self.category is not None and (not isinstance(self.category, str) or self.category not in SAFETY_CATEGORIES):
            raise ArgumentError(f"unknown safety assessment category: {self.category!r}")
        if not isinstance(self.ratings, Mapping):
            raise ArgumentError("risk assessment ratings must be an object")
        normalized: dict[str, str] = {}
        for dimension, rating in self.ratings.items():
            if dimension not in SAFETY_RISK_DIMENSIONS:
                raise ArgumentError(f"unknown safety risk dimension: {dimension!r}")
            if not isinstance(rating, str) or rating not in SAFETY_RATINGS:
                raise ArgumentError(f"unknown safety rating for {dimension}: {rating!r}")
            normalized[dimension] = rating
        object.__setattr__(self, "ratings", normalized)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RiskAssessmentRequest":
        raw = _route_mapping("risk assessment", value)
        return cls(
            subject=_route_text("risk assessment subject", raw.get("subject")),
            ratings=raw.get("ratings", {}),
            category=_optional_text("risk assessment category", raw.get("category")),
        )

    @property
    def unrated_dimensions(self) -> tuple[str, ...]:
        return tuple(dimension for dimension in SAFETY_RISK_DIMENSIONS if dimension not in self.ratings)

    def to_mcp_arguments(self) -> dict[str, Any]:
        assessment: dict[str, Any] = {"subject": self.subject, "ratings": dict(self.ratings)}
        if self.category is not None:
            assessment["category"] = self.category
        return {"assessment": assessment}


@dataclass(frozen=True)
class SafetyReleaseGateArgs:
    assessment: RiskAssessmentRequest | Mapping[str, Any]

    def __post_init__(self) -> None:
        assessment = self.assessment
        if isinstance(assessment, RiskAssessmentRequest):
            normalized = assessment
        elif isinstance(assessment, Mapping):
            normalized = RiskAssessmentRequest.from_wire(assessment)
        else:
            raise ArgumentError("assessment must be a RiskAssessmentRequest or object")
        object.__setattr__(self, "assessment", normalized)

    def to_mcp_arguments(self) -> dict[str, Any]:
        return self.assessment.to_mcp_arguments()


@dataclass(frozen=True)
class SafetyGateDecisionReport:
    raw: dict[str, Any]
    decision: str
    subject: str
    conditions: tuple[str, ...]
    driven_by: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SafetyGateDecisionReport":
        raw = _route_mapping("safety gate decision", value)
        decision = _route_text("safety gate decision kind", raw.get("decision"))
        if decision not in SAFETY_GATE_DECISIONS:
            raise ArgumentError(f"unknown safety gate decision: {decision!r}")
        conditions = _route_strings("safety gate conditions", raw.get("conditions", []))
        driven_by = _route_strings("safety gate driven_by", raw.get("driven_by", []))
        if any(dimension not in SAFETY_RISK_DIMENSIONS for dimension in driven_by):
            raise ArgumentError("safety gate driven_by contains an unknown risk dimension")
        if any(dimension in SAFETY_MITIGATING_DIMENSIONS for dimension in driven_by):
            raise ArgumentError("mitigating dimensions cannot drive a safety block or condition")
        if decision == "conditioned" and tuple(conditions) != SAFETY_CONDITION_CONTROLS:
            raise ArgumentError("conditioned safety gate must carry the kernel's exact controls")
        if decision != "conditioned" and conditions:
            raise ArgumentError("cleared or blocked safety gate cannot carry conditioned-only controls")
        if decision == "cleared" and driven_by:
            raise ArgumentError("cleared safety gate cannot be driven by high-risk dimensions")
        if decision == "conditioned" and len(driven_by) != 1:
            raise ArgumentError("conditioned safety gate must have exactly one driver")
        if decision == "blocked" and len(driven_by) < 2:
            raise ArgumentError("blocked safety gate must have at least two drivers")
        return cls(raw, decision, _route_text("safety gate decision subject", raw.get("subject")), conditions, driven_by)

    @property
    def cleared(self) -> bool:
        return self.decision == "cleared"


@dataclass(frozen=True)
class SafetyReleaseGateReport:
    raw: dict[str, Any]
    ok: bool
    subject: str
    category: str | None
    decision: SafetyGateDecisionReport
    cleared: bool
    unrated_dimensions: tuple[str, ...]
    high_risk_dimensions: tuple[str, ...]
    rule: str
    fail_closed: bool
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SafetyReleaseGateReport":
        raw = _payload(
            value,
            ("ok", "subject", "decision", "cleared", "unrated_dimensions", "high_risk_dimensions", "rule", "fail_closed", "limitations"),
            "safety release gate",
        )
        if not _bool("safety release gate ok", raw.get("ok")):
            raise ArgumentError("safety release gate report is not successful")
        subject = _route_text("safety release gate subject", raw.get("subject"))
        category = _optional_text("safety release gate category", raw.get("category"))
        if category is not None and category not in SAFETY_CATEGORIES:
            raise ArgumentError(f"unknown safety release gate category: {category!r}")
        decision = SafetyGateDecisionReport.from_wire(raw.get("decision"))
        if decision.subject != subject:
            raise ArgumentError("safety release gate decision subject does not reconcile")
        cleared = _bool("safety release gate cleared", raw.get("cleared"))
        if cleared != decision.cleared:
            raise ArgumentError("safety release gate cleared does not reconcile with decision")
        unrated = _route_strings("safety release gate unrated_dimensions", raw.get("unrated_dimensions"))
        high_risk = _route_strings("safety release gate high_risk_dimensions", raw.get("high_risk_dimensions"))
        if any(dimension not in SAFETY_RISK_DIMENSIONS for dimension in unrated + high_risk):
            raise ArgumentError("safety release gate contains an unknown risk dimension")
        if any(dimension in SAFETY_MITIGATING_DIMENSIONS for dimension in high_risk):
            raise ArgumentError("safety high_risk_dimensions cannot contain mitigating dimensions")
        if unrated:
            raise ArgumentError("successful safety release gate cannot contain unrated dimensions")
        if tuple(decision.driven_by) != tuple(high_risk):
            raise ArgumentError("safety release gate driven_by does not reconcile with high_risk_dimensions")
        rule = _route_text("safety release gate rule", raw.get("rule"))
        if rule != SAFETY_GATE_RULE:
            raise ArgumentError("safety release gate rule does not match the kernel rule")
        fail_closed = _bool("safety release gate fail_closed", raw.get("fail_closed"))
        if not fail_closed:
            raise ArgumentError("safety release gate must be fail-closed")
        return cls(raw, True, subject, category, decision, cleared, unrated, high_risk, rule, fail_closed, _route_strings("safety release gate limitations", raw.get("limitations")))

    @property
    def release_ready(self) -> bool:
        return self.cleared

    @property
    def conditioned(self) -> bool:
        return self.decision.decision == "conditioned"

    @property
    def blocked(self) -> bool:
        return self.decision.decision == "blocked"


@dataclass(frozen=True)
class MedicalBoundaryRequest:
    output: Mapping[str, Any]

    def __post_init__(self) -> None:
        if not isinstance(self.output, Mapping):
            raise ArgumentError("medical boundary output must be an object")
        side = _route_text("medical boundary output side", self.output.get("side"))
        _route_text("medical boundary output label", self.output.get("label"))
        if side == "research":
            use_case = _route_text("medical boundary research use_case", self.output.get("use_case"))
            if use_case not in SAFETY_RESEARCH_USES:
                raise ArgumentError(f"unknown research use case: {use_case!r}")
        elif side == "clinical":
            category = _route_text("medical boundary clinical category", self.output.get("category"))
            if category not in SAFETY_PROHIBITED_OUTPUTS:
                raise ArgumentError(f"unknown prohibited clinical output: {category!r}")
        else:
            raise ArgumentError(f"unknown medical boundary side: {side!r}")

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"output": dict(self.output)}


@dataclass(frozen=True)
class MedicalBoundaryReport:
    raw: dict[str, Any]
    ok: bool
    admitted: bool
    use_case: str | None
    refusal: str | None
    research_only_label: str
    boundary_is_unconditional: bool
    clinical_output_is_never_admitted: bool | None
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MedicalBoundaryReport":
        raw = _payload(value, ("admitted", "research_only_label", "boundary_is_unconditional"), "medical boundary")
        ok = _bool("medical boundary ok", raw.get("ok"))
        admitted = _bool("medical boundary admitted", raw.get("admitted"))
        if admitted != ok:
            raise ArgumentError("medical boundary admitted does not reconcile with ok")
        use_case = _optional_text("medical boundary use_case", raw.get("use_case"))
        if use_case is not None and use_case not in SAFETY_RESEARCH_USES:
            raise ArgumentError(f"unknown medical boundary use case: {use_case!r}")
        refusal = _optional_text("medical boundary refusal", raw.get("refusal"))
        never_admitted = raw.get("clinical_output_is_never_admitted")
        if never_admitted is not None:
            never_admitted = _bool("medical boundary clinical_output_is_never_admitted", never_admitted)
        if admitted:
            if use_case is None or refusal is not None or never_admitted is not None:
                raise ArgumentError("admitted medical research output must name a use case without refusal")
        else:
            if refusal is None or use_case is not None or never_admitted is not True:
                raise ArgumentError("refused medical output must preserve unconditional clinical refusal")
        boundary_is_unconditional = _bool("medical boundary boundary_is_unconditional", raw.get("boundary_is_unconditional"))
        if not boundary_is_unconditional:
            raise ArgumentError("medical boundary must remain unconditional")
        return cls(raw, ok, admitted, use_case, refusal, _route_text("medical boundary research_only_label", raw.get("research_only_label")), boundary_is_unconditional, never_admitted, _route_strings("medical boundary limitations", raw.get("limitations", [])))

    @property
    def research_only(self) -> bool:
        return self.admitted

    @property
    def clinical_refused(self) -> bool:
        return not self.admitted


def safety_release_gate_report(value: Mapping[str, Any]) -> SafetyReleaseGateReport:
    """Parse direct MCP or HTTP safety-release output."""

    return SafetyReleaseGateReport.from_wire(value)


def medical_boundary_report(value: Mapping[str, Any]) -> MedicalBoundaryReport:
    """Parse direct MCP or HTTP medical-boundary output, including structured refusals."""

    return MedicalBoundaryReport.from_wire(value)


__all__ = [
    "SAFETY_CATEGORIES",
    "SAFETY_CONDITION_CONTROLS",
    "SAFETY_GATE_RULE",
    "SAFETY_GATE_DECISIONS",
    "SAFETY_MITIGATING_DIMENSIONS",
    "SAFETY_PROHIBITED_OUTPUTS",
    "SAFETY_RATINGS",
    "SAFETY_RESEARCH_USES",
    "SAFETY_RISK_DIMENSIONS",
    "MedicalBoundaryReport",
    "MedicalBoundaryRequest",
    "RiskAssessmentRequest",
    "SafetyGateDecisionReport",
    "SafetyReleaseGateArgs",
    "SafetyReleaseGateReport",
    "medical_boundary_report",
    "safety_release_gate_report",
]
