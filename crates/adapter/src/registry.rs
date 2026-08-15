//! Deterministic adapter discovery and source planning.
//!
//! A format boundary is useful only when it tells the caller what will happen before bytes are
//! read. This module is the planning half of that boundary. It does not sniff content, fetch
//! sources, import optional Python packages, or execute an adapter. It answers a narrower and
//! more valuable question: given a caller-declared format, source shape, requested conformance
//! level, and an optional dependency inventory, which adapter can honestly be selected?
//!
//! The catalogue deliberately contains both native adapters and delegated biological adapters.
//! Rust owns the contract and the loss vocabulary; Python owns the reference implementations for
//! heavyweight formats whose mature ecosystems are the practical specification. A delegated
//! entry is therefore not a promise that this crate parses DICOM or AnnData. It is a typed route
//! to the layer that is allowed to do so, with its dependency and loss surface visible before
//! execution.

use crate::adapter::ConformanceLevel;
use crate::loss::LossKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const ADAPTER_REGISTRY_SCHEMA_VERSION: &str = "bioprism-adapter-registry/0.1";
pub const MAX_SOURCE_ID_BYTES: usize = 512;
pub const MAX_FORMAT_BYTES: usize = 256;
pub const MAX_DEPENDENCIES: usize = 128;
pub const MAX_CANDIDATES: usize = 64;

/// The physical shape a caller has already obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Bytes,
    Directory,
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceKind::Bytes => "bytes",
            SourceKind::Directory => "directory",
        }
    }
}

/// Where the implementation lives. `PythonDelegated` is an explicit architecture boundary,
/// not a best-effort fallback from a failed native parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterExecution {
    Native,
    PythonDelegated,
}

/// Why a catalogue entry is or is not executable for this request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Ready,
    DependencyUnknown,
    DependencyMissing,
    UnsupportedFormat,
    UnsupportedSourceKind,
    UnsupportedConformance,
}

impl PlanStatus {
    pub fn is_executable(self) -> bool {
        matches!(self, PlanStatus::Ready)
    }
}

/// A stable catalogue entry. The fields intentionally mirror [`crate::AdapterManifest`] while
/// adding planning-only details such as source shapes and optional runtime dependencies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterDescriptor {
    pub id: String,
    pub version: String,
    pub execution: AdapterExecution,
    pub accepted_formats: Vec<String>,
    pub accepts_undeclared_format: bool,
    pub source_kinds: BTreeSet<SourceKind>,
    pub conformance_level: ConformanceLevel,
    pub declared_loss_kinds: BTreeSet<LossKind>,
    pub scope_dimensions: BTreeSet<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional_dependency: Option<String>,
    pub description: String,
}

impl AdapterDescriptor {
    pub fn accepts_format(&self, format: Option<&str>) -> bool {
        match format {
            None => self.accepts_undeclared_format,
            Some(format) => {
                let normalized = normalize_format(format);
                self.accepted_formats
                    .iter()
                    .any(|candidate| candidate == &normalized)
            }
        }
    }
}

/// The bounded input to [`AdapterRegistry::plan`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterPlanRequest {
    pub source_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_format: Option<String>,
    pub source_kind: SourceKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_conformance: Option<ConformanceLevel>,
    /// `None` means the caller did not perform an environment check. It must not be interpreted
    /// as "all dependencies are installed".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_dependencies: Option<BTreeSet<String>>,
}

impl AdapterPlanRequest {
    pub fn validate(&self) -> Result<(), RegistryError> {
        validate_text("source_id", &self.source_id, MAX_SOURCE_ID_BYTES)?;
        if let Some(format) = &self.declared_format {
            validate_text("declared_format", format, MAX_FORMAT_BYTES)?;
            if format.trim().is_empty() {
                return Err(RegistryError::EmptyFormat);
            }
        }
        if let Some(dependencies) = &self.available_dependencies {
            if dependencies.len() > MAX_DEPENDENCIES {
                return Err(RegistryError::TooManyDependencies {
                    maximum: MAX_DEPENDENCIES,
                });
            }
            for dependency in dependencies {
                validate_text("available_dependencies", dependency, MAX_FORMAT_BYTES)?;
            }
        }
        Ok(())
    }
}

