from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    MeasurementCompareArgs,
    MeasurementCompareReport,
    measurement_compare_report,
)


def conversion_payload(*, blocked: bool = False) -> dict:
    verdict = {"verdict": "blocked", "reason": {"blocked_by": "dimension_mismatch", "left": "mm", "right": "mL", "left_dimension": {"length": 1}, "right_dimension": {}}} if blocked else {"verdict": "comparable"}
    report = {
        "left": "left",
        "right": "right",
        "verdict": verdict,
        "conversions": [] if blocked else [{"from": "cm", "to": "mm", "factor": 10.0, "exactness": {"exactness": "exact"}}],
        "caveats": ["neither measurement is bound to an ontology term; comparison rests on local labels"],
    }
    return {
        "ok": True,
        "comparable": not blocked,
        "policy": {"require_bound_terms": False},
        "report": report,
        "report_sha256": "a" * 64,
        "guarantees": ["unit conversion is explicit"],
        "limitations": ["caller-supplied declarations"],
    }


class StandardsTests(unittest.TestCase):
    def test_request_preserves_measurement_declarations_and_policy(self) -> None:
        request = MeasurementCompareArgs({"label": "left"}, {"label": "right"}, require_bound_terms=True)
        self.assertEqual(request.to_mcp_arguments()["require_bound_terms"], True)
        self.assertEqual(MeasurementCompareArgs.from_wire({"left": {}, "right": {}}).require_bound_terms, False)
        with self.assertRaises(ArgumentError):
            MeasurementCompareArgs([], {})  # type: ignore[arg-type]

    def test_report_preserves_conversion_receipts_and_blocking_class(self) -> None:
        comparable = measurement_compare_report(conversion_payload())
        blocked = measurement_compare_report({"ok": True, "mcp": {"result": {"structuredContent": conversion_payload(blocked=True)}}})
        self.assertIsInstance(comparable, MeasurementCompareReport)
        self.assertTrue(comparable.comparable)
        self.assertEqual(comparable.conversions[0].from_unit, "cm")
        self.assertTrue(blocked.blocked)
        self.assertEqual(blocked.verdict.reason.blocked_by, "dimension_mismatch")
        self.assertFalse(blocked.verdict.reason.metadata_silence)

    def test_report_rejects_silent_or_forged_comparability(self) -> None:
        forged = conversion_payload(blocked=True)
        forged["comparable"] = True
        with self.assertRaises(ArgumentError):
            measurement_compare_report(forged)
        forged = conversion_payload()
        forged["report_sha256"] = "not-a-digest"
        with self.assertRaises(ArgumentError):
            measurement_compare_report(forged)
        forged = conversion_payload()
        forged["report"]["conversions"][0]["exactness"] = {"exactness": "conventional"}
        with self.assertRaises(ArgumentError):
            measurement_compare_report(forged)


if __name__ == "__main__":
    unittest.main()
