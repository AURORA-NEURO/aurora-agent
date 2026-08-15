"""Bounded PDB fixed-column parsing with privacy-preserving structure evidence."""

from __future__ import annotations

from collections import Counter, defaultdict
from dataclasses import dataclass
import math
import re
from typing import Any, Mapping

from .authoring import content_digest
from .errors import ArgumentError


PDB_SCHEMA = "bioprism-python-pdb/0.1"
PDB_ADAPTER = "bioprism.python.pdb_text"
PDB_ADAPTER_VERSION = "0.1.0"
PDB_FORMAT = "chemical/x-pdb"
MAX_PDB_BYTES = 100_000_000
MAX_PDB_ATOMS = 1_000_000
MAX_PDB_ITEMS = 1_000
MAX_PDB_LINE_BYTES = 10_000
_SEVERITY_ORDER = {"advisory": 0, "degrading": 1, "blocking": 2}
_RESOLUTION = re.compile(r"RESOLUTION\.\s+([0-9]+(?:\.[0-9]+)?)", re.IGNORECASE)


class PdbParseError(ArgumentError):
    """A structurally invalid PDB source with a stable line locator."""

    def __init__(self, message: str, *, line: int | None = None, atom: int | None = None) -> None:
        location = ""
        if line is not None:
            location += f" at line {line}"
        if atom is not None:
            location += f" atom {atom}"
        super().__init__(f"PDB parse refused{location}: {message}")


@dataclass(frozen=True)
class PdbFinding:
    """One bounded PDB structural or metadata finding."""

    code: str
    severity: str
    location: Mapping[str, Any]
    detail: str

    def __post_init__(self) -> None:
        if self.severity not in {"warning", "error"}:
            raise ArgumentError(f"unsupported PDB finding severity: {self.severity!r}")

    def to_wire(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "severity": self.severity,
            "location": dict(self.location),
            "detail": self.detail,
        }


@dataclass(frozen=True)
class PdbParseResult:
    """A validated bounded PDB structural projection."""

    document: Mapping[str, Any]

    @property
    def atoms(self) -> list[Mapping[str, Any]]:
        return list(self.document["atoms"])

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
    record_type: str
    serial: int
    atom_name: str
    alt_loc: str
    residue_name: str
    chain_id: str
    residue_number: int
    insertion_code: str
    x: float
    y: float
    z: float
    occupancy: float | None
    temp_factor: float | None
    element: str
    charge: str
    model: int
    line: int


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[PdbFinding] = []
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
            self.findings.append(PdbFinding(code, severity, dict(location), detail))

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
    max_bytes = _validate_limit("max_bytes", max_bytes, MAX_PDB_BYTES)
    if isinstance(payload, bytes):
        if len(payload) > max_bytes:
            raise ArgumentError(f"PDB exceeds the {max_bytes}-byte limit")
        try:
            return payload.decode("ascii")
        except UnicodeDecodeError as error:
            raise ArgumentError(f"PDB is not valid ASCII: {error}") from error
    if isinstance(payload, str):
        try:
            raw = payload.encode("ascii")
        except UnicodeEncodeError as error:
            raise ArgumentError(f"PDB is not valid ASCII: {error}") from error
        if len(raw) > max_bytes:
            raise ArgumentError(f"PDB exceeds the {max_bytes}-byte limit")
        return payload
    raise ArgumentError("PDB payload must be text or bytes")


def _fixed_int(line: str, start: int, end: int, *, name: str, line_number: int, atom: int | None = None) -> int:
    value = line[start:end].strip()
    if not value:
        raise PdbParseError(f"{name} field is empty", line=line_number, atom=atom)
    try:
        return int(value)
    except ValueError as error:
        raise PdbParseError(f"{name} field is not an integer", line=line_number, atom=atom) from error


