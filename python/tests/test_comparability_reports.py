from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    ModalityComparabilityCheckArgs,
    ModalityComparabilityCheckReport,
    modality_comparability_check_report,
)


class ModalityComparabilityReportTests(unittest.TestCase):
    def test_blocked_modality_reason_and_digest_remain_visible(self) -> None:
        report = modality_comparability_check_report({
            "ok": True,
            "schema": "bioprism-mcp/modality-comparability-check/0.1",
            "outcome_kind": "blocked",
            "comparable": False,
            "policy": {"require_bound_terms": True},
            "check_order": ["measurand", "reported resolution axis"],
            "left": {"modality": "bulk_transcriptomics", "measurand": "RNA abundance"},
            "right": {"modality": "proteomics", "measurand": "protein abundance"},
            "report": {"verdict": {"comparable": False}},
            "verdict": {"reason": {"blocked_by": "measurand_mismatch"}},
            "report_sha256": "a" * 64,
        })
        self.assertIsInstance(report, ModalityComparabilityCheckReport)
        self.assertFalse(report.comparable)
        self.assertEqual(report.verdict["reason"]["blocked_by"], "measurand_mismatch")
        self.assertEqual(len(report.report_sha256), 64)

        args = ModalityComparabilityCheckArgs(
            {"descriptor": {}, "reported_at": "population", "measurement": {}},
            {"descriptor": {}, "reported_at": "population", "measurement": {}},
            {"require_bound_terms": True},
        )
        self.assertEqual(args.to_mcp_arguments()["policy"]["require_bound_terms"], True)
        with self.assertRaises(ArgumentError):
            ModalityComparabilityCheckArgs({}, None)
        with self.assertRaises(ArgumentError):
            modality_comparability_check_report({
                "ok": True,
                "schema": "bioprism-mcp/modality-comparability-check/0.1",
                "outcome_kind": "comparable",
                "comparable": True,
                "left": {},
                "right": {},
                "report": {},
                "verdict": {},
                "report_sha256": "not-a-digest",
            })


if __name__ == "__main__":
    unittest.main()
