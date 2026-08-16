from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    OncoWorldsClonalHistoryCheckArgs,
    OncoWorldsMethylationClassifyArgs,
    OncoWorldsMethylationCompareReport,
    OncoWorldsModelTransportReport,
    OncoWorldsRadiogenomicCheckReport,
    oncoworlds_clonal_history_check_report,
    oncoworlds_methylation_classify_report,
    oncoworlds_methylation_compare_report,
    oncoworlds_model_transport_report,
    oncoworlds_radiogenomic_check_report,
)


class OncoWorldsReportTests(unittest.TestCase):
    def test_model_transport_preserves_supported_claim_and_typed_refusal(self) -> None:
        accepted = oncoworlds_model_transport_report({
            "ok": True,
            "model_statement": "the organoid response",
            "effective_biological_n": 3,
            "patient_relevant_claim": {"claim": "bounded research transport"},
            "guarantees": ["loss ledger"],
            "limitations": ["caller supplied"],
        })
        self.assertIsInstance(accepted, OncoWorldsModelTransportReport)
        self.assertTrue(accepted.supported)
        refused = oncoworlds_model_transport_report({
            "ok": False,
            "stage": "model_to_patient_transport",
            "refusal": {"refusal": "unverified_model_identity", "model": "m", "specimen": "s"},
            "refusal_text": "model identity was not verified",
            "fail_closed": True,
            "model_statement": "effect",
        })
        self.assertFalse(refused.ok)
        self.assertEqual(refused.refusal["refusal"], "unverified_model_identity")
        self.assertTrue(refused.fail_closed)

    def test_methylation_classification_and_version_conditioning_are_not_collapsed(self) -> None:
        classified = oncoworlds_methylation_classify_report({
            "ok": True,
            "classified": True,
            "class": "class-a",
            "report": {"outcome": "classified", "caveats": ["tumour content unobserved"]},
            "guarantees": ["threshold"],
            "limitations": ["no fitting"],
        })
        self.assertFalse(classified.unclassifiable)
        abstained = oncoworlds_methylation_classify_report({
            "ok": True,
            "classified": False,
            "class": None,
            "report": {"outcome": "unclassifiable", "nearest": {"label_only": "class-b"}},
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(abstained.unclassifiable)
        comparison = oncoworlds_methylation_compare_report({
            "ok": True,
            "comparison": {"divergence": {"divergence": "version_conditioned", "under_left": "class-a", "under_right": "class-b"}},
            "left_classifier": {"version": "v1"},
            "right_classifier": {"version": "v2"},
            "guarantees": [],
            "limitations": [],
        })
        self.assertIsInstance(comparison, OncoWorldsMethylationCompareReport)
        self.assertTrue(comparison.version_conditioned)

    def test_radiogenomic_refusal_and_clonal_ambiguity_remain_visible(self) -> None:
        refused = oncoworlds_radiogenomic_check_report({
            "ok": False,
            "stage": "radiogenomic_claim",
            "refusal": {"refusal": "leaky_split", "unit": "image"},
            "refusal_text": "leaky split",
            "fail_closed": True,
        })
        self.assertIsInstance(refused, OncoWorldsRadiogenomicCheckReport)
        self.assertFalse(refused.supported)
        clonal = oncoworlds_clonal_history_check_report({
            "ok": True,
            "schema": "bioprism-mcp/oncoworlds-clonal-history-check/0.1",
            "compatible_count": 2,
            "rejected_count": 1,
            "candidate_count": 3,
            "compatible": [{"edges": []}, {"edges": [{"parent": "a", "child": "b"}]}],
            "rejected": [[{"edges": []}, {"refusal": "cyclic"}]],
            "rejected_records": [{
                "history": {"edges": []},
                "refusal": {"refusal": "cyclic"},
                "refusal_kind": "cyclic",
                "refusal_text": "ancestry edges contain a cycle",
            }],
            "unique_history": {"ok": False, "refusal": {"refusal": "ambiguous", "count": 2}, "refusal_text": "two histories remain"},
            "unique_status": "ambiguous",
            "guarantees": ["rejected retained"],
            "limitations": [],
        })
        self.assertFalse(clonal.unique)
        self.assertTrue(clonal.ambiguous_or_refused)
        self.assertEqual(clonal.rejected_count, 1)
        self.assertEqual(clonal.unique_status, "ambiguous")
        self.assertEqual(clonal.rejected_records[0].refusal_kind, "cyclic")
        self.assertEqual(clonal.candidate_count, 3)

    def test_oncoworlds_requests_enforce_bounded_transport_shape(self) -> None:
        args = OncoWorldsClonalHistoryCheckArgs({"subclones": []}, [{"edges": []}])
        self.assertEqual(args.to_mcp_arguments()["candidates"], [{"edges": []}])
        with self.assertRaises(ArgumentError):
            OncoWorldsMethylationClassifyArgs({}, {str(index): {} for index in range(10_001)}, {})


if __name__ == "__main__":
    unittest.main()
