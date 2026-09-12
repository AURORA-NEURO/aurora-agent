//! Composed, de-identification-first DICOM evidence workflow.
//!
//! This module is the narrow bridge between the DICOM metadata importer and the existing
//! source-bound research workers. It does not inspect pixels or create a clinical interpretation.
//! Instead, it projects one caller-sanitized DICOM JSON document, binds the resulting digest-only
//! asset report into evidence synthesis/program/acquisition, and returns the three reports plus a
//! restart-safe acquisition checkpoint in one replayable envelope.

use crate::case_dicom::{DicomCaseImport, DicomCaseImportReport};
use crate::evidence_acquisition::{
    EvidenceAcquisitionQuery, EvidenceAcquisitionReport, EvidenceAcquisitionSession,
};
use crate::evidence_program::{EvidenceProgramQuery, EvidenceProgramReport};
use crate::evidence_synthesis::{EvidenceSynthesisQuery, EvidenceSynthesisReport};
use crate::public_literature_reasoning_context::{
    PublicLiteratureReasoningContextQuery, PublicLiteratureReasoningContextReport,
};
use crate::real_data_reasoning_context::{
    RealDataReasoningContextQuery, RealDataReasoningContextReport,
};
use crate::{
    CaseRequest, NeurosurgeryError, PublicLiteratureBundle, PublicLiteratureQuery,
    RealDataFreshnessQuery, RealDataQuery, RealGliomaBundle, Specialty,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CASE_DICOM_EVIDENCE_WORKFLOW_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-case-dicom-evidence-workflow/0.1";

/// Bounded controls for one DICOM-to-evidence composition. Nested query objects are kept
/// explicit so a persisted envelope can be replayed without hidden defaults or worker state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DicomEvidenceWorkflowQuery {
    #[serde(default)]
    pub real_data_query: Option<RealDataQuery>,
    #[serde(default)]
    pub public_literature_query: Option<PublicLiteratureQuery>,
    #[serde(default)]
    pub freshness: Option<RealDataFreshnessQuery>,
    #[serde(default = "default_max_program_tracks")]
    pub max_program_tracks_per_lane: usize,
    #[serde(default = "default_max_program_references")]
    pub max_program_references_per_track: usize,
    #[serde(default = "default_max_acquisition_steps")]
    pub max_acquisition_steps: usize,
    #[serde(default = "default_max_acquisition_references")]
    pub max_acquisition_references_per_step: usize,
    #[serde(default = "default_max_synthesis_references")]
    pub max_synthesis_references: usize,
    #[serde(default)]
    pub include_source_text: bool,
    /// Optional bounded source-addressable context for a caller-owned local model or reviewer.
    /// The context is generated only from the already validated snapshot and never invokes a
    /// provider; it remains separate from the DICOM metadata plane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_reasoning_context: Option<RealDataReasoningContextQuery>,
    /// Optional PubMed context for non-glioma lanes or supplemental glioma review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_reasoning_context: Option<PublicLiteratureReasoningContextQuery>,
}

const fn default_max_program_tracks() -> usize {
    6
}
const fn default_max_program_references() -> usize {
    8
}
const fn default_max_acquisition_steps() -> usize {
    16
}
const fn default_max_acquisition_references() -> usize {
    4
}
const fn default_max_synthesis_references() -> usize {
    64
}

impl Default for DicomEvidenceWorkflowQuery {
    fn default() -> Self {
        Self {
            real_data_query: None,
            public_literature_query: None,
            freshness: None,
            max_program_tracks_per_lane: default_max_program_tracks(),
            max_program_references_per_track: default_max_program_references(),
            max_acquisition_steps: default_max_acquisition_steps(),
            max_acquisition_references_per_step: default_max_acquisition_references(),
            max_synthesis_references: default_max_synthesis_references(),
            include_source_text: false,
            real_data_reasoning_context: None,
            public_literature_reasoning_context: None,
        }
    }
}

/// The workflow always terminates at a human-review handoff. This status is intentionally not a
/// clinical readiness or model-confidence state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DicomEvidenceWorkflowStatus {
    ReadyForHumanReview,
}