def _fixed_float(line: str, start: int, end: int, *, name: str, line_number: int, atom: int | None = None) -> float:
    value = line[start:end].strip()
    if not value:
        raise PdbParseError(f"{name} field is empty", line=line_number, atom=atom)
    try:
        parsed = float(value)
    except ValueError as error:
        raise PdbParseError(f"{name} field is not numeric", line=line_number, atom=atom) from error
    if not math.isfinite(parsed):
        raise PdbParseError(f"{name} field is not finite", line=line_number, atom=atom)
    return parsed


def _optional_float(line: str, start: int, end: int, *, name: str, line_number: int, atom: int) -> float | None:
    value = line[start:end].strip()
    if not value:
        return None
    return _fixed_float(line, start, end, name=name, line_number=line_number, atom=atom)


def _parse_atom(line: str, *, line_number: int, model: int) -> _Atom:
    if len(line) < 54:
        raise PdbParseError("ATOM/HETATM row is shorter than the coordinate columns", line=line_number)
    serial = _fixed_int(line, 6, 11, name="serial", line_number=line_number)
    if serial < 1:
        raise PdbParseError("serial must be positive", line=line_number, atom=serial)
    atom_name = line[12:16].strip()
    residue_name = line[17:20].strip()
    chain_id = line[21:22].strip() or "."
    residue_number = _fixed_int(line, 22, 26, name="residue number", line_number=line_number, atom=serial)
    alt_loc = line[16:17].strip() or "."
    insertion_code = line[26:27].strip() or "."
    if not atom_name or not residue_name:
        raise PdbParseError("atom and residue names must be non-empty", line=line_number, atom=serial)
    x = _fixed_float(line, 30, 38, name="x", line_number=line_number, atom=serial)
    y = _fixed_float(line, 38, 46, name="y", line_number=line_number, atom=serial)
    z = _fixed_float(line, 46, 54, name="z", line_number=line_number, atom=serial)
    occupancy = _optional_float(line, 54, 60, name="occupancy", line_number=line_number, atom=serial)
    temp_factor = _optional_float(line, 60, 66, name="temperature factor", line_number=line_number, atom=serial)
    element = line[76:78].strip().upper() if len(line) >= 78 else ""
    charge = line[78:80].strip() if len(line) >= 80 else ""
    return _Atom(
        line[0:6].strip().upper(),
        serial,
        atom_name,
        alt_loc,
        residue_name,
        chain_id,
        residue_number,
        insertion_code,
        x,
        y,
        z,
        occupancy,
        temp_factor,
        element,
        charge,
        model,
        line_number,
    )


