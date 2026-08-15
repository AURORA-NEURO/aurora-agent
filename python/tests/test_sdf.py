from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from prism_sdk import (
    AdapterRuntime,
    ProjectionRequest,
    RuntimeStatus,
    SdfAdapter,
    parse_sdf,
    read_sdf,
)
from prism_sdk.errors import ArgumentError


def atom_line(x: float, y: float, z: float, element: str) -> str:
    return f"{x:10.4f}{y:10.4f}{z:10.4f} {element:<3}  0  0  0  0  0  0  0  0  0  0  0  0"


def bond_line(first: int, second: int, order: int = 1, stereo: int = 0) -> str:
    return f"{first:3d}{second:3d}{order:3d}{stereo:3d}  0  0  0"


def molecule(
    name: str,
    *,
    fields: tuple[tuple[str, str], ...] = (),
    charge: int | None = None,
    disconnected: bool = False,
) -> str:
    atom_rows = [
        atom_line(0.0, 0.0, 0.0, "O"),
        atom_line(0.9584, 0.0, 0.0, "H"),
        atom_line(-0.2392, 0.9271, 0.0, "H"),
    ]
    bonds = [bond_line(1, 2), bond_line(1, 3)] if not disconnected else [bond_line(1, 2)]
    lines = [
        name,
        "Prism SDF test",
        "private comment",
        f"{len(atom_rows):3d}{len(bonds):3d}  0  0  0  0            999 V2000",
        *atom_rows,
        *bonds,
    ]
    if charge is not None:
        lines.append(f"M  CHG  1   1  {charge:2d}")
    lines.append("M  END")
    for key, value in fields:
        lines.extend([f">  <{key}>", value, ""])
    lines.extend(["$$$$"])
    return "\n".join(lines)


SDF = molecule(
    "water-1",
    fields=(("ID", "sensitive-value"), ("FORMULA", "H2O")),
    charge=-1,
)


class SdfProjectionTests(unittest.TestCase):
    def test_valid_v2000_graph_is_summarized_without_raw_disclosure(self) -> None:
        result = parse_sdf(SDF, source_id="molecule", provenance={"accession": "chem-1"})

        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(result.document["manifest"]["adapter"], "bioprism.python.sdf_text")
        self.assertEqual(result.document["summary"]["molecules"], 1)
        self.assertEqual(result.document["summary"]["atoms"], 3)
        self.assertEqual(result.document["summary"]["bonds"], 2)
        self.assertEqual(result.document["summary"]["element_counts"], {"H": 2, "O": 1})
        self.assertEqual(result.document["summary"]["total_formal_charge"], -1)
        self.assertEqual(result.document["molecules"][0]["connected_components"], 1)
        self.assertFalse(result.document["manifest"]["property_values_disclosed"])
        self.assertNotIn("water-1", str(result.to_wire()))
        self.assertNotIn("sensitive-value", str(result.to_wire()))
        self.assertEqual(SdfAdapter().manifest()["name"], "bioprism.python.sdf_text")

    def test_duplicate_fields_and_disconnected_graph_are_evidence_bearing(self) -> None:
        duplicate = molecule("duplicate", fields=(("ID", "one"), ("ID", "two")), disconnected=True)
        duplicate_pair = duplicate + "\n" + molecule("duplicate-two", fields=(("ID", "three"), ("ID", "four")))
        result = parse_sdf(duplicate_pair, source_id="invalid-molecule", provenance={"version": "1"}, max_items=1)

        self.assertFalse(result.valid)
        self.assertEqual(result.document["summary"]["disconnected_molecules"], 1)
        self.assertEqual(result.document["summary"]["duplicate_data_fields"], 2)
        self.assertGreaterEqual(result.document["summary"]["errors"], 1)
        self.assertGreater(result.document["omitted_findings"], 0)
        expanded = parse_sdf(duplicate_pair, source_id="invalid-molecule", provenance={"version": "1"}, max_items=4)
        self.assertTrue(any(finding["code"] == "data_field_duplicate" for finding in expanded.document["findings"]))

    def test_malformed_records_v3000_and_bounds_are_refused(self) -> None:
        with self.assertRaisesRegex(ArgumentError, r"missing the \$\$\$\$"):
            parse_sdf(SDF.rstrip("$"), source_id="truncated")
        with self.assertRaisesRegex(ArgumentError, "V3000"):
            parse_sdf(SDF.replace("999 V2000", "999 V3000"), source_id="v3000")
        with self.assertRaises(ArgumentError):
            parse_sdf(SDF + "\n" + molecule("second"), source_id="molecule-bound", max_molecules=1)
        with self.assertRaises(ArgumentError):
            parse_sdf(SDF, source_id="byte-bound", max_bytes=len(SDF.encode("utf-8")) - 1)

    def test_raw_reader_and_runtime_share_the_same_molecular_audit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "molecule.sdf"
            path.write_text(SDF, encoding="utf-8")
            document = read_sdf(str(path), source_id="raw-molecule", provenance={"accession": "raw-1"})
            runtime = AdapterRuntime().execute(
                ProjectionRequest(
                    "bioprism.python.sdf_text",
                    "runtime-molecule",
                    {"path": str(path)},
                    provenance={"accession": "runtime-1"},
                )
            )

        self.assertTrue(document["valid"])
        self.assertEqual(document["manifest"]["raw_records_disclosed"], False)
        self.assertEqual(runtime.status, RuntimeStatus.LOSSY)
        self.assertTrue(runtime.executable)
        self.assertEqual(runtime.document["summary"]["atoms"], 3)


if __name__ == "__main__":
    unittest.main()
