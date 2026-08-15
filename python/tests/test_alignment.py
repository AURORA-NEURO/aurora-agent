from __future__ import annotations

import unittest

from prism_sdk import AlignmentAdapter, audit_alignments
from prism_sdk.errors import ArgumentError


REFERENCES = {"chr1": 1_000, "chr2": 500}


def record(record_id: str, start: int, **overrides: object) -> dict[str, object]:
    value: dict[str, object] = {
        "record_id": record_id,
        "read_id": f"read-{record_id.rsplit('/', 1)[0] if '/' in record_id else record_id}",
        "reference_name": "chr1",
        "start": start,
        "reference_end": start + 9,
        "cigar": "5M1I4M",
        "flags": 0x1 | (0x40 if record_id.endswith("/1") else 0x80),
        "mapping_quality": 60,
        "sequence_length": 10,
        "mate_reference_name": "chr1",
        "mate_start": start + 20,
        "template_length": 30,
        "read_group": "rg-1",
    }
    value.update(overrides)
    return value


class AlignmentProjectionTests(unittest.TestCase):
    def test_valid_coordinate_sorted_pair_and_coverage_projection(self) -> None:
        result = audit_alignments(
            REFERENCES,
            [record("pair/1", 10), record("pair/2", 20)],
            source_id="alignments",
            reference_build="GRCh38",
            provenance={"accession": "bam-1", "reader": "pysam"},
        )

        document = result.to_wire()
        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(document["manifest"]["mapped_count"], 2)
        self.assertEqual(document["summary"]["paired_reads"], 1)
        self.assertEqual(document["coverage"][0]["mapped_bases"], 18)
        self.assertEqual(document["records"][0]["reference_span"], 9)
        self.assertEqual(len(document["document_digest"]), 64)

    def test_cigar_and_coordinate_invariants_are_blocking_errors(self) -> None:
        result = audit_alignments(
            REFERENCES,
            [
                record("bad-1", 100, cigar="5M", reference_end=101),
                record("bad-2", 50, reference_name="missing", reference_end=60),
            ],
            source_id="invalid-alignments",
            reference_build="GRCh38",
            provenance={"accession": "invalid"},
        )

        codes = {finding["code"] for finding in result.findings}
        self.assertFalse(result.valid)
        self.assertFalse(result.publishable)
        self.assertIn("cigar_coordinate_mismatch", codes)
        self.assertIn("reference_unknown", codes)
        self.assertEqual(result.to_wire()["conformance"]["checks"]["cigar"], "fail")

    def test_missing_reference_build_and_provenance_are_losses_not_defaults(self) -> None:
        result = audit_alignments(
            REFERENCES,
            [record("unlocated", 10)],
            source_id="unlocated-alignments",
        )

        document = result.to_wire()
        self.assertTrue(result.valid)
        self.assertFalse(result.publishable)
        self.assertEqual(document["semantic_loss"]["max_severity"], "blocking")
        kinds = {loss["kind"] for loss in document["semantic_loss"]["lost"]}
        self.assertIn("coordinate_frame_not_carried", kinds)
        self.assertIn("provenance_unavailable", kinds)

    def test_bounded_findings_and_adapter_manifest_are_explicit(self) -> None:
        result = audit_alignments(
            REFERENCES,
            [record("dup", 10), record("dup", 5, flags=0x4)],
            source_id="bounded-alignments",
            reference_build="GRCh38",
            provenance={"accession": "bounded"},
            max_items=1,
        )
        document = result.to_wire()
        self.assertFalse(result.valid)
        self.assertEqual(len(document["findings"]), 1)
        self.assertGreater(document["omitted_findings"], 0)
        self.assertGreaterEqual(document["summary"]["errors"], 1)

        manifest = AlignmentAdapter().manifest()
        self.assertEqual(manifest["name"], "bioprism.python.alignment_metadata")
        self.assertEqual(manifest["accepted_formats"], ["application/alignment-manifest"])

    def test_input_guards_reject_empty_sources_and_bad_limits(self) -> None:
        with self.assertRaises(ArgumentError):
            audit_alignments({}, [], source_id="empty")
        with self.assertRaises(ArgumentError):
            audit_alignments(REFERENCES, [record("one", 1)], source_id="bad", max_records=0)


if __name__ == "__main__":
    unittest.main()
