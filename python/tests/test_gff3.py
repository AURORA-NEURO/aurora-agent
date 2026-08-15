from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from prism_sdk import (
    AdapterRuntime,
    Gff3Adapter,
    ProjectionRequest,
    RuntimeStatus,
    parse_gff3,
    read_gff3,
)
from prism_sdk.errors import ArgumentError


GFF3 = """##gff-version 3
##sequence-region chr1 1 1000
chr1\tRefSeq\tgene\t1\t100\t.\t+\t.\tID=gene1;Name=example%20gene
chr1\tRefSeq\tmRNA\t1\t100\t.\t+\t.\tID=tx1;Parent=gene1
chr1\tRefSeq\tCDS\t10\t50\t.\t+\t0\tID=cds1;Parent=tx1;Note=bounded
##FASTA
>chr1
ACGTACGT
"""

GTF = """# GTF-style annotation
chr1\tsource\tgene\t1\t10\t.\t+\t.\tgene_id \"gene1\"; transcript_id \"tx1\";
chr1\tsource\texon\t1\t10\t.\t+\t.\tgene_id \"gene1\"; transcript_id \"tx1\";
"""


class Gff3ProjectionTests(unittest.TestCase):
    def test_valid_hierarchy_and_embedded_fasta_are_audited_without_disclosure(self) -> None:
        result = parse_gff3(GFF3, source_id="annotation", provenance={"accession": "ann-1"})

        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(result.document["manifest"]["adapter"], "bioprism.python.gff3_text")
        self.assertEqual(result.document["manifest"]["feature_count"], 3)
        self.assertEqual(result.document["manifest"]["embedded_fasta_lines"], 2)
        self.assertEqual(result.document["summary"]["parent_edges"], 2)
        self.assertEqual(result.document["summary"]["unresolved_parents"], 0)
        self.assertEqual(result.document["summary"]["feature_type_counts"], {"CDS": 1, "gene": 1, "mRNA": 1})
        self.assertNotIn("gene1", str(result.to_wire()))
        self.assertNotIn("example%20gene", str(result.to_wire()))
        self.assertNotIn("ACGTACGT", str(result.to_wire()))
        self.assertEqual(Gff3Adapter().manifest()["name"], "bioprism.python.gff3_text")

    def test_gtf_attributes_and_trailing_semicolons_are_supported(self) -> None:
        result = parse_gff3(GTF, source_id="gtf", provenance={"version": "1"}, annotation_format="gtf")

        self.assertTrue(result.valid)
        self.assertEqual(result.document["manifest"]["annotation_format"], "gtf")
        self.assertEqual(result.document["summary"]["features"], 2)

    def test_parent_errors_duplicate_ids_and_preview_bounds_remain_explicit(self) -> None:
        invalid = """##gff-version 3
chr1\tsource\tgene\t1\t10\t.\t+\t.\tID=dup
chr1\tsource\texon\t1\t10\t.\t+\t.\tID=dup;Parent=missing
"""
        result = parse_gff3(invalid, source_id="invalid-annotation", provenance={"version": "1"}, max_items=1)

        self.assertFalse(result.valid)
        self.assertGreaterEqual(result.document["summary"]["errors"], 2)
        self.assertGreater(result.document["omitted_findings"], 0)
        expanded = parse_gff3(invalid, source_id="invalid-annotation", provenance={"version": "1"}, max_items=2)
        self.assertTrue(any(finding["code"] == "feature_id_duplicate" for finding in expanded.document["findings"]))
        self.assertTrue(any(finding["code"] == "parent_unresolved" for finding in expanded.document["findings"]))

    def test_malformed_rows_and_bounds_are_refused(self) -> None:
        with self.assertRaisesRegex(ArgumentError, "exactly nine"):
            parse_gff3("chr1\tsource\tgene\t1\t10\n", source_id="short-row")
        with self.assertRaisesRegex(ArgumentError, "phase"):
            parse_gff3("chr1\tsource\tCDS\t1\t10\t.\t+\t.\tID=cds\n", source_id="bad-cds")
        with self.assertRaises(ArgumentError):
            parse_gff3(GFF3, source_id="feature-bound", max_features=2)
        with self.assertRaises(ArgumentError):
            parse_gff3(GFF3, source_id="byte-bound", max_bytes=len(GFF3.encode("utf-8")) - 1)

    def test_raw_reader_and_runtime_share_the_same_feature_audit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "annotation.gff3"
            path.write_text(GFF3, encoding="utf-8")
            document = read_gff3(str(path), source_id="raw-annotation", provenance={"accession": "raw-1"})
            runtime = AdapterRuntime().execute(
                ProjectionRequest(
                    "bioprism.python.gff3_text",
                    "runtime-annotation",
                    {"path": str(path)},
                    provenance={"accession": "runtime-1"},
                )
            )

        self.assertTrue(document["valid"])
        self.assertEqual(document["manifest"]["attribute_values_disclosed"], False)
        self.assertEqual(runtime.status, RuntimeStatus.LOSSY)
        self.assertTrue(runtime.executable)
        self.assertEqual(runtime.document["summary"]["features"], 3)


if __name__ == "__main__":
    unittest.main()