/// One bounded explanation for a catalogue entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterPlanCandidate {
    pub adapter: AdapterDescriptor,
    pub status: PlanStatus,
    pub reasons: Vec<String>,
}

/// The complete deterministic plan. The MCP layer adds a content hash; keeping the semantic
/// result here makes the same contract available to Rust callers and SDKs without coupling this
/// crate to JSON-RPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterPlan {
    pub schema: String,
    pub request: AdapterPlanRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_adapter: Option<AdapterDescriptor>,
    pub executable: bool,
    pub candidates: Vec<AdapterPlanCandidate>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RegistryError {
    #[error("{field} must not be empty")]
    EmptyField { field: &'static str },
    #[error("{field} exceeds the {maximum}-byte limit")]
    TextTooLong { field: &'static str, maximum: usize },
    #[error("declared_format must not be empty when supplied")]
    EmptyFormat,
    #[error("available_dependencies exceeds the {maximum}-entry limit")]
    TooManyDependencies { maximum: usize },
}

/// The built-in cross-domain adapter catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterRegistry {
    descriptors: Vec<AdapterDescriptor>,
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::built_in()
    }
}

impl AdapterRegistry {
    pub fn built_in() -> Self {
        let mut descriptors = vec![
            descriptor(
                "bioprism.tabular",
                "0.1.0",
                AdapterExecution::Native,
                &["text/csv", "text/tab-separated-values", "text/tsv"],
                true,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::UnmappedColumn,
                    LossKind::UnpreservedUnit,
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::PrecisionReduced,
                    LossKind::ProvenanceUnavailable,
                    LossKind::OntologyTermUnmapped,
                    LossKind::TypeUndetermined,
                ],
                &["subject", "specimen", "observation"],
                None,
                "Validated CSV/TSV normalization under an explicit mapping profile.",
            ),
            descriptor(
                "bioprism.inventory",
                "0.1.0",
                AdapterExecution::Native,
                &["application/x-directory", "inode/directory"],
                true,
                &[SourceKind::Directory],
                ConformanceLevel::Normalize,
                &[LossKind::ContentUninterpreted, LossKind::ProvenanceUnavailable],
                &["repository", "artifact"],
                None,
                "Deterministic artifact inventory with hashes and explicit unread-content loss.",
            ),
            descriptor(
                "bioprism.python.dicom",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/dicom", "application/dicom+json"],
                false,
                &[SourceKind::Bytes, SourceKind::Directory],
                ConformanceLevel::Normalize,
                &[
                    LossKind::UnpreservedUnit,
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::PrecisionReduced,
                    LossKind::ProvenanceUnavailable,
                    LossKind::OntologyTermUnmapped,
                    LossKind::TypeUndetermined,
                    LossKind::ContentUninterpreted,
                ],
                &["subject", "specimen", "acquisition", "image"],
                Some("pydicom"),
                "Python-owned DICOM adapter route; the Rust layer plans and audits the boundary.",
            ),
            descriptor(
                "bioprism.python.dicom_metadata",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/dicom-manifest"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::ProvenanceUnavailable,
                    LossKind::ContentUninterpreted,
                    LossKind::TypeUndetermined,
                ],
                &["subject", "specimen", "acquisition", "image"],
                None,
                "Dependency-free audit of parsed DICOM identity, study/series hierarchy, frame geometry, and provenance; pixels remain uninterpreted.",
            ),
            descriptor(
                "bioprism.python.bids_manifest",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/bids-manifest"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::ProvenanceUnavailable,
                    LossKind::ContentUninterpreted,
                    LossKind::TypeUndetermined,
                ],
                &["subject", "session", "acquisition", "image", "event"],
                None,
                "Dependency-free BIDS manifest, entity, sidecar-inheritance, and participant audit; binary image bytes remain uninterpreted.",
            ),
            descriptor(
                "bioprism.python.nifti_bids",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/nifti", "application/x-nifti", "application/bids"],
                false,
                &[SourceKind::Bytes, SourceKind::Directory],
                ConformanceLevel::Normalize,
                &[
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::PrecisionReduced,
                    LossKind::ProvenanceUnavailable,
                    LossKind::OntologyTermUnmapped,
                    LossKind::TypeUndetermined,
                    LossKind::ContentUninterpreted,
                ],
                &["subject", "session", "acquisition", "image"],
                Some("nibabel"),
                "Python-owned NIfTI/BIDS adapter route with affine and sidecar provenance checks.",
            ),
            descriptor(
                "bioprism.python.nifti_metadata",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/nifti-manifest"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::ProvenanceUnavailable,
                    LossKind::ContentUninterpreted,
                    LossKind::TypeUndetermined,
                ],
                &["subject", "session", "acquisition", "image", "voxel"],
                None,
                "Dependency-free audit of parsed NIfTI shape, datatype, affine, qform/sform, units, and coordinate-frame metadata; arrays remain uninterpreted.",
            ),
            descriptor(
                "bioprism.python.anndata",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/anndata", "application/h5ad", "application/zarr"],
                false,
                &[SourceKind::Bytes, SourceKind::Directory],
                ConformanceLevel::Normalize,
                &[
                    LossKind::UnmappedColumn,
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::PrecisionReduced,
                    LossKind::ProvenanceUnavailable,
                    LossKind::OntologyTermUnmapped,
                    LossKind::ContentUninterpreted,
                ],
                &["subject", "cell", "feature", "assay"],
                Some("anndata"),
                "Python-owned AnnData/Zarr adapter route preserving obs/var/uns provenance.",
            ),
            descriptor(
                "bioprism.python.anndata_metadata",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/anndata-manifest"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::ProvenanceUnavailable,
                    LossKind::ContentUninterpreted,
                    LossKind::TypeUndetermined,
                ],
                &["subject", "cell", "feature", "assay", "embedding"],
                None,
                "Dependency-free audit of parsed AnnData/Zarr dimensions, indices, annotations, layers, embeddings, and sparse matrix metadata; payloads remain uninterpreted.",
            ),
            descriptor(
                "bioprism.python.vcf_text",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["text/vcf", "text/x-vcf", "application/vcf"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::PrecisionReduced,
                    LossKind::ProvenanceUnavailable,
                    LossKind::OntologyTermUnmapped,
                    LossKind::TypeUndetermined,
                    LossKind::ContentUninterpreted,
                ],
                &["subject", "sample", "variant", "genome"],
                None,
                "Dependency-free bounded text VCF adapter route requiring reference-build and sample identity checks.",
            ),
            descriptor(
                "bioprism.python.vcf_indexed",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/bcf", "application/vcf+bgzip", "application/vcf+gzip"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::PrecisionReduced,
                    LossKind::ProvenanceUnavailable,
                    LossKind::OntologyTermUnmapped,
                    LossKind::TypeUndetermined,
                    LossKind::ContentUninterpreted,
                ],
                &["subject", "sample", "variant", "genome"],
                Some("pysam"),
                "Python-owned indexed/compressed VCF and BCF route using pysam with reference-build and sample identity checks.",
            ),
            descriptor(
                "bioprism.python.bam_cram",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/bam", "application/cram"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::PrecisionReduced,
                    LossKind::ProvenanceUnavailable,
                    LossKind::ContentUninterpreted,
                ],
                &["subject", "sample", "read", "reference"],
                Some("pysam"),
                "Python-owned BAM/CRAM adapter route preserving reference and alignment metadata.",
            ),
            descriptor(
                "bioprism.python.alignment_metadata",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/alignment-manifest"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::ProvenanceUnavailable,
                    LossKind::ContentUninterpreted,
                    LossKind::TypeUndetermined,
                ],
                &["subject", "sample", "read", "reference", "locus"],
                None,
                "Dependency-free audit of parsed BAM/CRAM records, CIGAR accounting, coordinates, flags, pairing, sort order, and coverage; read payloads remain uninterpreted.",
            ),
            descriptor(
                "bioprism.python.fastq_text",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/fastq", "text/fastq", "text/x-fastq"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::ProvenanceUnavailable,
                    LossKind::ContentUninterpreted,
                    LossKind::TypeUndetermined,
                ],
                &["subject", "sample", "read", "sequence", "quality"],
                None,
                "Dependency-free bounded FASTQ reader validating complete records, quality lengths, and paired-read evidence without disclosing read content.",
            ),
            descriptor(
                "bioprism.python.fasta_text",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/fasta", "text/fasta", "text/x-fasta"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::ProvenanceUnavailable,
                    LossKind::ContentUninterpreted,
                    LossKind::TypeUndetermined,
                ],
                &["subject", "sample", "reference", "sequence"],
                None,
                "Dependency-free bounded FASTA reader validating complete records, optional nucleotide/protein alphabets, and duplicate identifiers without disclosing sequence content.",
            ),
            descriptor(
                "bioprism.python.gff3_text",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/gff3", "text/gff3", "application/gtf", "text/x-gtf"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::ContentUninterpreted,
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::OntologyTermUnmapped,
                    LossKind::ProvenanceUnavailable,
                ],
                &["subject", "sample", "reference", "feature", "interval"],
                None,
                "Dependency-free bounded GFF3/GTF reader validating coordinates, attributes, parent references, and feature hierarchy without disclosing attribute values.",
            ),
            descriptor(
                "bioprism.python.pdb_text",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/pdb", "chemical/x-pdb", "text/pdb"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::ContentUninterpreted,
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::OntologyTermUnmapped,
                    LossKind::ProvenanceUnavailable,
                ],
                &["subject", "sample", "structure", "chain", "residue", "atom"],
                None,
                "Dependency-free bounded PDB fixed-column reader validating models, coordinates, chains, residues, and connectivity without disclosing raw structure records.",
            ),
            descriptor(
                "bioprism.python.fhir_ndjson",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/fhir+ndjson"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::ProvenanceUnavailable,
                    LossKind::OntologyTermUnmapped,
                    LossKind::ContentUninterpreted,
                    LossKind::TypeUndetermined,
                ],
                &["subject", "encounter", "resource", "terminology", "time"],
                None,
                "Dependency-free bounded FHIR Bulk Data NDJSON reader with complete-record validation and privacy-safe reference projection.",
            ),
            descriptor(
                "bioprism.python.fhir_json",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/fhir+json"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::ProvenanceUnavailable,
                    LossKind::OntologyTermUnmapped,
                    LossKind::ContentUninterpreted,
                    LossKind::TypeUndetermined,
                ],
                &["subject", "encounter", "resource", "terminology", "time"],
                None,
                "Dependency-free bounded FHIR JSON resource and Bundle reader with privacy-safe reference projection.",
            ),
            descriptor(
                "bioprism.python.fhir_manifest",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/fhir-manifest"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::ProvenanceUnavailable,
                    LossKind::OntologyTermUnmapped,
                    LossKind::ContentUninterpreted,
                    LossKind::TypeUndetermined,
                ],
                &["subject", "encounter", "resource", "terminology", "time"],
                None,
                "Dependency-free audit of parsed FHIR structure, resource identity, references, profiles, and provenance; clinical values remain uninterpreted.",
            ),
            descriptor(
                "bioprism.python.mzml_text",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/mzml", "application/xml+mass-spectrometry", "text/mzml"],
                false,
                &[SourceKind::Bytes],
                ConformanceLevel::Normalize,
                &[
                    LossKind::ProvenanceUnavailable,
                    LossKind::OntologyTermUnmapped,
                    LossKind::ContentUninterpreted,
                    LossKind::TypeUndetermined,
                ],
                &["subject", "sample", "assay", "spectrum", "ion"],
                None,
                "Dependency-free bounded mzML XML metadata reader that audits spectra and binary-array declarations without decoding payloads.",
            ),
            descriptor(
                "bioprism.python.ome_zarr",
                "0.1.0",
                AdapterExecution::PythonDelegated,
                &["application/ome-zarr", "application/x-zarr"],
                false,
                &[SourceKind::Directory],
                ConformanceLevel::Normalize,
                &[
                    LossKind::CoordinateFrameNotCarried,
                    LossKind::PrecisionReduced,
                    LossKind::ProvenanceUnavailable,
                    LossKind::OntologyTermUnmapped,
                    LossKind::ContentUninterpreted,
                ],
                &["subject", "specimen", "image", "tile"],
                Some("zarr"),
                "Python-owned OME-Zarr adapter route preserving multiscale and spatial metadata.",
            ),
        ];
        descriptors.sort_by(|left, right| left.id.cmp(&right.id));
        AdapterRegistry { descriptors }
    }

    pub fn descriptors(&self) -> &[AdapterDescriptor] {
        &self.descriptors
    }

    pub fn plan(&self, request: AdapterPlanRequest) -> Result<AdapterPlan, RegistryError> {
        request.validate()?;
        let mut candidates = self
            .descriptors
            .iter()
            .map(|descriptor| plan_candidate(descriptor, &request))
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.status
                .cmp(&right.status)
                .then_with(|| left.adapter.id.cmp(&right.adapter.id))
        });
        candidates.truncate(MAX_CANDIDATES);
        let selected_adapter = candidates
            .iter()
            .find(|candidate| candidate.status.is_executable())
            .map(|candidate| candidate.adapter.clone());
        let executable = selected_adapter.is_some();
        let mut limitations = vec![
            "format matching is explicit; the planner never sniffs source bytes".to_string(),
            "planning does not fetch, parse, execute, or grant credentials".to_string(),
            "semantic-loss declarations describe the adapter surface; source-specific loss is only known after conformance".to_string(),
        ];
        if selected_adapter
            .as_ref()
            .is_some_and(|adapter| adapter.execution == AdapterExecution::PythonDelegated)
        {
            limitations.push(
                "the selected implementation is delegated to the Python adapter layer and must run its own independent conformance audit".to_string(),
            );
        }
        if !executable {
            limitations.push(
                "no executable adapter is available for this request; the caller must change the declared format, source shape, conformance requirement, or dependency inventory".to_string(),
            );
        }
        Ok(AdapterPlan {
            schema: ADAPTER_REGISTRY_SCHEMA_VERSION.to_string(),
            request,
            selected_adapter,
            executable,
            candidates,
            limitations,
        })
    }
}

