from __future__ import annotations

import unittest

from prism_sdk import AnnDataAdapter, audit_anndata
from prism_sdk.errors import ArgumentError


def dataset(**overrides: object) -> dict[str, object]:
    value: dict[str, object] = {
        "n_obs": 3,
        "n_vars": 2,
        "X": {"shape": [3, 2], "dtype": "float32", "format": "csr", "nnz": 4, "indptr_length": 4, "indices_length": 4},
        "obs_index": ["cell-1", "cell-2", "cell-3"],
        "var_index": ["gene-1", "gene-2"],
        "obs": {"cell_type": {"length": 3, "dtype": "category", "categories": ["T", "B"]}},
        "var": {"symbol": {"length": 2, "dtype": "string"}},
        "layers": {"counts": {"shape": [3, 2], "dtype": "int32", "format": "csr", "nnz": 4, "indptr_length": 4, "indices_length": 4}},
        "obsm": {"X_pca": {"shape": [3, 2], "dtype": "float32"}},
        "varm": {"PCs": {"shape": [2, 2], "dtype": "float32"}},
        "obsp": {"connectivities": {"shape": [3, 3], "dtype": "float32", "format": "csr", "nnz": 3, "indptr_length": 4, "indices_length": 3}},
        "varp": {"correlation": {"shape": [2, 2], "dtype": "float32", "format": "dense"}},
        "raw": {"n_vars": 2, "shape": [3, 2], "var_index": ["gene-1", "gene-2"]},
        "uns": {"neighbors": {"connectivities_key": "connectivities"}},
    }
    value.update(overrides)
    return value


class AnnDataProjectionTests(unittest.TestCase):
    def test_valid_matrix_and_annotation_projection(self) -> None:
        result = audit_anndata(
            dataset(),
            source_id="cells-demo",
            provenance={"accession": "cells-1", "reader": "anndata"},
        )

        document = result.to_wire()
        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(document["manifest"]["n_obs"], 3)
        self.assertEqual(document["summary"]["layers"], 1)
        self.assertEqual(document["summary"]["obsm"], 1)
        self.assertEqual(document["indices"]["obs"]["unique"], 3)
        self.assertEqual(document["raw"]["shape"], [3, 2])
        self.assertEqual(len(document["document_digest"]), 64)

    def test_dimensions_indices_and_sparse_metadata_are_checked(self) -> None:
        broken = dataset(
            obs_index=["cell-1", "cell-1", "cell-3"],
            layers={"counts": {"shape": [3, 4], "dtype": "int32", "format": "csr", "nnz": 4, "indptr_length": 2, "indices_length": 3}},
        )
        result = audit_anndata(broken, source_id="invalid-cells", provenance={"accession": "invalid"})

        codes = {finding["code"] for finding in result.findings}
        self.assertFalse(result.valid)
        self.assertIn("index_duplicate", codes)
        self.assertIn("shape_mismatch", codes)
        self.assertIn("sparse_indptr_invalid", codes)
        self.assertEqual(result.to_wire()["conformance"]["checks"]["indices"], "fail")

    def test_missing_provenance_is_a_blocking_loss_not_an_inferred_default(self) -> None:
        result = audit_anndata(dataset(), source_id="unlocated-cells")

        document = result.to_wire()
        self.assertTrue(result.valid)
        self.assertFalse(result.publishable)
        self.assertEqual(document["semantic_loss"]["max_severity"], "blocking")
        self.assertIn("provenance_unavailable", {loss["kind"] for loss in document["semantic_loss"]["lost"]})

    def test_bounded_findings_and_adapter_manifest_are_explicit(self) -> None:
        result = audit_anndata(
            dataset(n_obs=4),
            source_id="bounded-cells",
            provenance={"accession": "bounded"},
            max_items=1,
        )
        document = result.to_wire()
        self.assertFalse(result.valid)
        self.assertEqual(len(document["findings"]), 1)
        self.assertGreater(document["omitted_findings"], 0)
        self.assertGreaterEqual(document["summary"]["errors"], 2)

        manifest = AnnDataAdapter().manifest()
        self.assertEqual(manifest["name"], "bioprism.python.anndata_metadata")
        self.assertEqual(manifest["accepted_formats"], ["application/anndata-manifest"])

    def test_input_guards_reject_non_mapping_and_bad_limits(self) -> None:
        with self.assertRaises(ArgumentError):
            audit_anndata([], source_id="bad")  # type: ignore[arg-type]
        with self.assertRaises(ArgumentError):
            audit_anndata(dataset(), source_id="bad", max_items=0)


if __name__ == "__main__":
    unittest.main()
