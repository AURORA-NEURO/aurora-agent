from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest

from prism_sdk import OmeZarrAdapter, audit_ome_zarr
from prism_sdk.errors import ArgumentError


HAS_ZARR = importlib.util.find_spec("zarr") is not None


def projection() -> dict[str, object]:
    return {
        "multiscales": [
            {
                "name": "image",
                "version": "0.5",
                "axes": [
                    {"name": "z", "type": "space", "unit": "micrometer"},
                    {"name": "y", "type": "space", "unit": "micrometer"},
                    {"name": "x", "type": "space", "unit": "micrometer"},
                ],
                "datasets": [
                    {
                        "path": "0",
                        "shape": [16, 32, 32],
                        "chunks": [8, 16, 16],
                        "dtype": "uint16",
                        "coordinate_transformations": [{"type": "scale", "scale": [4.0, 1.0, 1.0]}, {"type": "translation", "translation": [0.0, 0.0, 0.0]}],
                    },
                    {
                        "path": "1",
                        "shape": [8, 16, 16],
                        "chunks": [8, 16, 16],
                        "dtype": "uint16",
                        "coordinate_transformations": [{"type": "scale", "scale": [8.0, 2.0, 2.0]}, {"type": "translation", "translation": [0.0, 0.0, 0.0]}],
                    },
                ],
            }
        ],
        "omero": {"channels": [{"label": "DAPI", "color": "0000FF", "active": True}]},
        "labels": {"nuclei": {"path": "labels/nuclei"}},
    }


class OmeZarrTests(unittest.TestCase):
    def test_valid_multiscale_axes_chunks_and_transforms(self) -> None:
        result = audit_ome_zarr(projection(), source_id="ome", provenance={"accession": "ome-1"})

        document = result.to_wire()
        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(document["manifest"]["dataset_count"], 2)
        self.assertEqual(document["summary"]["channels"], 1)
        self.assertEqual(document["multiscales"][0]["datasets"][0]["shape"], [16, 32, 32])
        self.assertEqual(document["manifest"]["payload_read"], False)

    def test_invalid_chunks_axes_and_missing_scale_are_reported(self) -> None:
        broken = projection()
        broken["multiscales"] = [
            {
                "axes": [{"name": "x"}, {"name": "x"}],
                "datasets": [{"path": "0", "shape": [4, 4], "chunks": [8, 4], "dtype": "uint16"}],
            }
        ]
        result = audit_ome_zarr(broken, source_id="invalid-ome", provenance={"accession": "invalid"})

        codes = {finding["code"] for finding in result.findings}
        self.assertFalse(result.valid)
        self.assertIn("axis_duplicate", codes)
        self.assertIn("chunks_invalid", codes)
        self.assertIn("coordinate_frame_not_carried", {loss["kind"] for loss in result.to_wire()["semantic_loss"]["lost"]})

    def test_missing_provenance_is_blocking_and_input_limits_are_bounded(self) -> None:
        result = audit_ome_zarr(projection(), source_id="unlocated-ome")
        self.assertTrue(result.valid)
        self.assertFalse(result.publishable)
        self.assertEqual(result.to_wire()["semantic_loss"]["max_severity"], "blocking")
        with self.assertRaises(ArgumentError):
            audit_ome_zarr(projection(), source_id="bad", max_items=0)

    def test_adapter_manifest_is_explicit(self) -> None:
        manifest = OmeZarrAdapter().manifest()
        self.assertEqual(manifest["name"], "bioprism.python.ome_zarr_metadata")
        self.assertEqual(manifest["accepted_formats"], ["application/ome-zarr-manifest"])


@unittest.skipUnless(HAS_ZARR, "zarr is not installed in this test environment")
class OmeZarrReaderTests(unittest.TestCase):
    def test_runtime_reads_only_zarr_metadata(self) -> None:
        import zarr

        from prism_sdk import AdapterRuntime, ProjectionRequest, RuntimeStatus

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "image.zarr"
            group = zarr.open_group(path, mode="w")
            group.create_array("0", shape=(4, 8, 8), chunks=(2, 4, 4), dtype="uint16")
            group.create_array("1", shape=(2, 4, 4), chunks=(2, 4, 4), dtype="uint16")
            attrs = projection()["multiscales"]
            group.attrs["multiscales"] = attrs
            result = AdapterRuntime().execute(
                ProjectionRequest("bioprism.python.ome_zarr", "runtime-ome", {"path": str(path)}, {"accession": "runtime"})
            )

        self.assertEqual(result.status, RuntimeStatus.SUCCEEDED)
        self.assertTrue(result.executable)
        self.assertEqual(result.to_wire()["document"]["manifest"]["payload_read"], False)


if __name__ == "__main__":
    unittest.main()