// The catalogue rows are deliberately expanded at the call site so every route's format,
// source, dependency, loss, and scope declarations remain reviewable together.
#[allow(clippy::too_many_arguments)]
fn descriptor(
    id: &str,
    version: &str,
    execution: AdapterExecution,
    formats: &[&str],
    accepts_undeclared_format: bool,
    source_kinds: &[SourceKind],
    conformance_level: ConformanceLevel,
    declared_loss_kinds: &[LossKind],
    scope_dimensions: &[&str],
    optional_dependency: Option<&str>,
    description: &str,
) -> AdapterDescriptor {
    let mut accepted_formats = formats
        .iter()
        .map(|format| normalize_format(format))
        .collect::<Vec<_>>();
    accepted_formats.sort();
    AdapterDescriptor {
        id: id.to_string(),
        version: version.to_string(),
        execution,
        accepted_formats,
        accepts_undeclared_format,
        source_kinds: source_kinds.iter().copied().collect(),
        conformance_level,
        declared_loss_kinds: declared_loss_kinds.iter().copied().collect(),
        scope_dimensions: scope_dimensions
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        optional_dependency: optional_dependency.map(str::to_string),
        description: description.to_string(),
    }
}

fn plan_candidate(
    descriptor: &AdapterDescriptor,
    request: &AdapterPlanRequest,
) -> AdapterPlanCandidate {
    let mut reasons = Vec::new();
    let status = if !descriptor.accepts_format(request.declared_format.as_deref()) {
        reasons.push(match &request.declared_format {
            Some(format) => format!("declared format {format:?} is not accepted by this adapter"),
            None => "this adapter requires an explicit declared format".to_string(),
        });
        PlanStatus::UnsupportedFormat
    } else if !descriptor.source_kinds.contains(&request.source_kind) {
        reasons.push(format!(
            "source kind {} is not supported by this adapter",
            request.source_kind.as_str()
        ));
        PlanStatus::UnsupportedSourceKind
    } else if request
        .required_conformance
        .is_some_and(|required| required > descriptor.conformance_level)
    {
        reasons.push(format!(
            "requested conformance exceeds this adapter's {:?} level",
            descriptor.conformance_level
        ));
        PlanStatus::UnsupportedConformance
    } else {
        match (
            &descriptor.optional_dependency,
            &request.available_dependencies,
        ) {
            (Some(dependency), None) => {
                reasons.push(format!(
                    "optional dependency {dependency:?} was not checked by the caller"
                ));
                PlanStatus::DependencyUnknown
            }
            (Some(dependency), Some(available)) if !available.contains(dependency) => {
                reasons.push(format!(
                    "optional dependency {dependency:?} is absent from the caller inventory"
                ));
                PlanStatus::DependencyMissing
            }
            (Some(dependency), Some(_)) => {
                reasons.push(format!(
                    "optional dependency {dependency:?} is present in the caller inventory"
                ));
                PlanStatus::Ready
            }
            (None, _) => {
                reasons.push("native adapter is available in this runtime".to_string());
                PlanStatus::Ready
            }
        }
    };
    AdapterPlanCandidate {
        adapter: descriptor.clone(),
        status,
        reasons,
    }
}

