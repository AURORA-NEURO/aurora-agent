from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from prism_sdk import (
    AdapterRuntime,
    BedAdapter,
    ProjectionRequest,
    RuntimeStatus,
    parse_bed,
    read_bed,
)
from prism_sdk.errors import ArgumentError


BED = """# bounded interval fixture
track name=private-track description=not-disclosed
chr1\t100\t200\ttranscript-1\t900\t+\t120\t190\t255,0,0\t2\t50,40,\t0,60,
chr1\t300\t350\tpeak-1\t.\t.\n"""


class BedProjectionTests(unittest.TestCase):
    def test_bed12_and_bed6_geometry_are_audited_without_label_disclosure(self) -> None:
        result = parse_bed(BED, source_id="intervals", provenance={"accession": "bed-1"})

        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(result.document["manifest"]["adapter"], "bioprism.python.bed_text")
        self.assertEqual(result.document["manifest"]["feature_count"], 2)
        self.assertEqual(result.document["manifest"]["directive_count"], 1)
        self.assertEqual(result.document["summary"]["total_span"], 150)
        self.assertEqual(result.document["summary"]["total_block_span"], 140)
        self.assertEqual(result.document["summary"]["total_blocks"], 3)
        self.assertEqual(result.document["summary"]["scored_features"], 1)
        self.assertEqual(result.document["summary"]["score_max"], 900)
        self.assertEqual(result.document["summary"]["strand_counts"], {"+": 1, ".": 1})
        self.assertEqual(result.document["features"][0]["block_sizes"], [50, 40])
        self.assertEqual(result.document["features"][0]["block_starts"], [0, 60])
        self.assertEqual(BedAdapter().manifest()["name"], "bioprism.python.bed_text")
        wire = str(result.to_wire())
        self.assertNotIn("transcript-1", wire)
        self.assertNotIn("private-track", wire)
        self.assertNotIn("chr1", wire)

    def test_missing_provenance_is_explicitly_non_publishable(self) -> None:
        result = parse_bed("chr1\t0\t10\n", source_id="no-provenance")

        self.assertTrue(result.valid)
        self.assertFalse(result.publishable)
        self.assertEqual(result.document["conformance"]["checks"]["provenance"], "fail")
        self.assertEqual(result.document["semantic_loss"]["max_severity"], "blocking")

    def test_duplicate_and_unsorted_intervals_are_findings_not_silent_normalization(self) -> None:
        result = parse_bed(
            "chr2\t20\t30\tdup\nchr1\t0\t10\tdup\nchr1\t0\t10\tother\n",
            source_id="findings",
            provenance={"version": "1"},
        )

        self.assertTrue(result.valid)
        self.assertFalse(result.document["summary"]["coordinate_sorted"])
        self.assertEqual(result.document["summary"]["duplicate_interval_count"], 1)
        self.assertEqual(result.document["summary"]["duplicate_name_count"], 1)
        codes = {finding["code"] for finding in result.document["findings"]}
        self.assertIn("coordinate_sort_violation", codes)
        self.assertIn("interval_duplicate", codes)
        self.assertIn("name_duplicate", codes)

    def test_malformed_coordinates_blocks_rgb_and_bounds_are_refused(self) -> None:
        with self.assertRaisesRegex(ArgumentError, "chromStart < chromEnd"):
            parse_bed("chr1\t10\t10\n", source_id="empty")
        with self.assertRaisesRegex(ArgumentError, "blockCount"):
            parse_bed("chr1\t0\t100\tname\t0\t+\t0\t100\t.\t2\t100,\t0,\n", source_id="block-count")
        with self.assertRaisesRegex(ArgumentError, "beyond"):
            parse_bed("chr1\t0\t100\tname\t0\t+\t0\t100\t.\t1\t101\t0\n", source_id="block-bound")
        with self.assertRaisesRegex(ArgumentError, "itemRgb"):
            parse_bed("chr1\t0\t10\tname\t0\t+\t0\t10\t999,0,0\n", source_id="rgb")
        with self.assertRaises(ArgumentError):
            parse_bed(BED, source_id="feature-bound", max_features=1)
        with self.assertRaises(ArgumentError):
            parse_bed(BED, source_id="byte-bound", max_bytes=len(BED.encode("utf-8")) - 1)

    def test_raw_reader_and_runtime_share_the_same_bounded_audit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "intervals.bed"
            path.write_text(BED, encoding="utf-8")
            document = read_bed(str(path), source_id="raw-bed", provenance={"accession": "raw-1"})
            runtime = AdapterRuntime().execute(
                ProjectionRequest(
                    "bioprism.python.bed_text",
                    "runtime-bed",
                    {"path": str(path)},
                    provenance={"accession": "runtime-1"},
                )
            )

        self.assertTrue(document["valid"])
        self.assertEqual(document["manifest"]["chromosome_labels_disclosed"], False)
        self.assertEqual(runtime.status, RuntimeStatus.LOSSY)
        self.assertTrue(runtime.executable)
        self.assertEqual(runtime.document["summary"]["features"], 2)


if __name__ == "__main__":
    unittest.main()
