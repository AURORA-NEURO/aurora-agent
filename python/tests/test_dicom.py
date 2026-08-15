from __future__ import annotations

import unittest

from prism_sdk import DicomAdapter, audit_dicom
from prism_sdk.errors import ArgumentError


STUDY = "1.2.840.113619.2.55.3.604688435.2.1"
SERIES = f"{STUDY}.1"
SOP_CLASS = "1.2.840.10008.5.1.4.1.1.2"
FRAME = f"{STUDY}.2"


def instance(instance_id: str, z: float, **overrides: object) -> dict[str, object]:
    value: dict[str, object] = {
        "instance_id": instance_id,
        "study_uid": STUDY,
        "series_uid": SERIES,
        "sop_instance_uid": f"{SERIES}.{int(z) + 1}",
        "sop_class_uid": SOP_CLASS,
        "frame_of_reference_uid": FRAME,
        "modality": "CT",
        "rows": 512,
        "columns": 512,
        "pixel_spacing": [0.5, 0.5],
        "image_orientation_patient": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        "image_position_patient": [0.0, 0.0, z],
        "instance_number": int(z) + 1,
        "tags": {"0028,0010": 512, "0028,0011": 512},
    }
    value.update(overrides)
    return value


class DicomProjectionTests(unittest.TestCase):
    def test_valid_series_checks_geometry_and_keeps_identifiers_digest_bound(self) -> None:
        result = audit_dicom(
            [instance("ct-1", 0.0), instance("ct-2", 1.0), instance("ct-3", 2.0)],
            source_id="ct-study",
            provenance={"accession": "study-1", "reader": "pydicom"},
        )

        document = result.to_wire()
        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(document["manifest"]["study_count"], 1)
        self.assertEqual(document["manifest"]["series_count"], 1)
        self.assertEqual(document["summary"]["modality_counts"], {"CT": 3})
        self.assertEqual(document["series"][0]["geometry"]["slice_spacing"], 1.0)
        self.assertEqual(document["manifest"]["patient_identifiers_disclosed"], False)
        self.assertNotIn("PatientName", str(document))
        self.assertEqual(len(document["document_digest"]), 64)

    def test_hierarchy_and_geometry_inconsistencies_are_blocking_errors(self) -> None:
        changed_frame = instance("ct-2", 1.0, frame_of_reference_uid=f"{STUDY}.99")
        duplicate_sop = instance("ct-3", 2.0, sop_instance_uid=f"{SERIES}.1")
        result = audit_dicom(
            [instance("ct-1", 0.0), changed_frame, duplicate_sop],
            source_id="invalid-study",
            provenance={"accession": "study-2"},
        )

        codes = {finding["code"] for finding in result.findings}
        self.assertFalse(result.valid)
        self.assertFalse(result.publishable)
        self.assertIn("frame_of_reference_mismatch", codes)
        self.assertIn("sop_instance_duplicate", codes)
        self.assertEqual(result.to_wire()["conformance"]["checks"]["identity_hierarchy"], "fail")

    def test_missing_geometry_and_provenance_are_losses_separate_from_structural_validity(self) -> None:
        projection = instance(
            "enhanced-1",
            0.0,
            frame_of_reference_uid=None,
            image_orientation_patient=None,
            image_position_patient=None,
            number_of_frames=2,
        )
        result = audit_dicom([projection], source_id="lossy-study")

        document = result.to_wire()
        self.assertTrue(result.valid)
        self.assertFalse(result.publishable)
        kinds = {loss["kind"] for loss in document["semantic_loss"]["lost"]}
        self.assertIn("coordinate_frame_not_carried", kinds)
        self.assertIn("provenance_unavailable", kinds)
        self.assertEqual(document["semantic_loss"]["max_severity"], "blocking")
        self.assertEqual(document["conformance"]["checks"]["provenance"], "loss")

    def test_bounded_disclosure_does_not_hide_structural_invalidity(self) -> None:
        result = audit_dicom(
            [
                instance("ct-1", 0.0),
                instance("ct-2", 1.0, sop_instance_uid=f"{SERIES}.1"),
                instance("ct-3", 2.0, modality="bad"),
            ],
            source_id="bounded",
            provenance={"accession": "bounded"},
            max_items=1,
        )

        document = result.to_wire()
        self.assertFalse(result.valid)
        self.assertEqual(len(document["findings"]), 1)
        self.assertGreater(document["omitted_findings"], 0)
        self.assertGreaterEqual(document["summary"]["errors"], 2)

    def test_adapter_manifest_and_input_guards_are_explicit(self) -> None:
        manifest = DicomAdapter().manifest()
        self.assertEqual(manifest["name"], "bioprism.python.dicom_metadata")
        self.assertEqual(manifest["accepted_formats"], ["application/dicom-manifest"])
        self.assertIn("coordinate_frame_not_carried", manifest["declared_loss_kinds"])
        with self.assertRaises(ArgumentError):
            audit_dicom([], source_id="empty")
        with self.assertRaises(ArgumentError):
            audit_dicom([instance("ct", 0.0)], source_id="bad", max_items=0)


if __name__ == "__main__":
    unittest.main()
