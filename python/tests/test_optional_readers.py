from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest

from prism_sdk import (
    AdapterRuntime,
    ProjectionRequest,
    RuntimeStatus,
    read_anndata_projection,
    read_nifti_header,
)


HAS_NIBABEL = importlib.util.find_spec("nibabel") is not None
HAS_ANNDATA = importlib.util.find_spec("anndata") is not None


@unittest.skipUnless(HAS_NIBABEL, "nibabel is not installed in this test environment")
class NiftiReaderTests(unittest.TestCase):
    def test_header_reader_uses_nibabel_without_loading_image_values(self) -> None:
        import nibabel as nib
        import numpy as np

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bold.nii.gz"
            image = nib.Nifti1Image(np.zeros((4, 4, 4), dtype=np.float32), np.diag([2.0, 2.0, 2.0, 1.0]))
            image.set_qform(image.affine, code=1)
            image.set_sform(image.affine, code=4)
            nib.save(image, str(path))

            document = read_nifti_header(
                str(path),
                source_id="raw-nifti",
                reference_space="MNI152NLin6Asym",
                provenance={"accession": "nifti-1"},
            )

        self.assertTrue(document["valid"])
        self.assertEqual(document["manifest"]["adapter"], "bioprism.python.nifti_metadata")
        self.assertEqual(document["images"][0]["shape"], [4, 4, 4])
        self.assertEqual(document["manifest"]["bytes_read"], False)

    def test_runtime_executes_raw_nifti_route_when_dependency_is_available(self) -> None:
        import nibabel as nib
        import numpy as np

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "raw.nii"
            image = nib.Nifti1Image(np.zeros((2, 2, 2), dtype=np.float32), np.eye(4))
            image.set_qform(image.affine, code=1)
            image.set_sform(image.affine, code=1)
            nib.save(image, str(path))
            result = AdapterRuntime().execute(
                ProjectionRequest(
                    "bioprism.python.nifti_bids",
                    "runtime-nifti",
                    {"path": str(path), "reference_space": "scanner"},
                    provenance={"accession": "runtime"},
                )
            )

        self.assertEqual(result.status, RuntimeStatus.SUCCEEDED)
        self.assertTrue(result.executable)


@unittest.skipUnless(HAS_ANNDATA, "anndata is not installed in this test environment")
class AnnDataReaderTests(unittest.TestCase):
    def test_h5ad_reader_projects_metadata_without_decoding_matrix_values(self) -> None:
        import anndata as ad
        import numpy as np
        import pandas as pd

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "cells.h5ad"
            value = ad.AnnData(
                X=np.zeros((2, 2), dtype=np.float32),
                obs=pd.DataFrame({"cell_type": pd.Categorical(["T", "B"])}, index=["cell-1", "cell-2"]),
                var=pd.DataFrame({"symbol": ["G1", "G2"]}, index=["gene-1", "gene-2"]),
            )
            value.write_h5ad(path)
            document = read_anndata_projection(
                str(path),
                source_id="raw-anndata",
                provenance={"accession": "cells-1"},
            )

        self.assertTrue(document["valid"])
        self.assertEqual(document["manifest"]["n_obs"], 2)
        self.assertEqual(document["indices"]["obs"]["unique"], 2)
        self.assertEqual(document["manifest"]["bytes_read"], False)

    def test_zarr_reader_inspects_store_metadata_without_anndata_full_load(self) -> None:
        import anndata as ad
        import numpy as np
        import pandas as pd

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "cells.zarr"
            value = ad.AnnData(
                X=np.zeros((2, 2), dtype=np.float32),
                obs=pd.DataFrame({"cell_type": ["T", "B"]}, index=["cell-1", "cell-2"]),
                var=pd.DataFrame(index=["gene-1", "gene-2"]),
            )
            value.write_zarr(path)
            document = read_anndata_projection(
                str(path),
                source_id="raw-zarr",
                storage_format="zarr",
                provenance={"accession": "zarr-1"},
            )

        self.assertTrue(document["valid"])
        self.assertEqual(document["manifest"]["n_obs"], 2)
        self.assertEqual(document["X"]["shape"], [2, 2])

    def test_runtime_executes_raw_h5ad_route_when_dependency_is_available(self) -> None:
        import anndata as ad
        import numpy as np
        import pandas as pd

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "runtime.h5ad"
            ad.AnnData(
                X=np.zeros((1, 1), dtype=np.float32),
                obs=pd.DataFrame(index=["cell-1"]),
                var=pd.DataFrame(index=["gene-1"]),
            ).write_h5ad(path)
            result = AdapterRuntime().execute(
                ProjectionRequest(
                    "bioprism.python.anndata",
                    "runtime-anndata",
                    {"path": str(path), "storage_format": "h5ad"},
                    provenance={"accession": "runtime"},
                )
            )

        self.assertEqual(result.status, RuntimeStatus.SUCCEEDED)
        self.assertTrue(result.executable)


if __name__ == "__main__":
    unittest.main()