/// Digest-bound output of one DICOM metadata projection plus the existing evidence workers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DicomEvidenceWorkflowReport {
    pub schema_version: String,
    pub workflow_digest: String,
    pub request_digest: String,
    pub specialty: Specialty,
    pub query: DicomEvidenceWorkflowQuery,
    pub dicom_import: DicomCaseImportReport,
    pub evidence_synthesis: EvidenceSynthesisReport,
    pub evidence_program: EvidenceProgramReport,
    pub evidence_acquisition: EvidenceAcquisitionReport,
    pub evidence_acquisition_session: EvidenceAcquisitionSession,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_reasoning_context: Option<RealDataReasoningContextReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_reasoning_context: Option<PublicLiteratureReasoningContextReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_digest: Option<String>,
    pub status: DicomEvidenceWorkflowStatus,
    pub human_review_required: bool,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl DicomEvidenceWorkflowReport {
    /// Validate envelope invariants without fetching sources, opening assets, or accepting a
    /// report that was rebound to another request or snapshot.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != CASE_DICOM_EVIDENCE_WORKFLOW_SCHEMA_VERSION
            || !is_sha256_hex(&self.workflow_digest)
            || !is_sha256_hex(&self.request_digest)
            || self.dicom_import.request_digest != self.request_digest
            || self.dicom_import.specialty != self.specialty
            || self.evidence_synthesis.request_digest != self.request_digest
            || self.evidence_program.request_digest != self.request_digest
            || self.evidence_acquisition.request_digest != self.request_digest
            || self.evidence_acquisition.specialty != self.specialty
            || self.evidence_acquisition_session.request_digest != self.request_digest
            || self.evidence_acquisition_session.specialty != self.specialty
            || self.evidence_acquisition_session.plan_digest
                != self.evidence_acquisition.plan_digest
            || self
                .evidence_acquisition_session
                .case_asset_report_digest
                .as_deref()
                != Some(self.dicom_import.manifest_report.report_digest.as_str())
            || self
                .evidence_acquisition
                .case_asset_report_digest
                .as_deref()
                != Some(self.dicom_import.manifest_report.report_digest.as_str())
            || self
                .evidence_synthesis
                .case_asset_summary
                .as_ref()
                .map(|summary| summary.report_digest.as_str())
                != Some(self.dicom_import.manifest_report.report_digest.as_str())
            || !self
                .evidence_program
                .lanes
                .iter()
                .flat_map(|lane| lane.tracks.iter())
                .all(|track| {
                    track.asset_coverage.is_some()
                        || self.dicom_import.manifest_report.assets.is_empty()
                })
            || self.status != DicomEvidenceWorkflowStatus::ReadyForHumanReview
            || !self.human_review_required
            || !self.provenance_bound
            || self.synthetic_data
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
            || self
                .real_data_digest
                .as_deref()
                .is_some_and(|digest| !is_sha256_hex(digest))
            || self
                .public_literature_digest
                .as_deref()
                .is_some_and(|digest| !is_sha256_hex(digest))
            || self.real_data_reasoning_context.is_some()
                != self.query.real_data_reasoning_context.is_some()
            || self.public_literature_reasoning_context.is_some()
                != self.query.public_literature_reasoning_context.is_some()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "DICOM evidence workflow envelope is invalid".to_string(),
            });
        }
        // The top-level digest binds nested JSON bytes, but each worker also owns a stronger
        // semantic validator (source closure, bounds, and internal digest). Re-run those
        // validators here so a caller cannot make a structurally re-digested envelope appear
        // valid after mutating one nested worker report.
        self.dicom_import.validate_integrity()?;
        self.evidence_synthesis.validate_integrity()?;
        self.evidence_program.validate_integrity()?;
        if let Some(context) = self.real_data_reasoning_context.as_ref() {
            context.validate_integrity()?;
            if self.real_data_digest.as_deref() != Some(context.bundle_digest.as_str()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "DICOM workflow real-data context is not bound to its bundle"
                        .to_string(),
                });
            }
        }
        if let Some(context) = self.public_literature_reasoning_context.as_ref() {
            context.validate_integrity()?;
            if self.public_literature_digest.as_deref() != Some(context.bundle_digest.as_str()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "DICOM workflow literature context is not bound to its bundle"
                        .to_string(),
                });
            }
        }
        if self.workflow_digest != digest_report(self)? {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "DICOM evidence workflow digest does not match its contents".to_string(),
            });
        }
        Ok(())
    }

    /// Rebuild the complete workflow from exact caller inputs. This is the replay gate used by
    /// the CLI, MCP, and SDK callers before persisting or forwarding an envelope.
    pub fn validate_for_inputs(
        &self,
        request: &CaseRequest,
        import: &DicomCaseImport,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = run(request, import, real_data, public_literature, &self.query)?;
        if self != &expected {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "DICOM evidence workflow does not replay to exact request and sources"
                    .to_string(),
            });
        }
        Ok(())
    }
}

