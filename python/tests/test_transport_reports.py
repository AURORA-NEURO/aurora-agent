from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    ModalityTransportCheckArgs,
    ModalityTransportCheckReport,
    modality_transport_check_report,
)


class ModalityTransportReportTests(unittest.TestCase):
    def test_loss_fidelity_inverse_and_claim_rows_remain_visible(self) -> None:
        report = modality_transport_check_report({
            "ok": True,
            "schema": "bioprism-mcp/modality-transport-check/0.1",
            "outcome_kind": "constructed",
            "constructed": True,
            "from": "single_cell",
            "to": "bulk_transcriptomics",
            "axis": "cell",
            "transport": {"from": "single_cell", "to": "bulk_transcriptomics", "kind": {"kind": "aggregation"}},
            "fidelity": {"fidelity": "exact"},
            "loss": {"discarded": ["cell distribution"]},
            "scope_mapping": {},
            "scope_mapping_check": "sound",
            "inverse": {"invertible": False, "refusal_kind": "not_invertible"},
            "application": {"applied": True, "refusal": None},
            "applied_descriptor": {"descriptor": {"resolutions": []}},
            "claims": [{"claim": "cell_intrinsic_change", "support_lost": True}],
        })
        self.assertIsInstance(report, ModalityTransportCheckReport)
        self.assertTrue(report.constructed)
        self.assertEqual(report.raw["fidelity"]["fidelity"], "exact")
        self.assertTrue(report.claims[0]["support_lost"])

        args = ModalityTransportCheckArgs(
            "single_cell",
            "bulk_transcriptomics",
            "cell",
            {"kind": "aggregation", "operator": "mean"},
            claims=("cell_composition",),
        )
        self.assertEqual(args.to_mcp_arguments()["from"], "single_cell")
        with self.assertRaises(ArgumentError):
            ModalityTransportCheckArgs("bulk_transcriptomics", "single_cell", "cell", {"kind": "deconvolution"})
        with self.assertRaises(ArgumentError):
            ModalityTransportCheckArgs("single_cell", "bulk_transcriptomics", "cell", {"kind": "unknown"})


if __name__ == "__main__":
    unittest.main()
