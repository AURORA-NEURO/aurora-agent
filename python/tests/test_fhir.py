from __future__ import annotations

import json
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
    parse_fhir_ndjson,
    read_fhir_json,
    read_fhir_ndjson,
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

    def test_ndjson_validates_every_record_and_keeps_bundle_reference_checks(self) -> None:
        resources = [entry["resource"] for entry in bundle()["entry"]]
        payload = "\n".join(json.dumps(resource) for resource in resources) + "\n"

        result = parse_fhir_ndjson(payload, source_id="bulk-fhir", provenance={"accession": "bulk-1"}, max_items=1)

        self.assertTrue(result.valid)
        self.assertEqual(result.document["manifest"]["adapter"], "bioprism.python.fhir_ndjson")
        self.assertEqual(result.document["manifest"]["record_count"], 2)
        self.assertEqual(result.document["manifest"]["declared_format"], "application/fhir+ndjson")
        self.assertEqual(result.document["summary"]["unresolved_internal_references"], 0)
        self.assertNotIn("patient-1", str(result.to_wire()))

        with self.assertRaises(ArgumentError):
            parse_fhir_ndjson(payload, source_id="record-bound", max_records=1)
        with self.assertRaises(ArgumentError):
            parse_fhir_ndjson(payload, source_id="byte-bound", max_bytes=len(payload.encode("utf-8")) - 1)
        with self.assertRaisesRegex(ArgumentError, "line 2"):
            parse_fhir_ndjson(
                '{"resourceType":"Patient","id":"patient-1"}\n{"resourceType":',
                source_id="late-malformed-record",
            )

        with self.assertRaises(ArgumentError):
            parse_fhir_ndjson(payload + "\n", source_id="blank-line", provenance={"version": "1"})

    def test_raw_reader_and_runtime_use_the_same_audited_projection(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bundle.json"
            path.write_text(json.dumps(bundle()), encoding="utf-8")
            ndjson_path = Path(directory) / "bundle.ndjson"
            resources = [entry["resource"] for entry in bundle()["entry"]]
            ndjson_path.write_text("\n".join(json.dumps(resource) for resource in resources) + "\n", encoding="utf-8")
            document = read_fhir_json(str(path), source_id="raw-fhir", provenance={"accession": "raw-1"})
            ndjson_document = read_fhir_ndjson(str(ndjson_path), source_id="raw-bulk", provenance={"accession": "bulk-1"})
            result = AdapterRuntime().execute(
                ProjectionRequest(
                    "bioprism.python.fhir_json",
                    "runtime-fhir",
                    {"path": str(path)},
                    provenance={"accession": "runtime-1"},
                )
            )
            ndjson_result = AdapterRuntime().execute(
                ProjectionRequest(
                    "bioprism.python.fhir_ndjson",
                    "runtime-bulk-fhir",
                    {"path": str(ndjson_path)},
                    provenance={"accession": "runtime-bulk-1"},
                )
            )

        self.assertTrue(document["valid"])
        self.assertEqual(document["manifest"]["adapter"], "bioprism.python.fhir_manifest")
        self.assertEqual(ndjson_document["manifest"]["adapter"], "bioprism.python.fhir_ndjson")
        self.assertEqual(result.status, RuntimeStatus.LOSSY)
        self.assertTrue(result.executable)
        self.assertEqual(result.document["manifest"]["bytes_read"], False)
        self.assertEqual(ndjson_result.status, RuntimeStatus.LOSSY)
        self.assertEqual(ndjson_result.document["manifest"]["record_count"], 2)


if __name__ == "__main__":
    unittest.main()