/// Compose a DICOM projection with source-bound synthesis, review programming, and a resumable
/// acquisition plan. The two public evidence planes remain independent and optional according to
/// specialty: glioma requires real glioma data, while other lanes require PubMed literature.
pub fn run(
    request: &CaseRequest,
    import: &DicomCaseImport,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    query: &DicomEvidenceWorkflowQuery,
) -> Result<DicomEvidenceWorkflowReport, NeurosurgeryError> {
    validate_query(query)?;
    if request.specialty == Specialty::Glioma && real_data.is_none() {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "glioma DICOM evidence workflows require a validated real-data bundle"
                .to_string(),
        });
    }
    if request.specialty != Specialty::Glioma && public_literature.is_none() {
        return Err(NeurosurgeryError::RealDataRejected {
            reason:
                "non-glioma DICOM evidence workflows require a validated public-literature bundle"
                    .to_string(),
        });
    }
    let dicom_import = import.project(request)?;
    let request_digest = dicom_import.request_digest.clone();
    let mut synthesis_query = EvidenceSynthesisQuery {
        real_data_query: query.real_data_query.clone(),
        public_literature_query: query.public_literature_query.clone(),
        freshness: query.freshness.clone(),
        max_references: query.max_synthesis_references,
        include_source_text: query.include_source_text,
    };
    if let Some(public_query) = synthesis_query.public_literature_query.as_mut() {
        if public_query.specialty.is_none() {
            public_query.specialty = Some(request.specialty);
        }
    }
    let evidence_synthesis = crate::evidence_synthesis::synthesize_with_case_assets(
        request,
        real_data,
        public_literature,
        &synthesis_query,
        Some(&dicom_import.manifest_report),
    )?;
    let evidence_program = crate::evidence_program::build_evidence_program_with_asset_report(
        request,
        real_data,
        public_literature,
        Some(&dicom_import.manifest_report),
        &EvidenceProgramQuery {
            max_tracks_per_lane: query.max_program_tracks_per_lane,
            max_references_per_track: query.max_program_references_per_track,
            freshness: query.freshness.clone(),
            ..EvidenceProgramQuery::default()
        },
    )?;
    let acquisition_start = crate::evidence_acquisition::start_with_case_assets(
        request,
        real_data,
        public_literature,
        Some(&dicom_import.manifest_report),
        &EvidenceAcquisitionQuery {
            max_steps: query.max_acquisition_steps,
            max_references_per_step: query.max_acquisition_references_per_step,
            freshness: query.freshness.clone(),
        },
    )?;
    let real_data_digest = real_data
        .map(|data| data.summary().map(|summary| summary.bundle_digest))
        .transpose()?;
    let public_literature_digest = public_literature
        .map(|data| data.summary().map(|summary| summary.bundle_digest))
        .transpose()?;
    let real_data_reasoning_context = match (real_data, query.real_data_reasoning_context.as_ref())
    {
        (Some(data), Some(context_query)) => Some(data.reasoning_context(context_query)?),
        (None, Some(_)) => {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data reasoning context requires a validated real-data bundle"
                    .to_string(),
            })
        }
        _ => None,
    };
    let public_literature_reasoning_context = match (
        public_literature,
        query.public_literature_reasoning_context.as_ref(),
    ) {
        (Some(data), Some(context_query)) => Some(data.reasoning_context(context_query)?),
        (None, Some(_)) => {
            return Err(NeurosurgeryError::RealDataRejected {
                reason:
                    "public-literature reasoning context requires a validated literature bundle"
                        .to_string(),
            })
        }
        _ => None,
    };
    let mut report = DicomEvidenceWorkflowReport {
        schema_version: CASE_DICOM_EVIDENCE_WORKFLOW_SCHEMA_VERSION.to_string(),
        workflow_digest: String::new(),
        request_digest,
        specialty: request.specialty,
        query: query.clone(),
        dicom_import,
        evidence_synthesis,
        evidence_program,
        evidence_acquisition: acquisition_start.plan,
        evidence_acquisition_session: acquisition_start.session,
        real_data_reasoning_context,
        public_literature_reasoning_context,
        real_data_digest,
        public_literature_digest,
        status: DicomEvidenceWorkflowStatus::ReadyForHumanReview,
        human_review_required: true,
        provenance_bound: true,
        synthetic_data: false,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: vec![
            "DICOM JSON is projected as de-identified series metadata only; pixel bytes, private tags, identifiers, and clinical image interpretation never enter the workflow".to_string(),
            "source bundles are caller-supplied validated public records; the workflow never fetches URLs, invokes a model, or turns population/citation metadata into a patient finding".to_string(),
            "synthesis, program tracks, and acquisition steps are reviewer work products; the acquisition checkpoint is restart-safe but does not schedule, authorize, or execute an external action".to_string(),
        ],
    };
    report.workflow_digest = digest_report(&report)?;
    report.validate_integrity()?;
    Ok(report)
}

