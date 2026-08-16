from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    ModalitySupportCheckArgs,
    ModalitySupportCheckReport,
    modality_support_check_report,
)


class ModalitySupportReportTests(unittest.TestCase):
    def test_support_and_analysis_unit_are_separate(self) -> None:
        refused = modality_support_check_report({
            "ok": True,
            "schema": "bioprism-mcp/modality-support-check/0.1",
            "outcome_kind": "refused",
            "modality": "bulk_transcriptomics",
            "claim": "cell_intrinsic_change",
            "supported": False,
            "claim_requirements": {"axes": ["cell"]},
            "support": {
                "supported": False,
                "refusal": {"unsupported": "missing_resolution"},
                "refusal_kind": "named_failure_mode",
                "root_refusal_kind": "missing_resolution",
            },
            "analysis_unit": {
                "requested": True,
                "counted": "population",
                "independent": "subject",
                "admissible": False,
                "refusal": {"unsupported": "named_failure_mode"},
                "refusal_kind": "named_failure_mode",
            },
            "descriptor": {"complete": True},
        })
        self.assertIsInstance(refused, ModalitySupportCheckReport)
        self.assertFalse(refused.supported)
        self.assertEqual(refused.support["root_refusal_kind"], "missing_resolution")
        self.assertFalse(refused.analysis_unit["admissible"])

        args = ModalitySupportCheckArgs("single_cell", "cell_composition", counted_unit="subject")
        self.assertEqual(args.to_mcp_arguments()["modality"], "single_cell")
        with self.assertRaises(ArgumentError):
            ModalitySupportCheckArgs("single_cell", "not_a_claim")
        with self.assertRaises(ArgumentError):
            ModalitySupportCheckArgs("single_cell", "cell_composition", counted_unit="unknown")


if __name__ == "__main__":
    unittest.main()
