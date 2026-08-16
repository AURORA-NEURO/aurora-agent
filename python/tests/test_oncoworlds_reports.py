from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    OncoWorldsClonalHistoryCheckArgs,
    OncoWorldsEraShiftCheckArgs,
    OncoWorldsEquityCheckArgs,
    OncoWorldsEntityWorldCheckArgs,
    OncoWorldsMethylationClassifyArgs,
    OncoWorldsMethylationCompareReport,
    OncoWorldsModelTransportReport,
    OncoWorldsRadiogenomicCheckReport,
    oncoworlds_clonal_history_check_report,
    oncoworlds_era_shift_check_report,
    oncoworlds_equity_check_report,
    oncoworlds_entity_world_check_report,
    oncoworlds_methylation_classify_report,
    oncoworlds_methylation_compare_report,
    oncoworlds_model_transport_report,
    oncoworlds_radiogenomic_check_report,
)


class OncoWorldsReportTests(unittest.TestCase):
    def test_model_transport_preserves_supported_claim_and_typed_refusal(self) -> None:
        accepted = oncoworlds_model_transport_report({
            "ok": True,
            "schema": "bioprism-mcp/oncoworlds-model-transport/0.1",
            "supported": True,
            "outcome_kind": "supported",
            "model_statement": "the organoid response",
            "effect": "the compound reduced viability",
            "model_identity": {"model": "ORG-1", "system": "organoid", "source_specimen": "S-1", "passage": 3, "verified_against_source": True},
            "rests_on": ["genomic"],
            "fidelity_axes": [{"axis": "genomic", "passage": 3, "measured": True}],
            "establishment": {"attempted": 3, "established": 3, "selected": False, "selection_modelled": False},
            "replicates": {"technical_wells": 6, "biological_replicates": 3, "effective_biological_n": 3, "claimed_n": 3},
            "transport_assumption_names": ["culture stated"],
            "required_assumptions": ["culture stated"],
            "effective_biological_n": 3,
            "patient_relevant_claim": {"result": {}, "cohort": {}, "transport": {}, "claimed_n": 3},
            "guarantees": ["loss ledger"],
            "limitations": ["caller supplied"],
        })
        self.assertIsInstance(accepted, OncoWorldsModelTransportReport)
        self.assertTrue(accepted.supported)
        self.assertEqual(accepted.outcome_kind, "supported")
        self.assertTrue(accepted.model_identity.verified_against_source)
        self.assertEqual(accepted.fidelity_axes[0].axis, "genomic")
        self.assertEqual(accepted.replicates.effective_biological_n, 3)
        self.assertEqual(accepted.patient_relevant_claim_record.claimed_n, 3)
        refused = oncoworlds_model_transport_report({
            "ok": False,
            "schema": "bioprism-mcp/oncoworlds-model-transport/0.1",
            "supported": False,
            "outcome_kind": "refused",
            "refusal_kind": "unverified_model_identity",
            "stage": "model_to_patient_transport",
            "refusal": {"refusal": "unverified_model_identity", "model": "m", "specimen": "s"},
            "refusal_text": "model identity was not verified",
            "fail_closed": True,
            "model_statement": "effect",
            "effect": "effect",
            "model_identity": {"model": "m", "system": "organoid", "source_specimen": "s", "passage": 1, "verified_against_source": False},
            "rests_on": [],
            "fidelity_axes": [],
            "establishment": {"attempted": 1, "established": 1, "selected": False, "selection_modelled": False},
            "replicates": {"technical_wells": 1, "biological_replicates": 1, "effective_biological_n": 1, "claimed_n": 1},
            "transport_assumption_names": [],
            "required_assumptions": [],
        })
        self.assertFalse(refused.ok)
        self.assertEqual(refused.refusal["refusal"], "unverified_model_identity")
        self.assertEqual(refused.refusal_kind, "unverified_model_identity")
        self.assertFalse(refused.model_identity.verified_against_source)
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
        versioned = oncoworlds_methylation_classify_report({
            "ok": True,
            "schema": "bioprism-mcp/oncoworlds-methylation-classify/0.1",
            "outcome_kind": "unclassifiable",
            "classified": False,
            "class": None,
            "classifier": {"name": "demo", "version": "v1", "reference_version": "ref-1", "reporting_threshold": 7000},
            "classifier_threshold": 7000,
            "threshold_declared": True,
            "qc": {"qc": "passed"},
            "tumour_content": {"unobserved": "not_collected"},
            "score_count": 1,
            "score_classes": ["class-b"],
            "caveat_count": 1,
            "nearest_present": True,
            "report": {
                "outcome": {
                    "outcome": "unclassifiable",
                    "reason": {"reason": "no_class_above_threshold", "best": 6500, "threshold": 7000},
                    "nearest": {"label_only": "class-b", "score": {"value": 6500, "calibration": {"method": "isotonic", "version": "cal-1"}}},
                },
                "caveats": ["tumour content is not measured"],
            },
            "guarantees": [],
            "limitations": [],
        })
        self.assertEqual(versioned.outcome_kind, "unclassifiable")
        self.assertEqual(versioned.classifier.reporting_threshold, 7000)
        self.assertEqual(versioned.outcome_record.reason["reason"], "no_class_above_threshold")
        self.assertTrue(versioned.nearest_present)

    def test_radiogenomic_refusal_and_clonal_ambiguity_remain_visible(self) -> None:
        accepted = oncoworlds_radiogenomic_check_report({
            "ok": True,
            "schema": "bioprism-mcp/oncoworlds-radiogenomic-check/0.1",
            "supported": True,
            "outcome_kind": "supported",
            "claim_target": "association",
            "claim_statement": "imaging carries molecular information in this cohort",
            "design": {
                "split_unit": "participant",
                "feature_provenance": "fitted_on_training_split_only",
                "feature_version": "features-v1",
                "external_cohort": None,
                "strata": ["site"],
                "mechanism_strata_present": False,
            },
            "transport_assumption_names": ["same epoch"],
            "required_assumptions": ["same epoch"],
            "supported_claim": {
                "claim": {
                    "target": "association",
                    "statement": "imaging carries molecular information in this cohort",
                },
                "label": {"marker": "idh_mutation", "basis": "detected in region(s) core"},
                "strata": ["site"],
                "transport": {"loss": {"discarded": ["heterogeneity"]}},
            },
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(accepted.supported)
        self.assertEqual(accepted.outcome_kind, "supported")
        self.assertEqual(accepted.supported_claim_record.target, "association")
        self.assertEqual(accepted.design.feature_provenance, "fitted_on_training_split_only")
        refused = oncoworlds_radiogenomic_check_report({
            "ok": False,
            "schema": "bioprism-mcp/oncoworlds-radiogenomic-check/0.1",
            "supported": False,
            "outcome_kind": "refused",
            "claim_target": "association",
            "claim_statement": "imaging predicts molecular state",
            "design": {
                "split_unit": "image",
                "feature_provenance": "fitted_on_training_split_only",
                "feature_version": "features-v1",
                "external_cohort": None,
                "strata": [],
                "mechanism_strata_present": False,
            },
            "transport_assumption_names": [],
            "required_assumptions": [
                "imaging and specimen describe the same disease epoch",
                "the molecular target is defined at the scope the model predicts",
                "the feature representation version is fixed across train and test",
            ],
            "refusal_kind": "leaky_split",
            "stage": "radiogenomic_claim",
            "refusal": {"refusal": "leaky_split", "unit": "image"},
            "refusal_text": "leaky split",
            "fail_closed": True,
        })
        self.assertIsInstance(refused, OncoWorldsRadiogenomicCheckReport)
        self.assertFalse(refused.supported)
        self.assertEqual(refused.schema, "bioprism-mcp/oncoworlds-radiogenomic-check/0.1")
        self.assertEqual(refused.outcome_kind, "refused")
        self.assertEqual(refused.refusal_kind, "leaky_split")
        self.assertEqual(refused.design.split_unit, "image")
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

    def test_era_shift_and_equity_reports_preserve_resource_and_interval_boundaries(self) -> None:
        era = oncoworlds_era_shift_check_report({
            "ok": True,
            "schema": "bioprism-mcp/oncoworlds-era-shift-check/0.1",
            "outcome_kind": "comparable",
            "comparable": True,
            "evidence": {
                "left": {"name": "historical", "site": "site-a", "classification_version": "criteria-a", "entities": ["entity-1"]},
                "right": {"name": "current", "site": "site-b", "classification_version": "criteria-b", "entities": ["entity-1a"]},
                "mapping": {"from": "criteria-a", "to": "criteria-b", "fates": {"entity-1": {"fate": "renamed", "to": "entity-1a"}}},
                "mapping_declared": True,
                "mapping_fate_count": 1,
                "mapping_versions_match": True,
                "same_classification_version": False,
                "left_entity_count": 1,
                "right_entity_count": 1,
                "assay_contexts": [{"site": "site-b", "assay": "methylation", "availability": {"availability": "unavailable_at_site"}, "observation": {"unobserved": "not_collected"}, "negative_call_supported": False, "negative_call_refusal": {"refusal": "resource_absence_read_as_biology"}, "negative_call_refusal_kind": "resource_absence_read_as_biology"}],
                "assay_context_count": 1,
                "descriptor_checks": [{"descriptor": "self_reported_race_or_ethnicity", "descriptor_label": "self-reported race or ethnicity", "use": "stratification", "use_label": "a stratification variable", "administrative": True, "allowed": True}, {"descriptor": "self_reported_race_or_ethnicity", "descriptor_label": "self-reported race or ethnicity", "use": "mechanistic_variable", "use_label": "a mechanistic variable", "administrative": True, "allowed": False, "refusal": {"refusal": "descriptor_used_as_mechanism"}, "refusal_kind": "descriptor_used_as_mechanism", "refusal_text": "administrative descriptor"}],
                "descriptor_check_count": 2,
            },
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(era.comparable)
        self.assertEqual(era.mapping_fate_count, 1)
        self.assertFalse(era.assay_contexts[0].negative_call_supported)
        self.assertEqual(era.assay_contexts[0].negative_call_refusal_kind, "resource_absence_read_as_biology")
        self.assertFalse(era.descriptor_checks[1].allowed)
        refused_era = oncoworlds_era_shift_check_report({
            "ok": False,
            "schema": "bioprism-mcp/oncoworlds-era-shift-check/0.1",
            "outcome_kind": "refused",
            "comparable": False,
            "stage": "classification_era_comparability",
            "refusal": {"refusal": "incomplete_mapping", "entity": "entity-2", "version": "criteria-a"},
            "refusal_kind": "incomplete_mapping",
            "refusal_text": "mapping is incomplete",
            "fail_closed": True,
            "evidence": {
                "left": {"name": "historical", "site": "site-a", "classification_version": "criteria-a", "entities": ["entity-1", "entity-2"]},
                "right": {"name": "current", "site": "site-b", "classification_version": "criteria-b", "entities": ["entity-1a"]},
                "mapping": {"from": "criteria-a", "to": "criteria-b", "fates": {"entity-1": {"fate": "renamed", "to": "entity-1a"}}},
                "mapping_declared": True,
                "mapping_fate_count": 1,
                "mapping_versions_match": True,
                "same_classification_version": False,
                "left_entity_count": 2,
                "right_entity_count": 1,
                "assay_contexts": [],
                "assay_context_count": 0,
                "descriptor_checks": [],
                "descriptor_check_count": 0,
            },
        })
        self.assertFalse(refused_era.ok)
        self.assertEqual(refused_era.refusal_kind, "incomplete_mapping")
        equity = oncoworlds_equity_check_report({
            "ok": True,
            "schema": "bioprism-mcp/oncoworlds-equity-check/0.1",
            "outcome_kind": "equity_report",
            "equity_supported": True,
            "pooled_value": 0.91,
            "subgroups": [{"subgroup": "large", "n": 900, "estimate": 0.93, "interval": {"low": 0.90, "high": 0.95}}, {"subgroup": "small", "n": 3, "estimate": 0.55, "interval": {"low": 0.28, "high": 0.80}}],
            "subgroup_count": 2,
            "interval_count": 2,
            "all_intervals_present": True,
            "report": {"pooled": 0.91},
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(equity.equity_supported)
        self.assertEqual(equity.subgroup_count, 2)
        self.assertTrue(equity.all_intervals_present)
        refused_equity = oncoworlds_equity_check_report({
            "ok": False,
            "schema": "bioprism-mcp/oncoworlds-equity-check/0.1",
            "outcome_kind": "refused",
            "equity_supported": False,
            "stage": "equity_report",
            "refusal": {"refusal": "unquantified_subgroup", "subgroup": "small", "n": 3},
            "refusal_kind": "unquantified_subgroup",
            "refusal_text": "interval is absent",
            "fail_closed": True,
            "pooled_value": 0.91,
            "subgroups": [{"subgroup": "small", "n": 3, "estimate": 0.55, "interval": None}],
            "subgroup_count": 1,
            "interval_count": 0,
            "all_intervals_present": False,
        })
        self.assertFalse(refused_equity.ok)
        self.assertEqual(refused_equity.refusal_kind, "unquantified_subgroup")

        entity = oncoworlds_entity_world_check_report({
            "ok": True,
            "schema": "bioprism-mcp/oncoworlds-entity-world-check/0.1",
            "outcome_kind": "report",
            "all_admissible": False,
            "check_count": 2,
            "refusal_count": 1,
            "checks": {
                "provenance": {"left": "diagnostic_biopsy", "right": "postmortem", "selection_modelled": False, "allowed": False, "refusal": {"refusal": "unmodelled_provenance_selection"}, "refusal_kind": "unmodelled_provenance_selection", "refusal_text": "selection"},
                "benchmark": {"macro_score": 0.88, "per_class_counts": {"common": 300, "rare": 3}, "class_count": 2, "zero_case_classes": [], "report": {}, "allowed": True, "published": {}, "feasibility": {"feasibility": "feasible"}, "feasibility_kind": "feasible", "refusal": None, "refusal_kind": None},
            },
            "guarantees": [],
            "limitations": [],
        })
        self.assertFalse(entity.all_admissible)
        self.assertEqual(entity.refusal_count, 1)
        self.assertEqual(entity.checks["provenance"].refusal_kind, "unmodelled_provenance_selection")
        self.assertEqual(entity.checks["benchmark"].feasibility_kind, "feasible")
        entity_args = OncoWorldsEntityWorldCheckArgs(provenance={"left": "diagnostic_biopsy", "right": "postmortem", "selection_modelled": True})
        self.assertIn("provenance", entity_args.to_mcp_arguments())

    def test_oncoworlds_requests_enforce_bounded_transport_shape(self) -> None:
        args = OncoWorldsClonalHistoryCheckArgs({"subclones": []}, [{"edges": []}])
        self.assertEqual(args.to_mcp_arguments()["candidates"], [{"edges": []}])
        era_args = OncoWorldsEraShiftCheckArgs({"name": "left"}, {"name": "right"}, assay_contexts=({"site": "a"},))
        self.assertEqual(len(era_args.to_mcp_arguments()["assay_contexts"]), 1)
        equity_args = OncoWorldsEquityCheckArgs({"value": 0.9, "subgroups": []})
        self.assertEqual(equity_args.to_mcp_arguments()["pooled"]["value"], 0.9)
        with self.assertRaises(ArgumentError):
            OncoWorldsEntityWorldCheckArgs()
        with self.assertRaises(ArgumentError):
            OncoWorldsMethylationClassifyArgs({}, {str(index): {} for index in range(10_001)}, {})


if __name__ == "__main__":
    unittest.main()
