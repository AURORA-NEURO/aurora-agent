"""Bounded SDF/MOL V2000 molecular-graph auditing without property-value disclosure."""

from __future__ import annotations

from collections import Counter, deque
from dataclasses import dataclass
import math
import re
from typing import Any, Mapping

from .authoring import content_digest
from .errors import ArgumentError


SDF_SCHEMA = "bioprism-python-sdf/0.1"
SDF_ADAPTER = "bioprism.python.sdf_text"
SDF_ADAPTER_VERSION = "0.1.0"
SDF_FORMAT = "chemical/x-mdl-sdfile"
MAX_SDF_BYTES = 100_000_000
MAX_SDF_MOLECULES = 100_000
MAX_SDF_ITEMS = 1_000
MAX_SDF_FIELDS = 10_000
_SEVERITY_ORDER = {"advisory": 0, "degrading": 1, "blocking": 2}
_ELEMENT = re.compile(r"^[A-Za-z][A-Za-z0-9*]?$|^\*$")
_DATA_HEADER = re.compile(r"^>\s*(?:\([^)]*\)\s*)?<([^>]+)>\s*$")
_CHARGE_CODES = {1: 3, 2: 2, 3: 1, 5: -1, 6: -2, 7: -3}


class SdfParseError(ArgumentError):
    """A structurally invalid SDF/MOL source with a stable line locator."""

    def __init__(self, message: str, *, line: int | None = None, molecule: int | None = None) -> None:
        location = ""
        if line is not None:
            location += f" at line {line}"
        if molecule is not None:
            location += f" molecule {molecule}"
        super().__init__(f"SDF parse refused{location}: {message}")


@dataclass(frozen=True)
class SdfFinding:
    """One bounded SDF structural or chemistry finding."""

    code: str
    severity: str
    location: Mapping[str, Any]
    detail: str

    def __post_init__(self) -> None:
        if self.severity not in {"warning", "error"}:
            raise ArgumentError(f"unsupported SDF finding severity: {self.severity!r}")

    def to_wire(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "severity": self.severity,
            "location": dict(self.location),
            "detail": self.detail,
        }


@dataclass(frozen=True)
class SdfParseResult:
    """A validated bounded molecular-graph projection."""

    document: Mapping[str, Any]

    @property
    def molecules(self) -> list[Mapping[str, Any]]:
        return list(self.document["molecules"])

    @property
    def valid(self) -> bool:
        return bool(self.document["valid"])

    @property
    def publishable(self) -> bool:
        return bool(self.document["publishable"])

    def to_wire(self) -> dict[str, Any]:
        return dict(self.document)


@dataclass(frozen=True)
class _Atom:
    x: float
    y: float
    z: float
    element: str
    charge: int


@dataclass(frozen=True)
class _Bond:
    first: int
    second: int
    order: int
    stereo: int


@dataclass(frozen=True)
class _Molecule:
    name: str
    program: str
    comment: str
    atoms: tuple[_Atom, ...]
    bonds: tuple[_Bond, ...]
    field_keys: tuple[str, ...]
    charges: Mapping[int, int]
    isotopes: Mapping[int, int]
    radicals: Mapping[int, int]
    line: int


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[SdfFinding] = []
        self.finding_count = 0
        self.error_count = 0
        self.losses: list[dict[str, Any]] = []
        self.loss_count = 0
        self.blocking_loss_count = 0
        self.max_loss_severity: str | None = None

    def finding(self, code: str, severity: str, location: Mapping[str, Any], detail: str) -> None:
        self.finding_count += 1
        if severity == "error":
            self.error_count += 1
        if len(self.findings) < self.limit:
            self.findings.append(SdfFinding(code, severity, dict(location), detail))

    def loss(self, kind: str, severity: str, location: str, detail: str) -> None:
        self.loss_count += 1
        if severity == "blocking":
            self.blocking_loss_count += 1
        if self.max_loss_severity is None or _SEVERITY_ORDER[severity] > _SEVERITY_ORDER[self.max_loss_severity]:
            self.max_loss_severity = severity
        if len(self.losses) < self.limit:
            self.losses.append(
                {
                    "kind": kind,
                    "severity": severity,
                    "location": location,
                    "detail": detail,
                }
            )

    @property
    def errors(self) -> int:
        return self.error_count

    @property
    def warnings(self) -> int:
        return self.finding_count - self.error_count


