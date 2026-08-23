"""Format-family conformance profiles for concrete adapter runtime results.

Profiles make the checks required for a structural adapter observation explicit. They do not
claim clinical validity, biological truth, ontology resolution, or release readiness; a profile
only determines whether the declared bounded checks were observed, incomplete, or refused.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
from typing import Any, Mapping

from .adapter_execution_evidence import AdapterExecutionEvidenceRequest
from .adapter_runtime import AdapterExecutionResult, RuntimeStatus
from .authoring import content_digest
from .errors import ArgumentError

ADAPTER_CONFORMANCE_SCHEMA = "bioprism-python-adapter-conformance/0.1"
ADAPTER_CONFORMANCE_STATUSES = frozenset({"verified", "partial", "refused", "unsupported"})


def _text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte bound")
    if any(ord(character) < 0x20 for character in value):
        raise ArgumentError(f"{name} must not contain control characters")
    return value


def _strings(name: str, value: Any, *, maximum: int = 64) -> tuple[str, ...]:
    if not isinstance(value, (list, tuple)) or len(value) > maximum:
        raise ArgumentError(f"{name} must contain at most {maximum} strings")
    result = tuple(_text(name, item, 256) for item in value)
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} must not contain duplicates")
    return result


_PROFILE_SPECS: dict[str, tuple[str, tuple[str, ...]]] = {
    "bioprism.inventory": (
        "catalogue_only",
        ("route_declared",),
    ),
    "bioprism.tabular": (
        "catalogue_only",
        ("route_declared",),
    ),
    "bioprism.python.vcf_text": (
        "variant_text",
        ("fileformat_header", "column_header", "record_structure", "typed_value_projection", "semantic_loss_audit"),
    ),
    "bioprism.python.vcf_indexed": (
        "variant_indexed",
        ("fileformat_header", "column_header", "record_structure", "typed_value_projection", "semantic_loss_audit"),
    ),
    "bioprism.python.bids_manifest": (
        "imaging_manifest",
        ("relative_paths", "entity_syntax", "sidecar_inheritance", "participant_coverage", "dataset_description"),
    ),
    "bioprism.python.dicom_metadata": (
        "clinical_imaging_metadata",
        ("identity_hierarchy", "required_tags", "dimensions", "geometry", "provenance"),
    ),
    "bioprism.python.dicom": (
        "clinical_imaging_binary",
        ("identity_hierarchy", "required_tags", "dimensions", "geometry", "provenance"),
    ),
    "bioprism.python.nifti_metadata": (
        "neuroimaging_metadata",
        ("shape_and_datatype", "affine", "form_declarations", "series_consistency", "provenance"),
    ),
    "bioprism.python.nifti_bids": (
        "neuroimaging_binary",
        ("shape_and_datatype", "affine", "form_declarations", "series_consistency", "provenance"),
    ),
    "bioprism.python.anndata_metadata": (
        "single_cell_metadata",
        ("dimensions", "indices", "annotations", "matrix_shapes", "provenance"),
    ),
    "bioprism.python.anndata": (
        "single_cell_binary",
        ("dimensions", "indices", "annotations", "matrix_shapes", "provenance"),
    ),
    "bioprism.python.alignment_metadata": (
        "alignment_metadata",
        ("reference_dictionary", "cigar", "coordinates", "pairing", "sort_order", "provenance"),
    ),
    "bioprism.python.bam_cram": (
        "alignment_binary",
        ("reference_dictionary", "cigar", "coordinates", "pairing", "sort_order", "provenance"),
    ),
    "bioprism.python.fasta_text": (
        "reference_sequence",
        ("record_structure", "identifier_uniqueness", "alphabet", "provenance"),
    ),
    "bioprism.python.fastq_text": (
        "sequencing_reads",
        ("record_structure", "sequence_quality_lengths", "quality_printability", "pairing_evidence", "provenance"),
    ),
    "bioprism.python.sam_text": (
        "alignment_text",
        ("header_structure", "alignment_fields", "cigar_semantics", "coordinate_bounds", "pairing", "sort_order", "provenance"),
    ),
    "bioprism.python.bed_text": (
        "interval_annotation",
        ("interval_structure", "coordinate_order", "block_structure", "provenance"),
    ),
    "bioprism.python.gff3_text": (
        "genome_annotation",
        ("feature_structure", "coordinate_order", "identifier_uniqueness", "parent_resolution", "provenance"),
    ),
    "bioprism.python.mzml_text": (
        "mass_spectrometry_metadata",
        ("root", "spectrum_list", "spectrum_identity", "binary_boundaries", "provenance"),
    ),
    "bioprism.python.pdb_text": (
        "structural_biology",
        ("fixed_column_atoms", "model_identity", "connectivity", "coordinate_finiteness", "provenance"),
    ),
    "bioprism.python.sdf_text": (
        "small_molecule",
        ("record_structure", "atom_bond_integrity", "data_fields", "graph_components", "provenance"),
    ),
    "bioprism.python.fhir_manifest": (
        "clinical_interoperability",
        ("resource_identity", "bundle_structure", "reference_scope", "provenance"),
    ),
    "bioprism.python.fhir_json": (
        "clinical_interoperability_file",
        ("resource_identity", "bundle_structure", "reference_scope", "provenance"),
    ),
    "bioprism.python.fhir_ndjson": (
        "clinical_interoperability_bulk",
        ("resource_identity", "bundle_structure", "reference_scope", "provenance"),
    ),
    "bioprism.python.ome_zarr": (
        "multiscale_imaging",
        ("axes", "datasets", "transforms", "channels_labels", "provenance"),
    ),
}


@dataclass(frozen=True)
class AdapterConformanceProfile:
    """Required bounded checks for one concrete adapter family."""

    adapter_id: str
    family: str
    required_checks: tuple[str, ...]

    def __post_init__(self) -> None:
        _text("adapter conformance adapter_id", self.adapter_id, 256)
        _text("adapter conformance family", self.family, 128)
        checks = _strings("adapter conformance required_checks", self.required_checks)
        if not checks:
            raise ArgumentError("adapter conformance profile requires at least one check")
        object.__setattr__(self, "required_checks", checks)

    @property
    def profile_digest(self) -> str:
        return content_digest(self.to_wire())

    def to_wire(self) -> dict[str, Any]:
        return {
            "schema": ADAPTER_CONFORMANCE_SCHEMA,
            "adapter_id": self.adapter_id,
            "family": self.family,
            "required_checks": list(self.required_checks),
        }


@dataclass(frozen=True)
class AdapterConformanceReport:
    """Result of applying a declared profile to one runtime document."""

    profile: AdapterConformanceProfile
    adapter_version: str | None
    status: str
    passed_claimed: bool
    observed_checks: Mapping[str, str]
    missing_checks: tuple[str, ...]
    failed_checks: tuple[str, ...]
    limitations: tuple[str, ...]
    reason: str | None = None

    def __post_init__(self) -> None:
        if self.status not in ADAPTER_CONFORMANCE_STATUSES:
            raise ArgumentError("adapter conformance status is invalid")
        if self.adapter_version is not None:
            _text("adapter conformance adapter_version", self.adapter_version, 256)
        _strings("adapter conformance missing_checks", self.missing_checks)
        _strings("adapter conformance failed_checks", self.failed_checks)
        _strings("adapter conformance limitations", self.limitations, maximum=32)
        if self.reason is not None:
            _text("adapter conformance reason", self.reason, 512)

    @property
    def report_digest(self) -> str:
        return content_digest(self._digest_input())

    @property
    def verified(self) -> bool:
        return self.status == "verified"

    def _digest_input(self) -> dict[str, Any]:
        return {
            "schema": ADAPTER_CONFORMANCE_SCHEMA,
            "profile": self.profile.to_wire(),
            "profile_digest": self.profile.profile_digest,
            "adapter_version": self.adapter_version,
            "status": self.status,
            "passed_claimed": self.passed_claimed,
            "observed_checks": dict(sorted(self.observed_checks.items())),
            "missing_checks": list(self.missing_checks),
            "failed_checks": list(self.failed_checks),
            "limitations": list(self.limitations),
            "reason": self.reason,
        }

    def to_wire(self) -> dict[str, Any]:
        result = self._digest_input()
        result["report_digest"] = self.report_digest
        return result

    def to_adapter_execution_evidence_request(
        self,
        result: AdapterExecutionResult,
        group_id: str,
        domains: tuple[str, ...],
        *,
        subject_id: str,
        input_digest: str,
        parent_digests: tuple[str, ...] = (),
        attempt_id: str | None = None,
    ) -> AdapterExecutionEvidenceRequest:
        """Attach the conformance report digest as an explicit evidence parent."""

        if result.adapter is None or result.adapter.id != self.profile.adapter_id:
            raise ArgumentError("conformance report adapter does not match the runtime result")
        evidence = result.to_adapter_execution_evidence_request(
            group_id,
            domains,
            subject_id=subject_id,
            input_digest=input_digest,
            parent_digests=parent_digests,
            attempt_id=attempt_id,
        )
        parents = tuple(dict.fromkeys((*evidence.parent_digests, self.report_digest)))
        return replace(evidence, parent_digests=parents)


def adapter_conformance_profile(adapter_id: str) -> AdapterConformanceProfile:
    """Return the explicit profile for one catalogued concrete adapter route."""

    _text("adapter conformance adapter_id", adapter_id, 256)
    try:
        family, checks = _PROFILE_SPECS[adapter_id]
    except KeyError as error:
        raise ArgumentError(f"no conformance profile is registered for {adapter_id!r}") from error
    return AdapterConformanceProfile(adapter_id, family, checks)


def adapter_conformance_profiles() -> tuple[AdapterConformanceProfile, ...]:
    """Return all concrete route profiles in deterministic adapter-id order."""

    return tuple(adapter_conformance_profile(adapter_id) for adapter_id in sorted(_PROFILE_SPECS))


def _check_value(value: Any) -> str:
    if value is True:
        return "pass"
    if value is False:
        return "fail"
    if isinstance(value, str) and value.strip():
        return value
    return "invalid"


def evaluate_adapter_conformance(
    result: AdapterExecutionResult,
    profile: AdapterConformanceProfile | None = None,
) -> AdapterConformanceReport:
    """Evaluate one runtime result without inferring scientific or release readiness."""

    if not isinstance(result, AdapterExecutionResult):
        raise ArgumentError("result must be an AdapterExecutionResult")
    selected = profile or adapter_conformance_profile(result.request.adapter_id)
    if result.adapter is not None and result.adapter.id != selected.adapter_id:
        raise ArgumentError("conformance profile adapter_id does not match the runtime result")
    adapter_version = result.adapter.version if result.adapter is not None else None
    error_detail = result.error.get("detail") if result.error else None
    if not isinstance(error_detail, str) or not error_detail.strip():
        error_detail = None

    if result.status is RuntimeStatus.UNSUPPORTED:
        return AdapterConformanceReport(selected, adapter_version, "unsupported", False, {}, selected.required_checks, (), (), error_detail or "adapter execution is unsupported")
    if result.status in {RuntimeStatus.REJECTED, RuntimeStatus.BLOCKED} or result.adapter is None:
        return AdapterConformanceReport(selected, adapter_version, "refused", False, {}, selected.required_checks, (), (), error_detail or "adapter execution did not produce a conformance document")

    document = result.document if isinstance(result.document, Mapping) else None
    conformance = document.get("conformance") if document is not None else None
    checks = conformance.get("checks") if isinstance(conformance, Mapping) else None
    observed: dict[str, str] = {}
    if isinstance(checks, Mapping):
        for key in selected.required_checks:
            if key in checks:
                observed[key] = _check_value(checks[key])
    missing = tuple(check for check in selected.required_checks if check not in observed)
    failed = tuple(check for check, value in observed.items() if value != "pass")
    passed_claimed = isinstance(conformance, Mapping) and conformance.get("passed") is True
    limitations = ()
    if isinstance(conformance, Mapping):
        raw_limitations = conformance.get("limitations", ())
        if isinstance(raw_limitations, (list, tuple)):
            limitations = tuple(item for item in raw_limitations if isinstance(item, str))[:32]
    status = "verified" if passed_claimed and not missing and not failed else "partial"
    reason = None
    if missing:
        reason = "required conformance checks were not emitted: " + ", ".join(missing)
    elif failed:
        reason = "required conformance checks failed: " + ", ".join(failed)
    elif not passed_claimed:
        reason = "adapter document did not claim conformance passed"
    return AdapterConformanceReport(
        selected,
        adapter_version,
        status,
        passed_claimed,
        observed,
        missing,
        failed,
        limitations,
        reason,
    )


__all__ = [
    "ADAPTER_CONFORMANCE_SCHEMA",
    "ADAPTER_CONFORMANCE_STATUSES",
    "AdapterConformanceProfile",
    "AdapterConformanceReport",
    "adapter_conformance_profile",
    "adapter_conformance_profiles",
    "evaluate_adapter_conformance",
]
