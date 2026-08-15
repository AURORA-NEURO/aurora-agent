from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from prism_sdk import (
    AdapterRuntime,
    FastqAdapter,
    ProjectionRequest,
    RuntimeStatus,
    parse_fastq,
    read_fastq,
)
from prism_sdk.errors import ArgumentError


FASTQ = """@read/1 comment\nACG\nTN\n+\nIII\nII\n@read/2 comment\nACG\n+read/2\nIII\n@solo 1:N:0:1\nAAA\n+\n!!!\n"""


class FastqProjectionTests(unittest.TestCase):
    def test_valid_multiline_and_paired_reads_are_summarized_without_disclosure(self) -> None:
        result = parse_fastq(FASTQ, source_id="sequencing-run", provenance={"accession": "run-1"})

        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(result.document["manifest"]["adapter"], "bioprism.python.fastq_text")
        self.assertEqual(result.document["manifest"]["record_count"], 3)
        self.assertEqual(result.document["summary"]["total_sequence_bases"], 11)
        self.assertEqual(result.document["summary"]["complete_pairs"], 1)
        self.assertEqual(result.document["summary"]["incomplete_pairs"], 1)
        self.assertEqual(result.document["summary"]["quality_phred_min"], 0)
        self.assertEqual(result.document["summary"]["quality_phred_max"], 40)
        self.assertNotIn("read/1", str(result.to_wire()))
        self.assertNotIn("ACGTN", str(result.to_wire()))
        self.assertNotIn("IIIII", str(result.to_wire()))
        self.assertEqual(FastqAdapter().manifest()["name"], "bioprism.python.fastq_text")

        blocked = parse_fastq(FASTQ, source_id="unlocated-run", max_items=1)
        self.assertFalse(blocked.publishable)
        self.assertEqual(blocked.document["semantic_loss"]["max_severity"], "blocking")
        self.assertEqual(blocked.document["semantic_loss"]["omitted_lost"], 2)

    def test_structural_and_bound_errors_are_refused_even_after_a_valid_prefix(self) -> None:
        with self.assertRaisesRegex(ArgumentError, "quality length"):
            parse_fastq("@first\nACGT\n+\nIII\n", source_id="short-quality")
        with self.assertRaisesRegex(ArgumentError, "does not match"):
            parse_fastq("@first\nACGT\n+other\nIIII\n", source_id="mismatched-plus")
        with self.assertRaisesRegex(ArgumentError, "non-printable"):
            parse_fastq("@first\nACGT\n+\nIII\x01\n", source_id="bad-quality")
        with self.assertRaises(ArgumentError):
            parse_fastq(FASTQ, source_id="record-bound", max_records=2)
        with self.assertRaises(ArgumentError):
            parse_fastq(FASTQ, source_id="byte-bound", max_bytes=len(FASTQ.encode("utf-8")) - 1)

    def test_duplicate_identifiers_are_invalid_but_preview_bounds_do_not_hide_them(self) -> None:
        duplicate = "@read/1\nAC\n+\nII\n@read/1\nGT\n+\nII\n"
        result = parse_fastq(duplicate, source_id="duplicate-read", provenance={"version": "1"}, max_items=1)

        self.assertFalse(result.valid)
        self.assertEqual(result.document["summary"]["reads"], 2)
        self.assertGreaterEqual(result.document["summary"]["errors"], 1)
        self.assertEqual(result.document["omitted_reads"], 1)
        self.assertTrue(any(finding["code"] == "read_id_duplicate" for finding in result.document["findings"]))

    def test_raw_reader_and_runtime_share_the_same_audited_projection(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "reads.fastq"
            path.write_text(FASTQ, encoding="utf-8")
            document = read_fastq(str(path), source_id="raw-fastq", provenance={"accession": "raw-1"})
            runtime = AdapterRuntime().execute(
                ProjectionRequest(
                    "bioprism.python.fastq_text",
                    "runtime-fastq",
                    {"path": str(path)},
                    provenance={"accession": "runtime-1"},
                )
            )

        self.assertTrue(document["valid"])
        self.assertEqual(document["manifest"]["record_count"], 3)
        self.assertEqual(runtime.status, RuntimeStatus.LOSSY)
        self.assertTrue(runtime.executable)
        self.assertEqual(runtime.document["manifest"]["bytes_read"], True)
        self.assertEqual(runtime.document["summary"]["complete_pairs"], 1)


if __name__ == "__main__":
    unittest.main()