def _digest(source_id: str, value: str) -> str:
    return content_digest({"source_id": source_id, "value": value})


def _validate_limit(name: str, value: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
        raise ArgumentError(f"{name} must be between 1 and {maximum}")
    return value


def _decode(payload: str | bytes, *, max_bytes: int) -> str:
    max_bytes = _validate_limit("max_bytes", max_bytes, MAX_SDF_BYTES)
    if isinstance(payload, bytes):
        if len(payload) > max_bytes:
            raise ArgumentError(f"SDF exceeds the {max_bytes}-byte limit")
        try:
            return payload.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ArgumentError(f"SDF is not valid UTF-8: {error}") from error
    if isinstance(payload, str):
        if len(payload.encode("utf-8")) > max_bytes:
            raise ArgumentError(f"SDF exceeds the {max_bytes}-byte limit")
        return payload
    raise ArgumentError("SDF payload must be text or bytes")


def _fixed_int(line: str, start: int, end: int, *, name: str, line_number: int, molecule: int) -> int:
    raw = line[start:end].strip()
    if not raw:
        raise SdfParseError(f"{name} field is empty", line=line_number, molecule=molecule)
    try:
        return int(raw)
    except ValueError as error:
        raise SdfParseError(f"{name} field is not an integer", line=line_number, molecule=molecule) from error


def _fixed_float(line: str, start: int, end: int, *, name: str, line_number: int, molecule: int) -> float:
    raw = line[start:end].strip()
    if not raw:
        raise SdfParseError(f"{name} field is empty", line=line_number, molecule=molecule)
    try:
        value = float(raw)
    except ValueError as error:
        raise SdfParseError(f"{name} field is not numeric", line=line_number, molecule=molecule) from error
    if not math.isfinite(value):
        raise SdfParseError(f"{name} field is not finite", line=line_number, molecule=molecule)
    return value


def _parse_property_line(line: str, *, prefix: str, atom_count: int, line_number: int, molecule: int) -> dict[int, int]:
    parts = line.split()
    if len(parts) < 4 or parts[0] != "M" or parts[1] != prefix:
        raise SdfParseError(f"malformed M  {prefix} property line", line=line_number, molecule=molecule)
    try:
        pair_count = int(parts[2])
    except ValueError as error:
        raise SdfParseError(f"M  {prefix} pair count is not an integer", line=line_number, molecule=molecule) from error
    if pair_count < 0 or len(parts) != 3 + pair_count * 2:
        raise SdfParseError(f"M  {prefix} pair count does not match fields", line=line_number, molecule=molecule)
    values: dict[int, int] = {}
    for offset in range(pair_count):
        try:
            atom_index = int(parts[3 + offset * 2])
            value = int(parts[4 + offset * 2])
        except ValueError as error:
            raise SdfParseError(f"M  {prefix} contains a non-integer pair", line=line_number, molecule=molecule) from error
        if not 1 <= atom_index <= atom_count:
            raise SdfParseError(f"M  {prefix} references an atom outside the counts block", line=line_number, molecule=molecule)
        values[atom_index] = value
    return values


def _parse_molecule(lines: list[str], *, line_start: int, molecule: int) -> _Molecule:
    if len(lines) < 4:
        raise SdfParseError("molecule record must contain at least four header/count lines", line=line_start, molecule=molecule)
    counts = lines[3]
    if "V3000" in counts or any(line.startswith("M  V30") for line in lines):
        raise SdfParseError("V3000 molfile records are not accepted by the bounded V2000 route", line=line_start + 3, molecule=molecule)
    if len(counts) < 6:
        raise SdfParseError("counts line is shorter than atom/bond count columns", line=line_start + 3, molecule=molecule)
    atom_count = _fixed_int(counts, 0, 3, name="atom count", line_number=line_start + 3, molecule=molecule)
    bond_count = _fixed_int(counts, 3, 6, name="bond count", line_number=line_start + 3, molecule=molecule)
    if atom_count < 0 or bond_count < 0:
        raise SdfParseError("atom and bond counts must be non-negative", line=line_start + 3, molecule=molecule)
    atom_end = 4 + atom_count
    bond_end = atom_end + bond_count
    if len(lines) <= bond_end:
        raise SdfParseError("record ends before its declared atom and bond rows", line=line_start, molecule=molecule)
    atoms: list[_Atom] = []
    for offset in range(atom_count):
        line = lines[4 + offset]
        line_number = line_start + 4 + offset
        if len(line) < 34:
            raise SdfParseError("atom row is shorter than coordinate and element columns", line=line_number, molecule=molecule)
        x = _fixed_float(line, 0, 10, name="atom x", line_number=line_number, molecule=molecule)
        y = _fixed_float(line, 10, 20, name="atom y", line_number=line_number, molecule=molecule)
        z = _fixed_float(line, 20, 30, name="atom z", line_number=line_number, molecule=molecule)
        element = line[31:34].strip()
        if not element or not _ELEMENT.fullmatch(element):
            raise SdfParseError("atom element is not a valid bounded symbol", line=line_number, molecule=molecule)
        charge_code = line[36:39].strip()
        if charge_code:
            try:
                charge = _CHARGE_CODES.get(int(charge_code), 0)
            except ValueError as error:
                raise SdfParseError("atom charge code is not an integer", line=line_number, molecule=molecule) from error
        else:
            charge = 0
        atoms.append(_Atom(x, y, z, element.upper(), charge))
    bonds: list[_Bond] = []
    for offset in range(bond_count):
        line = lines[atom_end + offset]
        line_number = line_start + atom_end + offset
        if len(line) < 9:
            raise SdfParseError("bond row is shorter than atom-index and order columns", line=line_number, molecule=molecule)
        first = _fixed_int(line, 0, 3, name="bond first atom", line_number=line_number, molecule=molecule)
        second = _fixed_int(line, 3, 6, name="bond second atom", line_number=line_number, molecule=molecule)
        order = _fixed_int(line, 6, 9, name="bond order", line_number=line_number, molecule=molecule)
        try:
            stereo = int(line[9:12].strip() or 0)
        except ValueError as error:
            raise SdfParseError("bond stereo field is not an integer", line=line_number, molecule=molecule) from error
        if not 1 <= first <= atom_count or not 1 <= second <= atom_count or first == second:
            raise SdfParseError("bond atom index is outside the molecule or self-referential", line=line_number, molecule=molecule)
        if order not in {1, 2, 3, 4}:
            raise SdfParseError("bond order must be one of 1, 2, 3, or aromatic 4", line=line_number, molecule=molecule)
        bonds.append(_Bond(first, second, order, stereo))
    property_end = None
    charges: dict[int, int] = {}
    isotopes: dict[int, int] = {}
    radicals: dict[int, int] = {}
    field_keys: list[str] = []
    data_mode = False
    index = bond_end
    while index < len(lines):
        line = lines[index]
        line_number = line_start + index
        if line == "M  END":
            property_end = index
            index += 1
            break
        if line.startswith("M  CHG"):
            charges.update(_parse_property_line(line, prefix="CHG", atom_count=atom_count, line_number=line_number, molecule=molecule))
        elif line.startswith("M  ISO"):
            isotopes.update(_parse_property_line(line, prefix="ISO", atom_count=atom_count, line_number=line_number, molecule=molecule))
        elif line.startswith("M  RAD"):
            radicals.update(_parse_property_line(line, prefix="RAD", atom_count=atom_count, line_number=line_number, molecule=molecule))
        elif line.startswith("M  "):
            # Other property records remain structurally bounded but are not interpreted.
            pass
        index += 1
    if property_end is None:
        raise SdfParseError("record is missing M  END", line=line_start + bond_end, molecule=molecule)
    current_key: str | None = None
    while index < len(lines):
        line = lines[index]
        line_number = line_start + index
        if not line:
            current_key = None
            index += 1
            continue
        header = _DATA_HEADER.fullmatch(line)
        if header is not None:
            current_key = header.group(1)
            field_keys.append(current_key)
        elif current_key is None:
            raise SdfParseError("unlabeled data line follows M  END", line=line_number, molecule=molecule)
        index += 1
    return _Molecule(lines[0], lines[1], lines[2], tuple(atoms), tuple(bonds), tuple(field_keys), charges, isotopes, radicals, line_start)


def _components(atom_count: int, bonds: tuple[_Bond, ...]) -> int:
    if atom_count == 0:
        return 0
    graph: list[list[int]] = [[] for _ in range(atom_count)]
    for bond in bonds:
        graph[bond.first - 1].append(bond.second - 1)
        graph[bond.second - 1].append(bond.first - 1)
    seen: set[int] = set()
    components = 0
    for start in range(atom_count):
        if start in seen:
            continue
        components += 1
        queue = deque([start])
        seen.add(start)
        while queue:
            current = queue.popleft()
            for neighbor in graph[current]:
                if neighbor not in seen:
                    seen.add(neighbor)
                    queue.append(neighbor)
    return components


def parse_sdf(
    payload: str | bytes,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_bytes: int = MAX_SDF_BYTES,
    max_molecules: int = MAX_SDF_MOLECULES,
    max_items: int = MAX_SDF_ITEMS,
) -> SdfParseResult:
    """Parse bounded SDF/MOL V2000 records and audit molecular graph integrity."""

    if not isinstance(source_id, str) or not source_id.strip():
        raise ArgumentError("source_id must be a non-empty string")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a mapping when supplied")
    max_molecules = _validate_limit("max_molecules", max_molecules, MAX_SDF_MOLECULES)
    max_items = _validate_limit("max_items", max_items, MAX_SDF_ITEMS)
    text = _decode(payload, max_bytes=max_bytes)
    if not text:
        raise SdfParseError("source is empty")
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    audit = _Audit(max_items)
    provenance_digest: str | None = None
    if provenance:
        try:
            provenance_digest = content_digest(dict(provenance))
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"provenance is not canonical JSON-safe: {error}") from error
    audit.loss("content_uninterpreted", "degrading", source_id, "molecule names, property values, atom labels, and raw molfile records are not emitted; bounded graph summaries and source-bound digests are carried")
    audit.loss("coordinate_frame_not_carried", "degrading", "coordinates", "coordinates are summarized in a molecule-local frame; conformer identity and experimental context are not inferred")
    audit.loss("ontology_term_unmapped", "degrading", "chemistry", "element and bond labels are structurally checked without external chemistry or ontology resolution")
    if provenance_digest is None:
        audit.loss("provenance_unavailable", "blocking", "provenance", "no non-empty provenance projection was supplied")

    records: list[list[str]] = []
    current: list[str] = []
    start_line = 1
    for line_number, raw_line in enumerate(lines, start=1):
        line = raw_line[:-1] if raw_line.endswith("\r") else raw_line
        if "\r" in line:
            raise SdfParseError("lone carriage return is not a record separator", line=line_number)
        if line == "$$$$":
            if not current:
                raise SdfParseError("empty molecule record before delimiter", line=line_number)
            records.append(current)
            if len(records) > max_molecules:
                raise ArgumentError(f"SDF contains more than the {max_molecules}-molecule limit")
            current = []
            start_line = line_number + 1
        else:
            current.append(line)
    if current:
        raise SdfParseError("final molecule is missing the $$$$ delimiter", line=start_line)
    if not records:
        raise SdfParseError("source contains no molecule records")

    molecules: list[_Molecule] = []
    for number, record in enumerate(records, start=1):
        molecules.append(_parse_molecule(record, line_start=sum(len(item) + 1 for item in records[: number - 1]) + 1, molecule=number))
    name_digests: list[str] = []
    molecule_rows: list[dict[str, Any]] = []
    element_counts: Counter[str] = Counter()
    bond_order_counts: Counter[str] = Counter()
    total_atoms = 0
    total_bonds = 0
    total_formal_charge = 0
    disconnected = 0
    duplicate_fields = 0
    for number, molecule in enumerate(molecules, start=1):
        location = {"source": source_id, "molecule": number}
        if not molecule.atoms:
            audit.finding("molecule_empty", "error", location, "molecule contains zero atom rows")
        if len(molecule.field_keys) != len(set(molecule.field_keys)):
            duplicate_fields += len(molecule.field_keys) - len(set(molecule.field_keys))
            audit.finding("data_field_duplicate", "error", location, "SDF data field key occurs more than once")
        if len(molecule.field_keys) > MAX_SDF_FIELDS:
            raise ArgumentError(f"SDF data fields exceed the {MAX_SDF_FIELDS}-field molecule limit")
        atoms = molecule.atoms
        bonds = molecule.bonds
        total_atoms += len(atoms)
        total_bonds += len(bonds)
        molecule_formal_charge = sum(molecule.charges.get(index, atom.charge) for index, atom in enumerate(atoms, start=1))
        total_formal_charge += molecule_formal_charge
        components = _components(len(atoms), bonds)
        if components > 1:
            disconnected += 1
        elements = Counter(atom.element for atom in atoms)
        element_counts.update(elements)
        bond_orders = Counter(str(bond.order) for bond in bonds)
        bond_order_counts.update(bond_orders)
        coords = [(atom.x, atom.y, atom.z) for atom in atoms]
        bbox_min = [min(values) for values in zip(*coords)] if coords else None
        bbox_max = [max(values) for values in zip(*coords)] if coords else None
        centroid = [round(sum(values) / len(values), 6) for values in zip(*coords)] if coords else None
        name_digest = _digest(source_id, molecule.name)
        name_digests.append(name_digest)
        graph_digest = content_digest(
            {
                "atoms": [{"element": atom.element, "charge": atom.charge} for atom in atoms],
                "bonds": [{"first": bond.first, "second": bond.second, "order": bond.order, "stereo": bond.stereo} for bond in bonds],
            }
        )
        molecule_rows.append(
            {
                "molecule": number,
                "molecule_name_digest": name_digest,
                "graph_digest": _digest(source_id, graph_digest),
                "atom_count": len(atoms),
                "bond_count": len(bonds),
                "element_counts": dict(sorted(elements.items())),
                "bond_order_counts": dict(sorted(bond_orders.items())),
                "formal_charge": molecule_formal_charge,
                "isotope_annotation_count": len(molecule.isotopes),
                "radical_annotation_count": len(molecule.radicals),
                "data_field_count": len(molecule.field_keys),
                "data_field_key_digests": sorted(_digest(source_id, key) for key in molecule.field_keys[:max_items]),
                "connected_components": components,
                "coordinate_min": bbox_min,
                "coordinate_max": bbox_max,
                "centroid": centroid,
            }
        )

    valid = audit.errors == 0
    publishable = valid and audit.max_loss_severity != "blocking"
    source_digest = content_digest({"source_id": source_id, "payload": text})
    manifest = {
        "source_id": source_id,
        "source_digest": source_digest,
        "adapter": SDF_ADAPTER,
        "adapter_version": SDF_ADAPTER_VERSION,
        "declared_format": SDF_FORMAT,
        "molecule_count": len(molecules),
        "provenance_digest": provenance_digest,
        "bytes_read": True,
        "molecule_names_disclosed": False,
        "property_values_disclosed": False,
        "raw_records_disclosed": False,
    }
    document: dict[str, Any] = {
        "schema": SDF_SCHEMA,
        "workflow": "sdf_molecular_graph_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": manifest,
        "summary": {
            "molecules": len(molecules),
            "atoms": total_atoms,
            "bonds": total_bonds,
            "element_counts": dict(sorted(element_counts.items())),
            "bond_order_counts": dict(sorted(bond_order_counts.items())),
            "total_formal_charge": total_formal_charge,
            "disconnected_molecules": disconnected,
            "duplicate_data_fields": duplicate_fields,
            "errors": audit.errors,
            "warnings": audit.warnings,
            "finding_count": audit.finding_count,
            "blocking_loss_count": audit.blocking_loss_count,
        },
        "molecules": molecule_rows[:max_items],
        "omitted_molecules": max(0, len(molecule_rows) - max_items),
        "findings": [finding.to_wire() for finding in audit.findings],
        "omitted_findings": max(0, audit.finding_count - len(audit.findings)),
        "semantic_loss": {
            "audit": "lossy" if audit.loss_count else "lossless",
            "lost_count": audit.loss_count,
            "max_severity": audit.max_loss_severity,
            "lost": list(audit.losses),
            "omitted_lost": max(0, audit.loss_count - len(audit.losses)),
        },
        "conformance": {
            "level": "normalize",
            "passed": valid,
            "publishable": publishable,
            "checks": {
                "record_structure": "pass" if molecules else "fail",
                "atom_bond_integrity": "pass",
                "data_fields": "pass" if duplicate_fields == 0 else "fail",
                "graph_components": "pass",
                "provenance": "pass" if provenance_digest is not None else "fail",
            },
            "limitations": [
                "molecule names, atom labels, property values, and raw molfile records are represented by source-bound digests or aggregate counts",
                "the bounded route handles V2000 records; V3000 records are explicitly refused rather than guessed",
                "the audit validates molecular graph structure and coordinates, not stereochemical correctness, valence, tautomerism, or chemical identity",
            ],
        },
        "max_molecules": MAX_SDF_MOLECULES,
        "max_items": max_items,
    }
    document["document_digest"] = content_digest(document)
    return SdfParseResult(document)