fn validate_query(query: &DicomEvidenceWorkflowQuery) -> Result<(), NeurosurgeryError> {
    if !(1..=crate::evidence_program::MAX_EVIDENCE_PROGRAM_TRACKS_PER_LANE)
        .contains(&query.max_program_tracks_per_lane)
    {
        return Err(NeurosurgeryError::TooMany {
            field: "case_dicom_evidence_workflow.max_program_tracks_per_lane",
            found: query.max_program_tracks_per_lane,
            max: crate::evidence_program::MAX_EVIDENCE_PROGRAM_TRACKS_PER_LANE,
        });
    }
    if !(1..=crate::evidence_program::MAX_EVIDENCE_PROGRAM_REFERENCES_PER_TRACK)
        .contains(&query.max_program_references_per_track)
    {
        return Err(NeurosurgeryError::TooMany {
            field: "case_dicom_evidence_workflow.max_program_references_per_track",
            found: query.max_program_references_per_track,
            max: crate::evidence_program::MAX_EVIDENCE_PROGRAM_REFERENCES_PER_TRACK,
        });
    }
    if !(1..=crate::evidence_acquisition::MAX_EVIDENCE_ACQUISITION_STEPS)
        .contains(&query.max_acquisition_steps)
    {
        return Err(NeurosurgeryError::TooMany {
            field: "case_dicom_evidence_workflow.max_acquisition_steps",
            found: query.max_acquisition_steps,
            max: crate::evidence_acquisition::MAX_EVIDENCE_ACQUISITION_STEPS,
        });
    }
    if !(1..=crate::evidence_acquisition::MAX_EVIDENCE_ACQUISITION_REFERENCES)
        .contains(&query.max_acquisition_references_per_step)
    {
        return Err(NeurosurgeryError::TooMany {
            field: "case_dicom_evidence_workflow.max_acquisition_references_per_step",
            found: query.max_acquisition_references_per_step,
            max: crate::evidence_acquisition::MAX_EVIDENCE_ACQUISITION_REFERENCES,
        });
    }
    if !(1..=crate::evidence_synthesis::MAX_EVIDENCE_SYNTHESIS_REFERENCES)
        .contains(&query.max_synthesis_references)
    {
        return Err(NeurosurgeryError::TooMany {
            field: "case_dicom_evidence_workflow.max_synthesis_references",
            found: query.max_synthesis_references,
            max: crate::evidence_synthesis::MAX_EVIDENCE_SYNTHESIS_REFERENCES,
        });
    }
    Ok(())
}

fn digest_report(report: &DicomEvidenceWorkflowReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.workflow_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}
