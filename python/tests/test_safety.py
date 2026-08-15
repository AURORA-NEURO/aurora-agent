from __future__ import annotations

import json
import unittest

from prism_sdk import (
    ArgumentError,
    MedicalBoundaryReport,
    MedicalBoundaryRequest,
    RiskAssessmentRequest,
    SafetyPostureArgs,
    SafetyPostureReport,
    SafetyReleaseGateArgs,
    SafetyReleaseGateReport,
    medical_boundary_report,
    safety_posture_report,
    safety_release_gate_report,
)


DIMENSIONS = (
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


def assessment(*high: str) -> dict:
    return {
        "subject": "pack/biological-design@1",
        "category": "biological_design",
        "ratings": {dimension: "high" if dimension in high else "low" for dimension in DIMENSIONS},
    }


def gate_payload(*, decision: str, high_risk: list[str], cleared: bool) -> dict:
    decision_payload = {
        "decision": decision,
        "subject": "pack/biological-design@1",
    }
    if decision == "conditioned":
        decision_payload["conditions"] = ["gated reviewer access", "non-executable release form"]
    if decision != "cleared":
        decision_payload["driven_by"] = high_risk
    return {
        "ok": True,
        "subject": "pack/biological-design@1",
        "category": "biological_design",
        "decision": decision_payload,
        "cleared": cleared,
        "unrated_dimensions": [],
        "high_risk_dimensions": high_risk,
        "rule": "zero high non-mitigating dimensions clears; one conditions release; two or more block; any unrated dimension refuses the gate",
        "fail_closed": True,
        "limitations": ["ratings are reviewer-supplied"],
    }


def medical_research_payload() -> dict:
    return {
        "ok": True,
        "admitted": True,
        "use_case": "evidence_synthesis",
        "research_only_label": "research use only; not evaluated for clinical use, and not a medical device",
        "boundary_is_unconditional": True,
        "limitations": ["research outputs require domain review"],
    }


def medical_refusal_payload() -> dict:
    return {
        "ok": False,
        "admitted": False,
        "refusal": '"choose a treatment" is a treatment_selection output; this platform is research-only and does not produce it',
        "research_only_label": "research use only; not evaluated for clinical use, and not a medical device",
        "boundary_is_unconditional": True,
        "clinical_output_is_never_admitted": True,
    }


def posture_payload(*, include_details: bool = False) -> dict:
    payload = {
        "ok": True,
        "model": "section_13",
        "adversaries": 2,
        "threats": 3,
        "coverage": {"mitigated": 1, "declared_only": 1, "unmitigated": 1},
        "coverage_summary": "1 enforced, 1 declared-only, 1 unmitigated (of 3)",
        "residual_threat_ids": ["T-1", "T-2"],
        "unanalysed_threat_ids": ["T-2"],
        "unreachable_threat_ids": [],
        "audit_acceptances": True,
        "perimeter_controls_are_not_claimed_as_enforced": True,
    }
    if include_details:
        payload["threat_details"] = [
            {
                "id": "T-1",
                "module": "13.01",
                "asset": "result_integrity",
                "class": "evaluator_tampering",
                "requires": ["observes_public_surface"],
                "surface": "catalog",
                "narrative": "a threat",
                "mitigations": [{"state": "enforced", "name": "sealed record", "role": "preventative", "by": {"unrepresentable": "typed"}}],
            },
            {
                "id": "T-2",
                "module": "13.02",
                "asset": "hidden_oracle",
                "class": "holdout_read",
                "requires": ["executes_in_agent_sandbox"],
                "surface": "agent_sandbox",
                "narrative": "a declared threat",
                "mitigations": [{"state": "declared_only", "name": "sandbox separation", "role": "preventative", "declared_in": "13.02"}],
            },
            {
                "id": "T-3",
                "module": "13.03",
                "asset": "provider_credential",
                "class": "secret_exfiltration",
                "requires": ["holds_credential"],
                "surface": "control_plane",
                "narrative": "an unmitigated threat",
                "mitigations": [{"state": "absent", "name": "credential broker", "role": "preventative", "reason": {"reason": "requires_absent_infrastructure"}}],
            },
        ]
    return payload


class SafetyTests(unittest.TestCase):
    def test_request_preserves_unrated_dimensions_and_nested_wire_shape(self) -> None:
        request = RiskAssessmentRequest("subject", {"capability_uplift": "high"}, "fraud")
        gate_request = SafetyReleaseGateArgs(request)
        self.assertEqual(request.unrated_dimensions[0], "actionability")
        self.assertEqual(request.to_mcp_arguments()["assessment"]["category"], "fraud")
        self.assertEqual(gate_request.to_mcp_arguments()["assessment"]["subject"], "subject")
        self.assertEqual(
            SafetyReleaseGateReport.from_wire(gate_payload(decision="conditioned", high_risk=["capability_uplift"], cleared=False)).decision.decision,
            "conditioned",
        )
        with self.assertRaises(ArgumentError):
            RiskAssessmentRequest("subject", {"capability_uplift": "unknown"})
        with self.assertRaises(ArgumentError):
            MedicalBoundaryRequest({"side": "research", "use_case": "unknown", "label": "x"})

    def test_gate_report_reconciles_clear_conditioned_and_blocked_kernel_decisions(self) -> None:
        cleared = safety_release_gate_report(gate_payload(decision="cleared", high_risk=[], cleared=True))
        conditioned = safety_release_gate_report(gate_payload(decision="conditioned", high_risk=["actionability"], cleared=False))
        blocked = safety_release_gate_report(gate_payload(decision="blocked", high_risk=["actionability", "scale"], cleared=False))
        self.assertTrue(cleared.release_ready)
        self.assertTrue(conditioned.conditioned)
        self.assertTrue(blocked.blocked)
        self.assertEqual(conditioned.decision.conditions, ("gated reviewer access", "non-executable release form"))
        self.assertEqual(blocked.decision.driven_by, ("actionability", "scale"))

    def test_gate_report_rejects_forged_or_partial_clearance(self) -> None:
        forged = gate_payload(decision="cleared", high_risk=["actionability"], cleared=True)
        with self.assertRaises(ArgumentError):
            safety_release_gate_report(forged)
        forged = gate_payload(decision="conditioned", high_risk=["actionability"], cleared=False)
        forged["decision"]["conditions"] = ["review later"]
        with self.assertRaises(ArgumentError):
            safety_release_gate_report(forged)
        forged = gate_payload(decision="cleared", high_risk=[], cleared=True)
        forged["unrated_dimensions"] = ["scale"]
        with self.assertRaises(ArgumentError):
            safety_release_gate_report(forged)
        forged = gate_payload(decision="blocked", high_risk=["actionability", "scale"], cleared=False)
        forged["decision"]["driven_by"] = ["actionability"]
        with self.assertRaises(ArgumentError):
            safety_release_gate_report(forged)
        forged = gate_payload(decision="cleared", high_risk=[], cleared=True)
        forged["fail_closed"] = False
        with self.assertRaises(ArgumentError):
            safety_release_gate_report(forged)

    def test_gate_report_accepts_http_structured_content_and_text_content(self) -> None:
        payload = gate_payload(decision="cleared", high_risk=[], cleared=True)
        structured = {"ok": True, "mcp": {"result": {"structuredContent": payload}}}
        text_envelope = {"mcp": {"result": {"content": [{"type": "text", "text": json.dumps(payload)}]}}}
        direct_mcp = {"result": {"content": [{"type": "text", "text": json.dumps(payload)}]}}
        self.assertIsInstance(safety_release_gate_report(structured), SafetyReleaseGateReport)
        self.assertTrue(safety_release_gate_report(text_envelope).release_ready)
        self.assertTrue(safety_release_gate_report(direct_mcp).release_ready)

    def test_medical_boundary_preserves_research_admission_and_clinical_refusal(self) -> None:
        research = MedicalBoundaryRequest({"side": "research", "use_case": "evidence_synthesis", "label": "review"})
        clinical = MedicalBoundaryRequest({"side": "clinical", "category": "treatment_selection", "label": "choose a treatment"})
        admitted = medical_boundary_report({"ok": True, "mcp": {"result": {"structuredContent": medical_research_payload()}}})
        refused = medical_boundary_report({"ok": False, "mcp": {"result": {"structuredContent": medical_refusal_payload()}}})
        self.assertEqual(research.to_mcp_arguments()["output"]["use_case"], "evidence_synthesis")
        self.assertEqual(clinical.to_mcp_arguments()["output"]["category"], "treatment_selection")
        self.assertIsInstance(admitted, MedicalBoundaryReport)
        self.assertTrue(admitted.research_only)
        self.assertTrue(refused.clinical_refused)
        self.assertTrue(refused.clinical_output_is_never_admitted)

    def test_medical_boundary_rejects_false_admission_and_conditional_boundary(self) -> None:
        payload = medical_refusal_payload()
        payload["boundary_is_unconditional"] = False
        with self.assertRaises(ArgumentError):
            medical_boundary_report(payload)
        payload = medical_research_payload()
        payload["clinical_output_is_never_admitted"] = True
        with self.assertRaises(ArgumentError):
            medical_boundary_report(payload)
        payload = medical_refusal_payload()
        payload["use_case"] = "evidence_synthesis"
        with self.assertRaises(ArgumentError):
            medical_boundary_report(payload)

    def test_safety_posture_reconciles_populations_and_optional_threat_details(self) -> None:
        self.assertEqual(SafetyPostureArgs(True).to_mcp_arguments(), {"include_threats": True})
        summary = safety_posture_report(posture_payload())
        detailed = safety_posture_report({"ok": True, "mcp": {"result": {"structuredContent": posture_payload(include_details=True)}}})
        self.assertIsInstance(summary, SafetyPostureReport)
        self.assertFalse(summary.details_included)
        self.assertTrue(summary.has_unmitigated_threats)
        self.assertTrue(detailed.details_included)
        self.assertEqual(len(detailed.threat_details), 3)
        self.assertEqual(detailed.threat_details[1].mitigations[0].state, "declared_only")

    def test_safety_posture_rejects_population_or_mitigation_forgery(self) -> None:
        forged = posture_payload()
        forged["coverage"]["unmitigated"] = 2
        with self.assertRaises(ArgumentError):
            safety_posture_report(forged)
        forged = posture_payload(include_details=True)
        forged["unanalysed_threat_ids"] = ["T-unknown"]
        with self.assertRaises(ArgumentError):
            safety_posture_report(forged)
        forged = posture_payload(include_details=True)
        forged["threat_details"][2]["mitigations"][0].pop("reason")
        with self.assertRaises(ArgumentError):
            safety_posture_report(forged)


if __name__ == "__main__":
    unittest.main()