def parse_pdb(
    payload: str | bytes,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_bytes: int = MAX_PDB_BYTES,
    max_atoms: int = MAX_PDB_ATOMS,
    max_items: int = MAX_PDB_ITEMS,
) -> PdbParseResult:
    """Parse bounded PDB fixed-column records without disclosing raw structure content."""

    if not isinstance(source_id, str) or not source_id.strip():
        raise ArgumentError("source_id must be a non-empty string")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a mapping when supplied")
    max_atoms = _validate_limit("max_atoms", max_atoms, MAX_PDB_ATOMS)
    max_items = _validate_limit("max_items", max_items, MAX_PDB_ITEMS)
    text = _decode(payload, max_bytes=max_bytes)
    if not text:
        raise PdbParseError("source is empty")
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
    audit.loss("content_uninterpreted", "degrading", source_id, "raw atom names, residue names, chain labels, and fixed-column records are not emitted; bounded structural summaries and source-bound digests are carried")
    audit.loss("coordinate_frame_not_carried", "degrading", "coordinates", "coordinates are summarized in source-local geometry; biological frame, biological assembly, and reference context are not inferred")
    audit.loss("ontology_term_unmapped", "degrading", "residues", "residue and element labels are summarized without external chemistry or ontology resolution")
    if provenance_digest is None:
        audit.loss("provenance_unavailable", "blocking", "provenance", "no non-empty provenance projection was supplied")

    atoms: list[_Atom] = []
    models: set[int] = set()
    model_atom_serials: defaultdict[int, set[int]] = defaultdict(set)
    conect_edges: list[tuple[int, int, int, int]] = []
    seqres: defaultdict[str, list[str]] = defaultdict(list)
    metadata_counts: Counter[str] = Counter()
    cell: dict[str, float] | None = None
    resolution: float | None = None
    current_model = 1
    explicit_model = False
    model_active = False
    ended = False
    line_count = 0
    for line_number, raw_line in enumerate(lines, start=1):
        line_count = line_number
        line = raw_line[:-1] if raw_line.endswith("\r") else raw_line
        if "\r" in line:
            raise PdbParseError("lone carriage return is not a record separator", line=line_number)
        if len(line.encode("ascii")) > MAX_PDB_LINE_BYTES:
            raise ArgumentError(f"PDB line exceeds the {MAX_PDB_LINE_BYTES}-byte limit")
        if not line.strip():
            continue
        record = line[0:6].strip().upper()
        if ended:
            audit.finding("records_after_end", "warning", {"source": source_id, "line": line_number}, "records occur after END and were not interpreted")
            continue
        if record in {"ATOM", "HETATM"}:
            if len(atoms) >= max_atoms:
                raise ArgumentError(f"PDB contains more than the {max_atoms}-atom limit")
            if not explicit_model and not models:
                models.add(1)
            atom = _parse_atom(line, line_number=line_number, model=current_model)
            if atom.serial in model_atom_serials[current_model]:
                audit.finding("atom_serial_duplicate", "error", {"source": source_id, "line": line_number, "model": current_model}, "atom serial is duplicated within a model")
            model_atom_serials[current_model].add(atom.serial)
            atoms.append(atom)
            continue
        if record == "MODEL":
            if model_active:
                audit.finding("model_nested", "error", {"source": source_id, "line": line_number}, "MODEL appears before the preceding model ended")
            model_number = _fixed_int(line, 10, 14, name="model number", line_number=line_number)
            if model_number < 1:
                raise PdbParseError("model number must be positive", line=line_number)
            current_model = model_number
            explicit_model = True
            model_active = True
            models.add(current_model)
            continue
        if record == "ENDMDL":
            if not model_active:
                audit.finding("model_end_without_model", "warning", {"source": source_id, "line": line_number}, "ENDMDL appears without an active MODEL")
            model_active = False
            continue
        if record == "END":
            ended = True
            metadata_counts[record] += 1
            continue
        metadata_counts[record] += 1
        if record == "CRYST1":
            if len(line) < 54:
                raise PdbParseError("CRYST1 row is shorter than the unit-cell columns", line=line_number)
            cell = {
                "a": _fixed_float(line, 6, 15, name="cell a", line_number=line_number),
                "b": _fixed_float(line, 15, 24, name="cell b", line_number=line_number),
                "c": _fixed_float(line, 24, 33, name="cell c", line_number=line_number),
                "alpha": _fixed_float(line, 33, 40, name="cell alpha", line_number=line_number),
                "beta": _fixed_float(line, 40, 47, name="cell beta", line_number=line_number),
                "gamma": _fixed_float(line, 47, 54, name="cell gamma", line_number=line_number),
            }
            if any(cell[key] <= 0 for key in ("a", "b", "c")) or any(not 0 < cell[key] < 180 for key in ("alpha", "beta", "gamma")):
                raise PdbParseError("CRYST1 unit-cell lengths or angles are outside valid bounds", line=line_number)
        elif record == "SEQRES":
            chain = line[11:12].strip() or "."
            seqres[chain].extend(line[19:].split())
        elif record == "CONECT":
            source_serial = _fixed_int(line, 6, 11, name="CONECT source", line_number=line_number)
            for offset in range(11, len(line), 5):
                field = line[offset : offset + 5].strip()
                if field:
                    try:
                        target_serial = int(field)
                    except ValueError as error:
                        raise PdbParseError("CONECT target is not an integer", line=line_number) from error
                    conect_edges.append((current_model, source_serial, target_serial, line_number))
        elif record == "REMARK":
            match = _RESOLUTION.search(line)
            if match is not None:
                resolution = float(match.group(1))

    if not atoms:
        audit.finding("atom_missing", "error", {"source": source_id}, "PDB contains no ATOM or HETATM records")
    atom_keys = {(atom.model, atom.serial) for atom in atoms}
    unresolved_conect = 0
    for model, source_serial, target_serial, line_number in conect_edges:
        if (model, source_serial) not in atom_keys or (model, target_serial) not in atom_keys:
            unresolved_conect += 1
            audit.finding("conect_unresolved", "error", {"source": source_id, "line": line_number, "model": model}, "CONECT references an atom serial absent from the same model")

    chain_keys = {(atom.model, atom.chain_id) for atom in atoms}
    residue_keys = {(atom.model, atom.chain_id, atom.residue_number, atom.insertion_code, atom.residue_name) for atom in atoms}
    element_counts: Counter[str] = Counter(atom.element or "unknown" for atom in atoms)
    record_type_counts: Counter[str] = Counter(atom.record_type for atom in atoms)
    alt_loc_counts: Counter[str] = Counter(atom.alt_loc for atom in atoms)
    chain_digests = {_digest(source_id, f"{model}:{chain}") for model, chain in chain_keys}
    residue_digests = {
        _digest(source_id, f"{model}:{chain}:{number}:{insertion}:{residue}")
        for model, chain, number, insertion, residue in residue_keys
    }
    coords = [(atom.x, atom.y, atom.z) for atom in atoms]
    coordinate_min = [min(values) for values in zip(*coords)] if coords else None
    coordinate_max = [max(values) for values in zip(*coords)] if coords else None
    centroid = [round(sum(values) / len(values), 6) for values in zip(*coords)] if coords else None
    occupancies = [atom.occupancy for atom in atoms if atom.occupancy is not None]
    temp_factors = [atom.temp_factor for atom in atoms if atom.temp_factor is not None]
    model_serial_gaps = sum(max(0, max(serials) - min(serials) + 1 - len(serials)) for serials in model_atom_serials.values() if serials)
    atom_rows: list[dict[str, Any]] = []
    for number, atom in enumerate(atoms, start=1):
        atom_identity = f"{atom.model}:{atom.serial}:{atom.chain_id}:{atom.residue_number}:{atom.insertion_code}:{atom.atom_name}"
        atom_rows.append(
            {
                "atom": number,
                "line": atom.line,
                "model": atom.model,
                "record_type": atom.record_type,
                "atom_digest": _digest(source_id, atom_identity),
                "chain_digest": _digest(source_id, f"{atom.model}:{atom.chain_id}"),
                "residue_digest": _digest(source_id, f"{atom.model}:{atom.chain_id}:{atom.residue_number}:{atom.insertion_code}:{atom.residue_name}"),
                "element": atom.element or None,
                "coordinate_digest": _digest(source_id, f"{atom.x:.6f}:{atom.y:.6f}:{atom.z:.6f}"),
                "occupancy_present": atom.occupancy is not None,
                "temperature_factor_present": atom.temp_factor is not None,
                "alt_loc": atom.alt_loc,
            }
        )

    valid = audit.errors == 0
    publishable = valid and audit.max_loss_severity != "blocking"
    source_digest = content_digest({"source_id": source_id, "payload": text})
    manifest = {
        "source_id": source_id,
        "source_digest": source_digest,
        "adapter": PDB_ADAPTER,
        "adapter_version": PDB_ADAPTER_VERSION,
        "declared_format": PDB_FORMAT,
        "atom_count": len(atoms),
        "model_count": len(models),
        "chain_count": len(chain_keys),
        "residue_count": len(residue_keys),
        "provenance_digest": provenance_digest,
        "bytes_read": True,
        "identifiers_disclosed": False,
        "raw_records_disclosed": False,
    }
    document: dict[str, Any] = {
        "schema": PDB_SCHEMA,
        "workflow": "pdb_structure_metadata_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": manifest,
        "summary": {
            "atoms": len(atoms),
            "hetero_atoms": sum(1 for atom in atoms if atom.record_type == "HETATM"),
            "models": len(models),
            "chains": len(chain_keys),
            "residues": len(residue_keys),
            "chain_digest_count": len(chain_digests),
            "residue_digest_count": len(residue_digests),
            "element_counts": dict(sorted(element_counts.items())),
            "record_type_counts": dict(sorted(record_type_counts.items())),
            "alternate_location_counts": dict(sorted(alt_loc_counts.items())),
            "model_serial_gaps": model_serial_gaps,
            "conect_edges": len(conect_edges),
            "unresolved_conect_edges": unresolved_conect,
            "coordinate_min": coordinate_min,
            "coordinate_max": coordinate_max,
            "centroid": centroid,
            "occupancy_min": min(occupancies) if occupancies else None,
            "occupancy_max": max(occupancies) if occupancies else None,
            "temperature_factor_min": min(temp_factors) if temp_factors else None,
            "temperature_factor_max": max(temp_factors) if temp_factors else None,
            "resolution": resolution,
            "crystallographic_cell": cell,
            "seqres_chain_count": len(seqres),
            "metadata_record_counts": dict(sorted(metadata_counts.items())),
            "errors": audit.errors,
            "warnings": audit.warnings,
            "finding_count": audit.finding_count,
            "blocking_loss_count": audit.blocking_loss_count,
            "lines_read": line_count,
        },
        "atoms": atom_rows[:max_items],
        "omitted_atoms": max(0, len(atom_rows) - max_items),
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
                "fixed_column_atoms": "pass" if atoms else "fail",
                "model_identity": "pass" if len(atom_keys) == len(atoms) else "fail",
                "connectivity": "pass" if unresolved_conect == 0 else "fail",
                "coordinate_finiteness": "pass",
                "provenance": "pass" if provenance_digest is not None else "fail",
            },
            "limitations": [
                "atom and residue identifiers, chain labels, and raw fixed-column records are represented by source-bound digests or aggregate counts",
                "coordinates are summarized in the source-local frame; biological assembly, symmetry expansion, and reference context are not inferred",
                "the audit validates PDB structure and metadata, not stereochemical correctness, refinement quality, or biological interpretation",
            ],
        },
        "max_atoms": MAX_PDB_ATOMS,
        "max_items": max_items,
    }
    document["document_digest"] = content_digest(document)
    return PdbParseResult(document)