fn normalize_format(format: &str) -> String {
    format.trim().to_ascii_lowercase()
}

fn validate_text(field: &'static str, value: &str, maximum: usize) -> Result<(), RegistryError> {
    if value.trim().is_empty() {
        return Err(RegistryError::EmptyField { field });
    }
    if value.len() > maximum {
        return Err(RegistryError::TextTooLong { field, maximum });
    }
    if value.chars().any(char::is_control) {
        return Err(RegistryError::EmptyField { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(format: Option<&str>, source_kind: SourceKind) -> AdapterPlanRequest {
        AdapterPlanRequest {
            source_id: "source-1".to_string(),
            declared_format: format.map(str::to_string),
            source_kind,
            required_conformance: Some(ConformanceLevel::Normalize),
            available_dependencies: None,
        }
    }

    #[test]
    fn native_tabular_planning_is_ready_without_optional_dependencies() {
        let plan = AdapterRegistry::default()
            .plan(request(Some("TEXT/CSV"), SourceKind::Bytes))
            .unwrap();
        assert!(plan.executable);
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.tabular")
        );
    }

    #[test]
    fn delegated_planning_distinguishes_unknown_from_missing_dependencies() {
        let registry = AdapterRegistry::default();
        let unknown = registry
            .plan(request(Some("application/dicom"), SourceKind::Bytes))
            .unwrap();
        assert!(!unknown.executable);
        assert_eq!(unknown.candidates[0].status, PlanStatus::DependencyUnknown);

        let mut with_dependency = request(Some("application/dicom"), SourceKind::Bytes);
        with_dependency.available_dependencies = Some(BTreeSet::from(["pydicom".to_string()]));
        let ready = registry.plan(with_dependency).unwrap();
        assert!(ready.executable);
        assert_eq!(
            ready
                .selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.python.dicom")
        );
    }

    #[test]
    fn bounded_text_vcf_selects_the_dependency_free_python_reader() {
        let plan = AdapterRegistry::default()
            .plan(request(Some("text/vcf"), SourceKind::Bytes))
            .unwrap();
        assert!(plan.executable);
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.python.vcf_text")
        );
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .and_then(|adapter| adapter.optional_dependency.as_deref()),
            None
        );
    }

    #[test]
    fn bounded_bids_manifest_selects_the_dependency_free_python_auditor() {
        let plan = AdapterRegistry::default()
            .plan(request(
                Some("application/bids-manifest"),
                SourceKind::Bytes,
            ))
            .unwrap();
        assert!(plan.executable);
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.python.bids_manifest")
        );
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .and_then(|adapter| adapter.optional_dependency.as_deref()),
            None
        );
    }

    #[test]
    fn parsed_dicom_manifest_selects_the_dependency_free_metadata_auditor() {
        let plan = AdapterRegistry::default()
            .plan(request(
                Some("application/dicom-manifest"),
                SourceKind::Bytes,
            ))
            .unwrap();
        assert!(plan.executable);
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.python.dicom_metadata")
        );
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .and_then(|adapter| adapter.optional_dependency.as_deref()),
            None
        );
    }

    #[test]
    fn parsed_nifti_manifest_selects_the_dependency_free_header_auditor() {
        let plan = AdapterRegistry::default()
            .plan(request(
                Some("application/nifti-manifest"),
                SourceKind::Bytes,
            ))
            .unwrap();
        assert!(plan.executable);
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.python.nifti_metadata")
        );
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .and_then(|adapter| adapter.optional_dependency.as_deref()),
            None
        );
    }

    #[test]
    fn parsed_anndata_manifest_selects_the_dependency_free_matrix_auditor() {
        let plan = AdapterRegistry::default()
            .plan(request(
                Some("application/anndata-manifest"),
                SourceKind::Bytes,
            ))
            .unwrap();
        assert!(plan.executable);
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.python.anndata_metadata")
        );
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .and_then(|adapter| adapter.optional_dependency.as_deref()),
            None
        );
    }

    #[test]
    fn parsed_alignment_manifest_selects_the_dependency_free_record_auditor() {
        let plan = AdapterRegistry::default()
            .plan(request(
                Some("application/alignment-manifest"),
                SourceKind::Bytes,
            ))
            .unwrap();
        assert!(plan.executable);
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.python.alignment_metadata")
        );
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .and_then(|adapter| adapter.optional_dependency.as_deref()),
            None
        );
    }

    #[test]
    fn bounded_fastq_selects_the_dependency_free_python_reader() {
        let plan = AdapterRegistry::default()
            .plan(request(Some("text/fastq"), SourceKind::Bytes))
            .unwrap();
        assert!(plan.executable);
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.python.fastq_text")
        );
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .and_then(|adapter| adapter.optional_dependency.as_deref()),
            None
        );
    }

    #[test]
    fn bounded_mzml_selects_the_dependency_free_python_reader() {
        let plan = AdapterRegistry::default()
            .plan(request(Some("application/mzml"), SourceKind::Bytes))
            .unwrap();
        assert!(plan.executable);
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.python.mzml_text")
        );
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .and_then(|adapter| adapter.optional_dependency.as_deref()),
            None
        );
    }

    #[test]
    fn bounded_fasta_selects_the_dependency_free_python_reader() {
        let plan = AdapterRegistry::default()
            .plan(request(Some("text/fasta"), SourceKind::Bytes))
            .unwrap();
        assert!(plan.executable);
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.python.fasta_text")
        );
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .and_then(|adapter| adapter.optional_dependency.as_deref()),
            None
        );
    }

    #[test]
    fn bounded_gff3_selects_the_dependency_free_python_reader() {
        let plan = AdapterRegistry::default()
            .plan(request(Some("text/gff3"), SourceKind::Bytes))
            .unwrap();
        assert!(plan.executable);
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.python.gff3_text")
        );
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .and_then(|adapter| adapter.optional_dependency.as_deref()),
            None
        );
    }

    #[test]
    fn bounded_pdb_selects_the_dependency_free_python_reader() {
        let plan = AdapterRegistry::default()
            .plan(request(Some("chemical/x-pdb"), SourceKind::Bytes))
            .unwrap();
        assert!(plan.executable);
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.python.pdb_text")
        );
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .and_then(|adapter| adapter.optional_dependency.as_deref()),
            None
        );
    }

    #[test]
    fn fhir_routes_select_dependency_free_structural_auditors() {
        let registry = AdapterRegistry::default();
        for (format, expected) in [
            ("application/fhir+json", "bioprism.python.fhir_json"),
            ("application/fhir-manifest", "bioprism.python.fhir_manifest"),
            ("application/fhir+ndjson", "bioprism.python.fhir_ndjson"),
        ] {
            let plan = registry
                .plan(request(Some(format), SourceKind::Bytes))
                .unwrap();
            assert!(plan.executable, "{format} should be executable");
            assert_eq!(
                plan.selected_adapter
                    .as_ref()
                    .map(|adapter| adapter.id.as_str()),
                Some(expected)
            );
            assert_eq!(
                plan.selected_adapter
                    .as_ref()
                    .and_then(|adapter| adapter.optional_dependency.as_deref()),
                None
            );
        }
    }

    #[test]
    fn unsupported_format_refuses_without_content_sniffing() {
        let plan = AdapterRegistry::default()
            .plan(request(Some("application/octet-stream"), SourceKind::Bytes))
            .unwrap();
        assert!(!plan.executable);
        assert!(plan
            .candidates
            .iter()
            .all(|candidate| candidate.status == PlanStatus::UnsupportedFormat));
    }

    #[test]
    fn omitted_format_still_allows_the_explicitly_undeclared_native_adapters() {
        let plan = AdapterRegistry::default()
            .plan(request(None, SourceKind::Bytes))
            .unwrap();
        assert!(plan.executable);
        assert_eq!(
            plan.selected_adapter
                .as_ref()
                .map(|adapter| adapter.id.as_str()),
            Some("bioprism.tabular")
        );
    }
}
