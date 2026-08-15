from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest

from prism_sdk import (
    AdapterRuntime,
    BatchStatus,
    ProjectionBatchRequest,
    ProjectionRequest,
    RuntimeStatus,
    execute_projection,
    execute_projection_batch,
)
from prism_sdk.errors import ArgumentError


VCF = """##fileformat=VCFv4.3
##reference=GRCh38
##INFO=<ID=DP,Number=1,Type=Integer,Description="Read depth">
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO
chr1\t10\t.\tA\tG\t50\tPASS\tDP=4
"""


HAS_PYSAM = importlib.util.find_spec("pysam") is not None


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
            (
                "bioprism.python.fhir_manifest",
                {
                    "document": {
                        "resourceType": "Patient",
                        "id": "patient-1",
                        "meta": {"profile": ["https://example.invalid/fhir/Patient"]},
                    }
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

    def test_heterogeneous_batch_is_deterministic_and_preserves_member_documents(self) -> None:
        requests = (
            ProjectionRequest(
                "bioprism.python.vcf_text",
                "batch-vcf",
                {"text": VCF, "reference_build": "GRCh38"},
                provenance={"accession": "batch-vcf"},
                max_items=10,
            ),
            ProjectionRequest(
                "bioprism.python.fhir_manifest",
                "batch-fhir",
                {"document": {"resourceType": "Patient", "id": "patient-1"}},
                provenance={"accession": "batch-fhir"},
                max_items=10,
            ),
        )
        first = execute_projection_batch(requests, max_total_items=20)
        second = execute_projection_batch(requests, max_total_items=20)

        self.assertEqual(first.status, BatchStatus.SUCCEEDED)
        self.assertEqual(first.to_wire()["status_counts"], {"lossy": 1, "succeeded": 1})
        self.assertEqual(first.batch_digest, second.batch_digest)
        self.assertEqual(len(first.document_digests), 2)
        self.assertNotIn(VCF, str(first.to_wire()["request"]))
        self.assertEqual(first.to_wire()["result_count"], 2)

    def test_batch_reports_partial_completion_and_preserves_refusal_evidence(self) -> None:
        good = ProjectionRequest(
            "bioprism.python.vcf_text",
            "batch-good",
            {"text": VCF, "reference_build": "GRCh38"},
            provenance={"accession": "good"},
            max_items=5,
        )
        bad = ProjectionRequest("bioprism.python.not_real", "batch-bad", {"secret": VCF}, max_items=5)

        result = execute_projection_batch((good, bad), max_total_items=10)

        self.assertEqual(result.status, BatchStatus.PARTIAL)
        self.assertEqual(result.omitted_requests, 0)
        self.assertEqual(result.results[1].status, RuntimeStatus.UNSUPPORTED)
        self.assertEqual(result.to_wire()["status_counts"], {"succeeded": 1, "unsupported": 1})
        self.assertNotIn(VCF, str(result.to_wire()["request"]))
        self.assertEqual(result.to_wire()["results"][1]["error"]["kind"], "unknown_adapter")

    def test_stop_on_error_reports_omitted_requests_instead_of_silently_dropping_them(self) -> None:
        good = ProjectionRequest("bioprism.python.vcf_text", "first", {"text": VCF}, max_items=10)
        bad = ProjectionRequest("bioprism.python.not_real", "second", {}, max_items=10)
        later = ProjectionRequest("bioprism.python.vcf_text", "third", {"text": VCF}, max_items=10)

        result = execute_projection_batch((good, bad, later), stop_on_error=True, max_total_items=30)

        self.assertEqual(result.status, BatchStatus.PARTIAL)
        self.assertTrue(result.stopped_on_error)
        self.assertEqual(result.omitted_requests, 1)
        self.assertEqual(result.to_wire()["result_count"], 2)

    def test_batch_bounds_reject_empty_requests_and_unbounded_total_preview(self) -> None:
        with self.assertRaises(ArgumentError):
            ProjectionBatchRequest(())
        request = ProjectionRequest("bioprism.python.vcf_text", "bounded", {"text": VCF}, max_items=2)
        with self.assertRaises(ArgumentError):
            ProjectionBatchRequest((request,), max_total_items=1)
        with self.assertRaises(ArgumentError):
            execute_projection_batch("not-a-sequence")  # type: ignore[arg-type]

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

    def test_pysam_routes_refuse_explicitly_when_dependency_is_unavailable(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "existing.data"
            path.write_bytes(b"not a biological file")
            for adapter_id, dependency in (
                ("bioprism.python.vcf_indexed", "pysam"),
                ("bioprism.python.bam_cram", "pysam"),
            ):
                with self.subTest(adapter_id=adapter_id):
                    result = execute_projection(adapter_id, "raw-indexed", {"path": str(path)})
                    if HAS_PYSAM:
                        self.assertEqual(result.status, RuntimeStatus.REJECTED)
                        self.assertEqual(result.to_wire()["error"]["kind"], "argument_error")
                    else:
                        self.assertEqual(result.status, RuntimeStatus.UNSUPPORTED)
                        self.assertEqual(result.to_wire()["error"]["kind"], "optional_dependency_missing")
                        self.assertIn(dependency, result.to_wire()["error"]["detail"])

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