class PdbAdapter:
    """Concrete adapter facade matching the dependency-free bounded PDB route."""

    name = PDB_ADAPTER
    version = PDB_ADAPTER_VERSION
    accepted_formats = ("application/pdb", "chemical/x-pdb", "text/pdb")
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
            "scope_dimensions": ["subject", "sample", "structure", "chain", "residue", "atom"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def parse(
        self,
        pdb: str | bytes,
        *,
        source_id: str,
        provenance: Mapping[str, Any] | None = None,
        max_bytes: int = MAX_PDB_BYTES,
        max_atoms: int = MAX_PDB_ATOMS,
        max_items: int = MAX_PDB_ITEMS,
    ) -> PdbParseResult:
        return parse_pdb(
            pdb,
            source_id=source_id,
            provenance=provenance,
            max_bytes=max_bytes,
            max_atoms=max_atoms,
            max_items=max_items,
        )


__all__ = [
    "MAX_PDB_ATOMS",
    "MAX_PDB_BYTES",
    "MAX_PDB_ITEMS",
    "PDB_ADAPTER",
    "PDB_ADAPTER_VERSION",
    "PDB_FORMAT",
    "PDB_SCHEMA",
    "PdbAdapter",
    "PdbFinding",
    "PdbParseError",
    "PdbParseResult",
    "parse_pdb",
]
