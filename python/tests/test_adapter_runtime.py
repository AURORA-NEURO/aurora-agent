from __future__ import annotations

import importlib.util
import unittest

from prism_sdk import (
    AdapterRuntime,
    ProjectionRequest,
    RuntimeStatus,
    execute_projection,
)
from prism_sdk.errors import ArgumentError


VCF = """##fileformat=VCFv4.3
##reference=GRCh38
##INFO=<ID=DP,Number=1,Type=Integer,Description="Read depth">
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO
chr1\t10\t.\tA\tG\t50\tPASS\tDP=4
"""


def nifti_payload() -> dict[str, object]:
    affine = [[2.0, 0.0, 0.0, 0.0], [0.0, 2.0, 0.0, 0.0], [0.0, 0.0, 2.0, 0.0], [0.0, 0.0, 0.0, 1.0]]
    return {
        "images": [
            {
                "image_id": "bold.nii.gz",
                "shape": [8, 8, 8, 2],
                "dtype": "float32",
                "affine": affine,
                "qform_code": 1,
                "sform_code": 1,
                "qform_affine": affine,
                "sform_affine": affine,
                "voxel_sizes": [2.0, 2.0, 2.0],
                "axis_codes": ["R", "A", "S"],
                "coordinate_system": "MNI152",
                "units": {"space": "mm", "time": "sec"},
            }
        ]
    }


class AdapterRuntimeTests(unittest.TestCase):
    def test_one_gateway_executes_every_concrete_projection_route(self) -> None:
        runtime = AdapterRuntime()
        requests = [
            ("bioprism.python.vcf_text", {"text": VCF, "reference_build": "GRCh38"}),
            (
                "bioprism.python.bids_manifest",
                {
                    "files": ["dataset_description.json", "sub-01/anat/sub-01_T1w.nii.gz"],
                    "metadata": {"dataset_description.json": {"Name": "demo", "BIDSVersion": "1.8.0"}},
                },
            ),
            (
                "bioprism.python.dicom_metadata",
                {
                    "instances": [
                        {
                            "instance_id": "ct-1",
                            "study_uid": "1.2.3",
                            "series_uid": "1.2.3.1",
                            "sop_instance_uid": "1.2.3.1.1",
                            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2",
                            "frame_of_reference_uid": "1.2.3.2",
                            "modality": "CT",
                            "rows": 2,
                            "columns": 2,
                            "pixel_spacing": [1.0, 1.0],
                            "image_orientation_patient": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                            "image_position_patient": [0.0, 0.0, 0.0],
                        }
                    ]
                },
            ),
            ("bioprism.python.nifti_metadata", nifti_payload()),
            (
                "bioprism.python.anndata_metadata",
                {
                    "dataset": {
                        "n_obs": 1,
                        "n_vars": 1,
                        "X": {"shape": [1, 1], "dtype": "float32"},
                        "obs_index": ["cell-1"],
                        "var_index": ["gene-1"],
                    }
                },
            ),
            (
                "bioprism.python.alignment_metadata",
                {
                    "reference_build": "GRCh38",
                    "references": {"chr1": 100},
                    "records": [{"record_id": "read-1", "read_id": "read-1", "reference_name": "chr1", "start": 1, "cigar": "5M", "reference_end": 6}],
                },
            ),
        ]

        for adapter_id, payload in requests:
            with self.subTest(adapter_id=adapter_id):
                result = runtime.execute(
                    ProjectionRequest(adapter_id, f"source-{adapter_id.rsplit('.', 1)[-1]}", payload, provenance={"accession": "runtime"})
                )
                self.assertTrue(result.accepted)
                self.assertTrue(result.executable)
                self.assertIn(result.status, {RuntimeStatus.SUCCEEDED, RuntimeStatus.LOSSY})
                self.assertEqual(len(result.document_digest or ""), 64)
                self.assertNotIn(VCF, str(result.to_wire()["request"]))

    def test_raw_binary_routes_refuse_explicitly_instead_of_falling_back(self) -> None:
        result = execute_projection(
            "bioprism.python.dicom",
            "raw-dicom",
            {"path": "missing.dcm"},
        )

        if importlib.util.find_spec("pydicom") is None:
            self.assertEqual(result.status, RuntimeStatus.UNSUPPORTED)
            self.assertEqual(result.to_wire()["error"]["kind"], "optional_dependency_missing")
            self.assertIn("pydicom", result.to_wire()["error"]["detail"])
        else:
            self.assertEqual(result.status, RuntimeStatus.REJECTED)
            self.assertEqual(result.to_wire()["error"]["kind"], "argument_error")

    def test_invalid_payload_is_rejected_and_unknown_route_is_typed(self) -> None:
        rejected = execute_projection("bioprism.python.bids_manifest", "bad", {"metadata": {}})
        self.assertEqual(rejected.status, RuntimeStatus.REJECTED)
        self.assertEqual(rejected.to_wire()["error"]["kind"], "argument_error")

        unknown = execute_projection("bioprism.python.not_real", "unknown", {})
        self.assertEqual(unknown.status, RuntimeStatus.UNSUPPORTED)
        self.assertFalse(unknown.accepted)
        self.assertEqual(unknown.to_wire()["error"]["kind"], "unknown_adapter")

    def test_blocking_semantic_loss_is_not_reported_as_success(self) -> None:
        result = execute_projection(
            "bioprism.python.vcf_text",
            "unlocated-vcf",
            {"text": VCF.replace("##reference=GRCh38\n", "")},
        )
        self.assertEqual(result.status, RuntimeStatus.BLOCKED)
        self.assertTrue(result.executable)
        self.assertEqual(result.to_wire()["document"]["semantic_loss"]["max_severity"], "blocking")

    def test_request_validation_bounds_payload_metadata(self) -> None:
        with self.assertRaises(ArgumentError):
            ProjectionRequest("", "source", {})
        with self.assertRaises(ArgumentError):
            ProjectionRequest("adapter", "source", {}, max_items=0)


if __name__ == "__main__":
    unittest.main()
