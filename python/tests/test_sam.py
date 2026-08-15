from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from prism_sdk import (
    AdapterRuntime,
    ProjectionRequest,
    RuntimeStatus,
    SamAdapter,
    parse_sam,
    read_sam,
)
from prism_sdk.errors import ArgumentError


SAM = "\n".join(
    [
        "@HD\tVN:1.6\tSO:coordinate",
        "@SQ\tSN:chr1\tLN:1000\tAS:reference-v1",
        "@RG\tID:rg1\tSM:sample-1",
        "readA\t99\tchr1\t10\t60\t5M1I4M\t=\t30\t100\tACGTACGTAA\tIIIIIIIIII\tNH:i:1\tAS:f:42.5\tRG:Z:rg1",
        "readA\t147\tchr1\t30\t60\t10M\t=\t10\t-100\tTTTTCCCCAA\tJJJJJJJJJJ\tNM:i:1",
        "unmapped\t4\t*\t0\t255\t*\t*\t0\t0\tACGT\t!!!!",
    ]
)


class SamProjectionTests(unittest.TestCase):
    def test_valid_alignment_stream_is_summarized_without_raw_disclosure(self) -> None:
        result = parse_sam(SAM, source_id="alignments", provenance={"accession": "sam-1"})

        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(result.document["manifest"]["adapter"], "bioprism.python.sam_text")
        self.assertEqual(result.document["summary"]["alignments"], 3)
        self.assertEqual(result.document["summary"]["mapped"], 2)
        self.assertEqual(result.document["summary"]["unmapped"], 1)
        self.assertEqual(result.document["summary"]["aligned_bases"], 19)
        self.assertEqual(result.document["summary"]["inserted_bases"], 1)
        self.assertEqual(result.document["summary"]["complete_pairs"], 1)
        self.assertEqual(result.document["summary"]["mapq_unknown_255"], 1)
        self.assertEqual(result.document["header"]["sort_order"], "coordinate")
        self.assertFalse(result.document["manifest"]["read_names_disclosed"])
        self.assertNotIn("readA", str(result.to_wire()))
        self.assertNotIn("ACGTACGTAA", str(result.to_wire()))
        self.assertNotIn("chr1", str(result.to_wire()))
        self.assertEqual(SamAdapter().manifest()["name"], "bioprism.python.sam_text")

    def test_semantic_findings_cover_bounds_and_sort_order(self) -> None:
        invalid = (
            SAM.replace("LN:1000", "LN:20")
            .replace("readA\t99\tchr1\t10", "readA\t99\tchr1\t20")
            .replace("readA\t147\tchr1\t30", "readA\t147\tchr2\t20")
        )
        result = parse_sam(invalid, source_id="invalid-alignments", provenance={"version": "1"}, max_items=1)

        self.assertFalse(result.valid)
        self.assertGreaterEqual(result.document["summary"]["errors"], 1)
        self.assertGreater(result.document["omitted_findings"], 0)
        expanded = parse_sam(SAM.replace("readA\t147\tchr1\t30", "readA\t147\tchr1\t5"), source_id="sort", provenance={"version": "1"}, max_items=8)
        self.assertTrue(any(finding["code"] == "coordinate_sort_violation" for finding in expanded.document["findings"]))

    def test_malformed_tags_cigar_and_bounds_are_refused(self) -> None:
        with self.assertRaisesRegex(ArgumentError, "CIGAR"):
            parse_sam(SAM.replace("5M1I4M", "5Z"), source_id="bad-cigar")
        with self.assertRaisesRegex(ArgumentError, "optional field"):
            parse_sam(SAM.replace("NH:i:1", "BROKEN"), source_id="bad-tag")
        with self.assertRaises(ArgumentError):
            parse_sam(SAM + "\n" + SAM.splitlines()[-1], source_id="record-bound", max_records=3)
        with self.assertRaises(ArgumentError):
            parse_sam(SAM, source_id="byte-bound", max_bytes=len(SAM.encode("utf-8")) - 1)

    def test_raw_reader_and_text_runtime_share_the_same_alignment_audit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "alignments.sam"
            path.write_text(SAM, encoding="utf-8")
            document = read_sam(str(path), source_id="raw-alignments", provenance={"accession": "raw-1"})
            runtime = AdapterRuntime().execute(
                ProjectionRequest(
                    "bioprism.python.sam_text",
                    "runtime-alignments",
                    {"text": SAM},
                    provenance={"accession": "runtime-1"},
                )
            )

        self.assertTrue(document["valid"])
        self.assertEqual(document["manifest"]["optional_tag_values_disclosed"], False)
        self.assertEqual(runtime.status, RuntimeStatus.LOSSY)
        self.assertTrue(runtime.executable)
        self.assertEqual(runtime.document["summary"]["mapped"], 2)


if __name__ == "__main__":
    unittest.main()
