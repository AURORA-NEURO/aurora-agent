from __future__ import annotations

import unittest

from prism_sdk import BidsAdapter, audit_bids
from prism_sdk.errors import ArgumentError


DATASET = {"Name": "AURORA demo", "BIDSVersion": "1.8.0"}


class BidsManifestTests(unittest.TestCase):
    def test_valid_layout_resolves_sidecars_and_participant_coverage(self) -> None:
        files = [
            "dataset_description.json",
            "participants.tsv",
            "sub-01/func/sub-01_task-rest_bold.nii.gz",
            "sub-01/func/sub-01_task-rest_bold.json",
            "sub-02/func/sub-02_task-rest_bold.nii.gz",
            "sub-02/func/sub-02_task-rest_bold.json",
        ]
        metadata = {
            "dataset_description.json": DATASET,
            "sub-01/func/sub-01_task-rest_bold.json": {"TaskName": "rest", "RepetitionTime": 2.0},
            "sub-02/func/sub-02_task-rest_bold.json": {"TaskName": "rest", "RepetitionTime": 2.0},
        }
        result = audit_bids(
            files,
            source_id="demo",
            metadata=metadata,
            participants_tsv="participant_id\tage\nsub-01\t41\nsub-02\t39\n",
        )

        document = result.to_wire()
        self.assertTrue(result.valid)
        self.assertEqual(document["manifest"]["subject_count"], 2)
        self.assertEqual(document["manifest"]["data_file_count"], 2)
        self.assertEqual(document["participants"]["missing_subjects"], [])
        self.assertEqual(document["resolved_metadata"][0]["metadata"]["TaskName"], "rest")
        self.assertEqual(document["conformance"]["checks"]["sidecar_inheritance"], "pass")
        self.assertEqual(len(document["document_digest"]), 64)

    def test_inheritance_merges_root_and_subject_sidecars_by_specificity(self) -> None:
        files = [
            "dataset_description.json",
            "task-rest_bold.json",
            "sub-01/func/sub-01_task-rest_bold.nii.gz",
            "sub-01/func/sub-01_task-rest_bold.json",
        ]
        result = audit_bids(
            files,
            source_id="inheritance",
            metadata={
                "dataset_description.json": DATASET,
                "task-rest_bold.json": {"TaskName": "rest", "Manufacturer": "root"},
                "sub-01/func/sub-01_task-rest_bold.json": {"RepetitionTime": 2.0},
            },
        )

        resolved = result.to_wire()["resolved_metadata"][0]
        self.assertTrue(result.valid)
        self.assertEqual(resolved["sidecars"], ["task-rest_bold.json", "sub-01/func/sub-01_task-rest_bold.json"])
        self.assertEqual(resolved["metadata"], {"Manufacturer": "root", "RepetitionTime": 2.0, "TaskName": "rest"})

    def test_equal_specificity_sidecar_conflict_is_blocking(self) -> None:
        result = audit_bids(
            [
                "dataset_description.json",
                "sub-01_task-rest_bold.json",
                "ses-01_task-rest_bold.json",
                "sub-01/func/sub-01_ses-01_task-rest_bold.nii.gz",
            ],
            source_id="conflict",
            metadata={
                "dataset_description.json": DATASET,
                "sub-01_task-rest_bold.json": {"TaskName": "rest"},
                "ses-01_task-rest_bold.json": {"TaskName": "memory"},
            },
        )

        self.assertFalse(result.valid)
        self.assertTrue(any(finding["code"] == "metadata_conflict" for finding in result.findings))
        self.assertEqual(result.to_wire()["conformance"]["checks"]["sidecar_inheritance"], "fail")

    def test_dataset_and_participant_errors_are_reported_even_when_findings_are_bounded(self) -> None:
        result = audit_bids(
            [
                "dataset_description.json",
                "sub-01/func/sub-01_task-rest_bold.nii.gz",
                "sub-02/func/sub-02_task-rest_bold.nii.gz",
            ],
            source_id="bounded",
            metadata={"dataset_description.json": DATASET},
            participants_tsv="participant_id\nage\nsub-99\t20\n",
            max_items=1,
        )

        document = result.to_wire()
        self.assertFalse(result.valid)
        self.assertEqual(document["max_items"], 1)
        self.assertEqual(len(document["findings"]), 1)
        self.assertGreater(document["omitted_findings"], 0)
        self.assertGreaterEqual(document["summary"]["errors"], 2)

    def test_derivative_pipeline_requires_its_own_description(self) -> None:
        result = audit_bids(
            [
                "dataset_description.json",
                "derivatives/fmriprep/dataset_description.json",
                "derivatives/fmriprep/sub-01/anat/sub-01_T1w.nii.gz",
            ],
            source_id="derivative",
            metadata={
                "dataset_description.json": DATASET,
                "derivatives/fmriprep/dataset_description.json": {"Name": "fMRIPrep", "BIDSVersion": "1.8.0"},
            },
        )

        self.assertTrue(result.valid)
        self.assertNotIn("derivative_description_missing", {finding["code"] for finding in result.findings})

    def test_adapter_manifest_is_explicit_and_matches_registry_route(self) -> None:
        manifest = BidsAdapter().manifest()
        self.assertEqual(manifest["name"], "bioprism.python.bids_manifest")
        self.assertEqual(manifest["optional_dependency"], None)
        self.assertIn("content_uninterpreted", manifest["declared_loss_kinds"])
        self.assertEqual(manifest["execution"], "python_delegated")

    def test_invalid_or_duplicate_paths_are_rejected_before_audit(self) -> None:
        with self.assertRaises(ArgumentError):
            audit_bids([], source_id="empty")
        with self.assertRaises(ArgumentError):
            audit_bids(["dataset_description.json", "dataset_description.json"], source_id="duplicate")
        with self.assertRaises(ArgumentError):
            audit_bids(["../outside.json"], source_id="traversal")


if __name__ == "__main__":
    unittest.main()
