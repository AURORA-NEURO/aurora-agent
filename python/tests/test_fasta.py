from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from prism_sdk import (
    AdapterRuntime,
    FastaAdapter,
    ProjectionRequest,
    RuntimeStatus,
    parse_fasta,
    read_fasta,
)
from prism_sdk.errors import ArgumentError


FASTA = "; reference comment\n>chr1 primary chromosome\nACGT\nNRY\n>chr2\nGGCC\n>chr3 example\nATGC\n"


class FastaProjectionTests(unittest.TestCase):
    def test_valid_multiline_nucleotide_records_are_summarized_without_disclosure(self) -> None:
        result = parse_fasta(FASTA, source_id="reference", provenance={"accession": "ref-1"}, sequence_type="nucleotide")

        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(result.document["manifest"]["adapter"], "bioprism.python.fasta_text")
        self.assertEqual(result.document["manifest"]["record_count"], 3)
        self.assertEqual(result.document["summary"]["total_bases"], 15)
        self.assertEqual(result.document["summary"]["gc_bases"], 8)
        self.assertEqual(result.document["summary"]["unique_identifier_count"], 3)
        self.assertNotIn("chr1", str(result.to_wire()))
        self.assertNotIn("ACGT", str(result.to_wire()))
        self.assertEqual(FastaAdapter().manifest()["name"], "bioprism.python.fasta_text")

    def test_duplicate_ids_and_alphabet_mismatches_remain_evidence_bearing(self) -> None:
        invalid = ">chr1\nACGT\n>chr1\nPEPT\n"
        result = parse_fasta(invalid, source_id="invalid-reference", provenance={"version": "1"}, sequence_type="nucleotide", max_items=1)

        self.assertFalse(result.valid)
        self.assertGreaterEqual(result.document["summary"]["errors"], 2)
        self.assertGreater(result.document["omitted_findings"], 0)
        expanded = parse_fasta(invalid, source_id="invalid-reference", provenance={"version": "1"}, sequence_type="nucleotide", max_items=2)
        self.assertTrue(any(finding["code"] == "sequence_id_duplicate" for finding in expanded.document["findings"]))
        self.assertTrue(any(finding["code"] == "alphabet_mismatch" for finding in expanded.document["findings"]))

    def test_structure_and_bounds_are_refused(self) -> None:
        with self.assertRaisesRegex(ArgumentError, "before the first header"):
            parse_fasta("ACGT\n>chr1\nAC\n", source_id="no-header")
        with self.assertRaisesRegex(ArgumentError, "no sequence lines"):
            parse_fasta(">empty\n>next\nAC\n", source_id="empty-record")
        with self.assertRaises(ArgumentError):
            parse_fasta(FASTA, source_id="record-bound", max_records=2)
        with self.assertRaises(ArgumentError):
            parse_fasta(FASTA, source_id="byte-bound", max_bytes=len(FASTA.encode("utf-8")) - 1)

    def test_raw_reader_and_runtime_share_the_same_sequence_audit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "reference.fasta"
            path.write_text(FASTA, encoding="utf-8")
            document = read_fasta(str(path), source_id="raw-reference", provenance={"accession": "raw-1"}, sequence_type="nucleotide")
            runtime = AdapterRuntime().execute(
                ProjectionRequest(
                    "bioprism.python.fasta_text",
                    "runtime-reference",
                    {"path": str(path), "sequence_type": "nucleotide"},
                    provenance={"accession": "runtime-1"},
                )
            )

        self.assertTrue(document["valid"])
        self.assertEqual(document["manifest"]["sequence_bases_disclosed"], False)
        self.assertEqual(runtime.status, RuntimeStatus.LOSSY)
        self.assertTrue(runtime.executable)
        self.assertEqual(runtime.document["summary"]["records"], 3)


if __name__ == "__main__":
    unittest.main()
