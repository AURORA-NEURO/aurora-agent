from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from prism_sdk import (
    AdapterRuntime,
    MzmlAdapter,
    ProjectionRequest,
    RuntimeStatus,
    parse_mzml,
    read_mzml,
)
from prism_sdk.errors import ArgumentError


MZML = """<?xml version="1.0" encoding="UTF-8"?>
<mzML xmlns="http://psi.hupo.org/ms/mzml" version="1.1.0">
  <cvList count="1"><cv id="MS" fullName="Proteomics Standards Initiative Mass Spectrometry Ontology"/></cvList>
  <run id="run-1">
    <spectrumList count="2">
      <spectrum index="0" id="scan=1" defaultArrayLength="2">
        <cvParam accession="MS:1000511" name="ms level" value="1"/>
        <scanList count="1"><scan><cvParam accession="MS:1000016" name="scan start time" value="1.5" unitAccession="UO:0000031"/></scan></scanList>
        <binaryDataArrayList count="2">
          <binaryDataArray encodedLength="12">
            <cvParam accession="MS:1000514" name="m/z array"/>
            <cvParam accession="MS:1000523" name="32-bit float"/>
            <binary>QUJDREVGRw==</binary>
          </binaryDataArray>
          <binaryDataArray encodedLength="4">
            <cvParam accession="MS:1000515" name="intensity array"/>
            <cvParam accession="MS:1000574" name="zlib compression"/>
            <binary>AAAA</binary>
          </binaryDataArray>
        </binaryDataArrayList>
      </spectrum>
      <spectrum index="1" id="scan=2" defaultArrayLength="1">
        <cvParam accession="MS:1000511" name="ms level" value="2"/>
        <binaryDataArrayList count="1">
          <binaryDataArray encodedLength="4">
            <cvParam accession="MS:1000595" name="time array"/>
            <binary>BBBB</binary>
          </binaryDataArray>
        </binaryDataArrayList>
      </spectrum>
    </spectrumList>
  </run>
</mzML>
"""


class MzmlProjectionTests(unittest.TestCase):
    def test_valid_namespace_document_audits_spectra_without_binary_disclosure(self) -> None:
        result = parse_mzml(MZML, source_id="proteomics-run", provenance={"accession": "run-1"})

        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(result.document["manifest"]["adapter"], "bioprism.python.mzml_text")
        self.assertEqual(result.document["manifest"]["spectrum_count"], 2)
        self.assertEqual(result.document["summary"]["ms_level_counts"], {"1": 1, "2": 1})
        self.assertEqual(result.document["summary"]["array_type_counts"], {"intensity": 1, "m/z": 1, "time": 1})
        self.assertEqual(result.document["summary"]["compression_counts"], {"zlib": 1})
        self.assertEqual(result.document["summary"]["declared_points"], 3)
        self.assertNotIn("scan=1", str(result.to_wire()))
        self.assertNotIn("QUJDREVGRw==", str(result.to_wire()))
        self.assertEqual(MzmlAdapter().manifest()["name"], "bioprism.python.mzml_text")

    def test_count_drift_duplicate_ids_and_bounds_are_not_hidden_by_preview(self) -> None:
        duplicate = MZML.replace('count="2"', 'count="3"', 1).replace('id="scan=2"', 'id="scan=1"')
        result = parse_mzml(duplicate, source_id="duplicate-spectrum", provenance={"version": "1"}, max_items=1)

        self.assertFalse(result.valid)
        self.assertGreaterEqual(result.document["summary"]["errors"], 2)
        self.assertEqual(result.document["omitted_spectra"], 1)
        self.assertGreater(result.document["omitted_findings"], 0)
        expanded = parse_mzml(duplicate, source_id="duplicate-spectrum", provenance={"version": "1"}, max_items=2)
        self.assertTrue(any(finding["code"] == "spectrum_id_duplicate" for finding in expanded.document["findings"]))
        with self.assertRaises(ArgumentError):
            parse_mzml(MZML, source_id="spectrum-bound", max_spectra=1)

    def test_xml_security_and_missing_provenance_are_explicit(self) -> None:
        with self.assertRaisesRegex(ArgumentError, "DTD"):
            parse_mzml("<!DOCTYPE mzML [<!ENTITY x 'x'>]>" + MZML, source_id="unsafe-xml")
        blocked = parse_mzml(MZML, source_id="unlocated-run", max_items=1)
        self.assertFalse(blocked.publishable)
        self.assertEqual(blocked.document["semantic_loss"]["max_severity"], "blocking")
        self.assertEqual(blocked.document["semantic_loss"]["omitted_lost"], 2)

    def test_raw_reader_and_runtime_share_the_same_metadata_audit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "run.mzML"
            path.write_text(MZML, encoding="utf-8")
            document = read_mzml(str(path), source_id="raw-mzml", provenance={"accession": "raw-1"})
            runtime = AdapterRuntime().execute(
                ProjectionRequest(
                    "bioprism.python.mzml_text",
                    "runtime-mzml",
                    {"path": str(path)},
                    provenance={"accession": "runtime-1"},
                )
            )

        self.assertTrue(document["valid"])
        self.assertEqual(document["manifest"]["binary_arrays_decoded"], False)
        self.assertEqual(runtime.status, RuntimeStatus.LOSSY)
        self.assertTrue(runtime.executable)
        self.assertEqual(runtime.document["summary"]["spectra"], 2)


if __name__ == "__main__":
    unittest.main()