class SdfAdapter:
    """Concrete adapter facade matching the dependency-free bounded SDF route."""

    name = SDF_ADAPTER
    version = SDF_ADAPTER_VERSION
    accepted_formats = ("chemical/x-mdl-sdfile", "chemical/x-mdl-molfile", "text/sdf")
    declared_loss_kinds = frozenset(
        {
            "content_uninterpreted",
            "coordinate_frame_not_carried",
            "ontology_term_unmapped",
            "provenance_unavailable",
        }
    )

    def manifest(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "version": self.version,
            "accepted_formats": list(self.accepted_formats),
            "conformance_level": "normalize",
            "declared_loss_kinds": sorted(self.declared_loss_kinds),
            "scope_dimensions": ["subject", "sample", "molecule", "atom", "bond", "assay"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def parse(
        self,
        sdf: str | bytes,
        *,
        source_id: str,
        provenance: Mapping[str, Any] | None = None,
        max_bytes: int = MAX_SDF_BYTES,
        max_molecules: int = MAX_SDF_MOLECULES,
        max_items: int = MAX_SDF_ITEMS,
    ) -> SdfParseResult:
        return parse_sdf(
            sdf,
            source_id=source_id,
            provenance=provenance,
            max_bytes=max_bytes,
            max_molecules=max_molecules,
            max_items=max_items,
        )


__all__ = [
    "MAX_SDF_BYTES",
    "MAX_SDF_FIELDS",
    "MAX_SDF_ITEMS",
    "MAX_SDF_MOLECULES",
    "SDF_ADAPTER",
    "SDF_ADAPTER_VERSION",
    "SDF_FORMAT",
    "SDF_SCHEMA",
    "SdfAdapter",
    "SdfFinding",
    "SdfParseError",
    "SdfParseResult",
    "parse_sdf",
]
