from __future__ import annotations

import unittest

from prism_sdk import VcfAdapter, VcfParseError, parse_vcf
from prism_sdk.errors import ArgumentError


VCF = """##fileformat=VCFv4.3
##reference=GRCh38
##INFO=<ID=DP,Number=1,Type=Integer,Description="Read depth">
##INFO=<ID=AF,Number=A,Type=Float,Description="Allele frequency">
##FORMAT=<ID=GT,Number=1,Type=String,Description="Genotype">
##FORMAT=<ID=DP,Number=1,Type=Integer,Description="Sample depth">
##contig=<ID=chr7,assembly=GRCh38,length=159345973>
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\ttumor\tnormal
chr7\t140453136\tvar-1\tA\tT,C\t99.1234567890123456789\tPASS\tDP=42;AF=0.5,0.25\tGT:DP\t0/1:30\t0|0:40
chr7\t140453137\t.\tG\tA\t.\tq10\tDP=10\tGT:DP\t1/1:8\t./.:.
"""


class VcfReaderTests(unittest.TestCase):
    def test_reader_preserves_multiallelic_values_genotypes_raw_spellings_and_digest(self) -> None:
        result = parse_vcf(
            VCF,
            source_id="cohort-1",
            provenance={"accession": "db:cohort-1", "version": "2026.08"},
            max_items=10,
        )
        document = result.to_wire()
        self.assertEqual(document["schema"], "bioprism-python-vcf/0.1")
        self.assertEqual(document["manifest"]["reference_build"], "GRCh38")
        self.assertEqual(document["variant_count"], 2)
        variant = document["variants"][0]
        self.assertEqual(variant["alt"], ["T", "C"])
        self.assertEqual(variant["info"]["DP"], 42)
        self.assertEqual(variant["info"]["AF"], [0.5, 0.25])
        self.assertEqual(variant["info_raw"]["AF"], "0.5,0.25")
        self.assertEqual(variant["samples"]["tumor"]["GT"]["alleles"], [0, 1])
        self.assertFalse(variant["samples"]["tumor"]["GT"]["phased"])
        self.assertEqual(variant["samples"]["normal"]["GT"]["phased"], True)
        self.assertEqual(variant["samples"]["normal"]["DP"], 40)
        self.assertEqual(len(document["document_digest"]), 64)
        self.assertGreater(document["semantic_loss"]["lost_count"], 0)
        self.assertTrue(
            any(loss["kind"] == "precision_reduced" for loss in document["semantic_loss"]["lost"])
        )

    def test_adapter_manifest_is_explicit_and_matches_the_registry_boundary(self) -> None:
        adapter = VcfAdapter()
        manifest = adapter.manifest()
        self.assertEqual(manifest["name"], "bioprism.python.vcf_text")
        self.assertEqual(manifest["optional_dependency"], None)
        self.assertIn("type_undetermined", manifest["declared_loss_kinds"])
        self.assertEqual(adapter.parse(VCF, source_id="facade", reference_build="GRCh38").variants[0]["pos"], 140453136)

    def test_missing_reference_and_provenance_are_losses_not_inferred_defaults(self) -> None:
        result = parse_vcf(
            "##fileformat=VCFv4.2\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t1\t.\tA\tG\t.\tPASS\t.\n",
            source_id="unlocated",
        )
        loss = result.semantic_loss
        self.assertEqual(loss["audit"], "lossy")
        self.assertEqual(loss["max_severity"], "blocking")
        kinds = {entry["kind"] for entry in loss["lost"]}
        self.assertIn("coordinate_frame_not_carried", kinds)
        self.assertIn("provenance_unavailable", kinds)
        self.assertIsNone(result.to_wire()["header"]["reference_build"])

    def test_caller_reference_is_checked_against_header_reference(self) -> None:
        result = parse_vcf(
            VCF,
            source_id="mismatch",
            reference_build="GRCh37",
            provenance={"accession": "a"},
        )
        losses = result.semantic_loss["lost"]
        self.assertTrue(
            any("disagrees" in entry["detail"] for entry in losses if entry["kind"] == "coordinate_frame_not_carried")
        )
        self.assertEqual(result.to_wire()["header"]["reference_source"], "caller")

    def test_disclosure_is_bounded_but_validation_covers_all_records(self) -> None:
        source = "##fileformat=VCFv4.2\n##reference=GRCh38\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\n" + "\n".join(
            f"chr1\t{index}\t.\tA\tG\t.\tPASS\t." for index in range(1, 6)
        ) + "\n"
        result = parse_vcf(source, source_id="bounded", provenance={"accession": "a"}, max_items=2)
        document = result.to_wire()
        self.assertEqual(document["variant_count"], 5)
        self.assertEqual(len(document["variants"]), 2)
        self.assertEqual(document["omitted_variants"], 3)
        self.assertEqual(document["semantic_loss"]["mapped_count"], 5)

    def test_unknown_info_and_format_types_remain_raw_and_are_reported(self) -> None:
        source = """##fileformat=VCFv4.2
##reference=GRCh38
##FORMAT=<ID=GT,Number=1,Type=String,Description="Genotype">
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tsample
chr1\t2\t.\tA\tG\t.\tPASS\tNEW=abc\tGT:NEWFMT\t0/1:opaque
"""
        result = parse_vcf(source, source_id="unknown", provenance={"accession": "a"})
        variant = result.to_wire()["variants"][0]
        self.assertEqual(variant["info"]["NEW"], "abc")
        self.assertEqual(variant["samples"]["sample"]["NEWFMT"], "opaque")
        self.assertIn("NEW", variant["info_raw"])
        self.assertTrue(all(entry["kind"] == "type_undetermined" for entry in result.semantic_loss["lost"]))

    def test_malformed_structure_and_bounds_fail_closed(self) -> None:
        with self.assertRaises(VcfParseError):
            parse_vcf("##fileformat=VCFv4.2\n#CHROM\tPOS\tBAD\n", source_id="bad")
        with self.assertRaises(ArgumentError):
            parse_vcf(
                "##fileformat=VCFv4.2\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t1\t.\tA\tG\t.\tPASS\t.\n",
                source_id="bad-gt",
                max_records=0,
            )
        with self.assertRaises(ArgumentError):
            parse_vcf("##fileformat=VCFv4.2\n", source_id="huge", max_bytes=1)


if __name__ == "__main__":
    unittest.main()
