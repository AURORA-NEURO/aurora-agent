from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from prism_sdk import (
    AdapterRuntime,
    PdbAdapter,
    ProjectionRequest,
    RuntimeStatus,
    parse_pdb,
    read_pdb,
)
from prism_sdk.errors import ArgumentError


def atom_line(
    record: str,
    serial: int,
    atom_name: str,
    residue: str,
    chain: str,
    residue_number: int,
    x: float,
    y: float,
    z: float,
    element: str,
) -> str:
    return f"{record:<6}{serial:5d} {atom_name:<4}{' ':1}{residue:>3} {chain}{residue_number:4d}{' ':1}   {x:8.3f}{y:8.3f}{z:8.3f}{1.00:6.2f}{20.00:6.2f}          {element:>2}{' ':>2}"


PDB = "\n".join(
    [
        "HEADER    BOUNDED TEST STRUCTURE",
        "TITLE     STRUCTURAL METADATA",
        "REMARK   2 RESOLUTION.    2.00 ANGSTROMS.",
        "CRYST1   20.000   20.000   20.000  90.00  90.00  90.00 P 1           1",
        "MODEL        1",
        atom_line("ATOM", 1, "N", "GLY", "A", 1, 1.0, 2.0, 3.0, "N"),
        atom_line("ATOM", 2, "CA", "GLY", "A", 1, 2.0, 3.0, 4.0, "C"),
        atom_line("HETATM", 3, "O", "HOH", "A", 2, 3.0, 4.0, 5.0, "O"),
        "CONECT    1    2",
        "ENDMDL",
        "END",
        "",
    ]
)


class PdbProjectionTests(unittest.TestCase):
    def test_valid_fixed_column_structure_is_summarized_without_raw_disclosure(self) -> None:
        result = parse_pdb(PDB, source_id="structure", provenance={"accession": "pdb-1"})

        self.assertTrue(result.valid)
        self.assertTrue(result.publishable)
        self.assertEqual(result.document["manifest"]["adapter"], "bioprism.python.pdb_text")
        self.assertEqual(result.document["summary"]["atoms"], 3)
        self.assertEqual(result.document["summary"]["models"], 1)
        self.assertEqual(result.document["summary"]["chains"], 1)
        self.assertEqual(result.document["summary"]["residues"], 2)
        self.assertEqual(result.document["summary"]["unresolved_conect_edges"], 0)
        self.assertEqual(result.document["summary"]["resolution"], 2.0)
        self.assertEqual(result.document["summary"]["element_counts"], {"C": 1, "N": 1, "O": 1})
        self.assertNotIn("GLY", str(result.to_wire()))
        self.assertNotIn("STRUCTURAL METADATA", str(result.to_wire()))
        self.assertEqual(PdbAdapter().manifest()["name"], "bioprism.python.pdb_text")

    def test_duplicate_serials_and_unresolved_connectivity_are_evidence_bearing(self) -> None:
        duplicate = PDB.replace(atom_line("ATOM", 2, "CA", "GLY", "A", 1, 2.0, 3.0, 4.0, "C"), atom_line("ATOM", 1, "CA", "GLY", "A", 1, 2.0, 3.0, 4.0, "C"))
        unresolved = duplicate.replace("CONECT    1    2", "CONECT    1   99")
        result = parse_pdb(unresolved, source_id="invalid-structure", provenance={"version": "1"}, max_items=1)

        self.assertFalse(result.valid)
        self.assertGreaterEqual(result.document["summary"]["errors"], 2)
        self.assertGreater(result.document["omitted_findings"], 0)
        expanded = parse_pdb(unresolved, source_id="invalid-structure", provenance={"version": "1"}, max_items=2)
        self.assertTrue(any(finding["code"] == "atom_serial_duplicate" for finding in expanded.document["findings"]))
        self.assertTrue(any(finding["code"] == "conect_unresolved" for finding in expanded.document["findings"]))

    def test_malformed_fixed_columns_and_bounds_are_refused(self) -> None:
        with self.assertRaisesRegex(ArgumentError, "coordinate"):
            parse_pdb("ATOM      1  N   GLY A   1      1.0\n", source_id="short-atom")
        with self.assertRaises(ArgumentError):
            parse_pdb(PDB, source_id="atom-bound", max_atoms=2)
        with self.assertRaises(ArgumentError):
            parse_pdb(PDB, source_id="byte-bound", max_bytes=len(PDB.encode("ascii")) - 1)

    def test_raw_reader_and_runtime_share_the_same_structure_audit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "structure.pdb"
            path.write_text(PDB, encoding="ascii")
            document = read_pdb(str(path), source_id="raw-structure", provenance={"accession": "raw-1"})
            runtime = AdapterRuntime().execute(
                ProjectionRequest(
                    "bioprism.python.pdb_text",
                    "runtime-structure",
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
