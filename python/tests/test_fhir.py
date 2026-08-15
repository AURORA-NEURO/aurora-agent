from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from prism_sdk import (
    AdapterRuntime,
    FhirAdapter,
    ProjectionRequest,
    RuntimeStatus,
    audit_fhir,
    parse_fhir_json,
    read_fhir_json,
)
from prism_sdk.errors import ArgumentError


def bundle() -> dict[str, object]:
    return {
        "resourceType": "Bundle",
        "id": "bundle-1",
        "type": "collection",
        "entry": [
            {
                "fullUrl": "urn:uuid:patient-1",
                "resource": {
                    "resourceType": "Patient",
                    "id": "patient-1",
                    "meta": {"profile": ["https://example.invalid/fhir/StructureDefinition/patient"]},
                    "gender": "female",
                },
            },
            {
                "resource": {
                    "resourceType": "Observation",
                    "id": "observation-1",
                    "meta": {"profile": ["https://example.invalid/fhir/StructureDefinition/observation"]},
                    "status": "final",
                    "subject": {"reference": "Patient/patient-1"},
                    "code": {"coding": [{"system": "http://loinc.org", "code": "1234-5"}]},
                }
            },
        ],
    }


class FhirProjectionTests(unittest.TestCase):
    def test_valid_bundle_preserves_reference_scope_without_disclosing_ids(self) -> None:
        result = audit_fhir(bundle(), source_id="clinical-bundle", provenance={"accession": "fhir-1"})

        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(result.document["manifest"]["resource_count"], 2)
        self.assertEqual(result.document["summary"]["reference_scope_counts"], {"local": 1})
        self.assertEqual(result.document["summary"]["unresolved_internal_references"], 0)
        self.assertEqual(len(result.document["references"]["patient_reference_digests"]), 1)
        self.assertNotEqual(result.document["references"]["patient_reference_digests"][0], "Patient/patient-1")
        self.assertNotIn("patient-1", str(result.to_wire()))
        self.assertEqual(FhirAdapter().manifest()["name"], "bioprism.python.fhir_manifest")

    def test_missing_provenance_blocks_publication_but_keeps_structural_validity(self) -> None:
        result = audit_fhir(bundle(), source_id="unlocated-fhir")

        self.assertTrue(result.valid)
        self.assertFalse(result.publishable)
        self.assertEqual(result.document["semantic_loss"]["max_severity"], "blocking")
        self.assertEqual(result.document["conformance"]["checks"]["provenance"], "fail")

    def test_invalid_duplicate_resource_ids_are_not_hidden_by_preview_bounds(self) -> None:
        invalid = bundle()
        entries = list(invalid["entry"])
        entries.append({"resource": {"resourceType": "Patient", "id": "patient-1"}})
        invalid["entry"] = entries

        result = audit_fhir(invalid, source_id="duplicate-fhir", provenance={"version": "1"}, max_items=1)

        self.assertFalse(result.valid)
        self.assertEqual(result.document["summary"]["resources"], 3)
        self.assertEqual(result.document["omitted_resources"], 2)
        self.assertTrue(any(finding["code"] == "resource_duplicate" for finding in result.document["findings"]))

    def test_json_parser_rejects_duplicate_keys_and_non_standard_numbers(self) -> None:
        with self.assertRaises(ArgumentError):
            parse_fhir_json('{"resourceType":"Patient","resourceType":"Observation"}', source_id="duplicate")
        with self.assertRaises(ArgumentError):
            parse_fhir_json('{"resourceType":"Patient","value":NaN}', source_id="nan")

    def test_raw_reader_and_runtime_use_the_same_audited_projection(self) -> None:
        import json

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bundle.json"
            path.write_text(json.dumps(bundle()), encoding="utf-8")
            document = read_fhir_json(str(path), source_id="raw-fhir", provenance={"accession": "raw-1"})
            result = AdapterRuntime().execute(
                ProjectionRequest(
                    "bioprism.python.fhir_json",
                    "runtime-fhir",
                    {"path": str(path)},
                    provenance={"accession": "runtime-1"},
                )
            )

        self.assertTrue(document["valid"])
        self.assertEqual(document["manifest"]["adapter"], "bioprism.python.fhir_manifest")
        self.assertEqual(result.status, RuntimeStatus.LOSSY)
        self.assertTrue(result.executable)
        self.assertEqual(result.document["manifest"]["bytes_read"], False)


if __name__ == "__main__":
    unittest.main()
