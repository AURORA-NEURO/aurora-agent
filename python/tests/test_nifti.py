from __future__ import annotations

import unittest

from prism_sdk import NiftiAdapter, audit_nifti
from prism_sdk.errors import ArgumentError


AFFINE = [[2.0, 0.0, 0.0, -64.0], [0.0, 2.0, 0.0, -64.0], [0.0, 0.0, 2.0, -36.0], [0.0, 0.0, 0.0, 1.0]]


def image(image_id: str, **overrides: object) -> dict[str, object]:
    value: dict[str, object] = {
        "image_id": image_id,
        "series_id": "sub-01_task-rest",
        "shape": [64, 64, 36, 120],
        "dtype": "float32",
        "affine": AFFINE,
        "qform_code": 1,
        "sform_code": 1,
        "qform_affine": AFFINE,
        "sform_affine": AFFINE,
        "voxel_sizes": [2.0, 2.0, 2.0],
        "axis_codes": ["R", "A", "S"],
        "coordinate_system": "MNI152",
        "reference_space": "MNI152NLin6Asym",
        "units": {"space": "mm", "time": "sec"},
        "intent": "bold",
    }
    value.update(overrides)
    return value


class NiftiProjectionTests(unittest.TestCase):
    def test_valid_header_projection_checks_affine_and_series_metadata(self) -> None:
        result = audit_nifti(
            [image("sub-01_task-rest_bold.nii.gz")],
            source_id="nifti-demo",
            provenance={"accession": "demo", "reader": "nibabel"},
        )

        document = result.to_wire()
        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(document["manifest"]["image_count"], 1)
        self.assertEqual(document["series"][0]["shape"], [64, 64, 36, 120])
        self.assertEqual(document["images"][0]["affine_geometry"]["column_norms"], [2.0, 2.0, 2.0])
        self.assertEqual(len(document["images"][0]["affine_digest"]), 24)
        self.assertEqual(len(document["document_digest"]), 64)

    def test_effective_affine_and_series_inconsistency_are_blocking_errors(self) -> None:
        alternate = [[2.0, 0.0, 0.0, -60.0], [0.0, 2.0, 0.0, -64.0], [0.0, 0.0, 2.0, -36.0], [0.0, 0.0, 0.0, 1.0]]
        result = audit_nifti(
            [
                image("bold-1"),
                image("bold-2", shape=[32, 64, 36, 120], sform_affine=alternate),
            ],
            source_id="invalid-nifti",
            provenance={"accession": "invalid"},
        )

        codes = {finding["code"] for finding in result.findings}
        self.assertFalse(result.valid)
        self.assertFalse(result.publishable)
        self.assertIn("effective_affine_mismatch", codes)
        self.assertIn("series_shape_inconsistent", codes)
        self.assertEqual(result.to_wire()["conformance"]["checks"]["affine"], "fail")

    def test_missing_coordinate_metadata_and_provenance_are_losses_not_silent_defaults(self) -> None:
        result = audit_nifti(
            [
                image(
                    "header-only",
                    qform_code=0,
                    sform_code=0,
                    qform_affine=None,
                    sform_affine=None,
                    axis_codes=None,
                    coordinate_system=None,
                    units=None,
                )
            ],
            source_id="lossy-nifti",
        )

        document = result.to_wire()
        self.assertTrue(result.valid)
        self.assertFalse(result.publishable)
        self.assertEqual(document["semantic_loss"]["max_severity"], "blocking")
        kinds = {loss["kind"] for loss in document["semantic_loss"]["lost"]}
        self.assertIn("coordinate_frame_not_carried", kinds)
        self.assertIn("provenance_unavailable", kinds)

    def test_bounded_invalid_findings_and_manifest_route(self) -> None:
        result = audit_nifti(
            [
                image("duplicate"),
                image("duplicate", dtype="not-a-dtype"),
            ],
            source_id="bounded-nifti",
            provenance={"accession": "bounded"},
            max_items=1,
        )
        document = result.to_wire()
        self.assertFalse(result.valid)
        self.assertEqual(len(document["findings"]), 1)
        self.assertGreater(document["omitted_findings"], 0)
        self.assertGreaterEqual(document["summary"]["errors"], 2)

        manifest = NiftiAdapter().manifest()
        self.assertEqual(manifest["name"], "bioprism.python.nifti_metadata")
        self.assertEqual(manifest["accepted_formats"], ["application/nifti-manifest"])

    def test_input_guards_reject_empty_or_invalid_limits(self) -> None:
        with self.assertRaises(ArgumentError):
            audit_nifti([], source_id="empty")
        with self.assertRaises(ArgumentError):
            audit_nifti([image("one")], source_id="bad", max_images=0)


if __name__ == "__main__":
    unittest.main()
