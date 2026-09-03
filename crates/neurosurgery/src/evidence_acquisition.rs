//! Deterministic, source-aware acquisition planning for the provider-free neurosurgical agent.
//!
//! This module turns explicit intake missingness into a bounded set of replayable local queries
//! over caller-supplied, validated public bundles. It never fetches a source, opens a credential,
//! reads a case asset, or promotes a population/citation match to a patient finding. A caller can
//! use the emitted queries as the next autonomous worker wave and then re-run the same request
//! after refreshing a snapshot through the repository's network-isolated refresh boundary.

use crate::case_asset_manifest::{CaseAssetManifestReport, CaseAssetReviewItem};
use crate::real_data_freshness::{RealDataFreshnessQuery, RealDataFreshnessReport};
use crate::research_plan::{ResearchPlanReference, ResearchPlanSource};
use crate::{
    audit as audit_evidence, CaseRequest, EvidenceAuditReport, EvidenceState, NeurosurgeryError,
    ObservationKind, PublicLiteratureBundle, PublicLiteratureQuery, RealDataQuery,
    RealDataRecordKind, RealGliomaBundle, Specialty,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const EVIDENCE_ACQUISITION_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-evidence-acquisition/0.1";
pub const EVIDENCE_ACQUISITION_SESSION_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-evidence-acquisition-session/0.1";
pub const EVIDENCE_ACQUISITION_EXECUTION_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-evidence-acquisition-execution/0.1";
pub const MAX_EVIDENCE_ACQUISITION_STEPS: usize = 64;
pub const MAX_EVIDENCE_ACQUISITION_REFERENCES: usize = 16;
pub const MAX_EVIDENCE_ACQUISITION_ADVANCE_STEPS: usize = 16;

fn default_max_steps() -> usize {
    16
}

fn default_max_references_per_step() -> usize {
    4
}

/// Query bounds for one autonomous acquisition wave. The caller owns any later network refresh;
/// this query only projects deterministic work over already validated local snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionQuery {
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,
    #[serde(default = "default_max_references_per_step")]
    pub max_references_per_step: usize,
    #[serde(default)]
    pub freshness: Option<RealDataFreshnessQuery>,
}

impl Default for EvidenceAcquisitionQuery {
    fn default() -> Self {
        Self {
            max_steps: default_max_steps(),
            max_references_per_step: default_max_references_per_step(),
            freshness: None,
        }
    }
}

/// Why the worker emitted one bounded acquisition step. These are evidence-state triggers, not
/// clinical priorities or severity scores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAcquisitionTrigger {
    MissingObservation,
    UninterpretableObservation,
    ConflictingObservation,
    MissingProvenance,
    MissingEvidenceRecord,
    BaselineSpecialtyCoverage,
}

/// A source query that can be replayed against one caller-supplied validated snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", content = "query", rename_all = "snake_case")]
pub enum EvidenceAcquisitionSourceQuery {
    /// Box the larger real-glioma query so adding bounded facets does not inflate every
    /// acquisition step or trigger a large-enum regression in downstream binaries.
    RealGliomaPopulation(Box<RealDataQuery>),
    PublicLiterature(PublicLiteratureQuery),
}

/// Explicit local-query outcome. A zero-match result never becomes negative evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAcquisitionStepStatus {
    CandidatesFound,
    NoLocalMatches,
    Truncated,
}

/// One deterministic, source-linked acquisition/review step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionStep {
    pub sequence: u16,
    pub step_id: String,
    pub source: ResearchPlanSource,
    pub trigger: EvidenceAcquisitionTrigger,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_kind: Option<ObservationKind>,
    pub query: EvidenceAcquisitionSourceQuery,
    /// True when a controlled observation term returned no local rows and the bounded query was
    /// widened to the specialty facet. The caller can distinguish that recovery from a direct
    /// unfiltered scan.
    pub fallback_to_specialty_scan: bool,
    pub status: EvidenceAcquisitionStepStatus,
    pub total_matches: usize,
    pub returned_matches: usize,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<ResearchPlanReference>,
}

/// Digest-bound autonomous acquisition/review wave. It is a caller-owned worklist, never a
/// network scheduler or clinical action plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionReport {
    pub schema_version: String,
    pub plan_digest: String,
    pub request_digest: String,
    pub specialty: Specialty,
    pub query: EvidenceAcquisitionQuery,
    pub audit: EvidenceAuditReport,
    pub steps: Vec<EvidenceAcquisitionStep>,
    pub candidate_step_count: usize,
    pub omitted_step_count: usize,
    pub truncated: bool,
    pub source_query_count: usize,
    pub source_candidate_count: usize,
    pub required_sources: Vec<ResearchPlanSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_freshness: Option<RealDataFreshnessReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_freshness: Option<RealDataFreshnessReport>,
    /// Digest/count projection of the persisted case-asset review ledger. Optional for
    /// compatibility with plans compiled before a disposition ledger was supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_disposition_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_pending_item_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_resolved_decision_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_unresolved_decision_count: Option<usize>,
    /// Digest-only projection of caller-owned multimodal asset review work. Asset bytes are
    /// never opened by the acquisition worker; the digest binds this worklist to the manifest
    /// projection used by the enclosing mission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_report_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub case_asset_review_items: Vec<CaseAssetReviewItem>,
    #[serde(default)]
    pub case_asset_omitted_review_item_count: usize,
    #[serde(default)]
    pub case_asset_review_truncated: bool,
    pub ready_for_local_replay: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

/// Lifecycle state for a caller-persisted acquisition worker. The state contains only digests and
/// bounded event metadata; source records and any case assets remain caller-owned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAcquisitionSessionStatus {
    Planned,
    Running,
    NeedsEvidence,
    AwaitingHumanReview,
}

/// One digest-only execution event in an acquisition checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionEvent {
    pub ordinal: u16,
    pub sequence: u16,
    pub step_id: String,
    pub source: ResearchPlanSource,
    pub status: EvidenceAcquisitionStepStatus,
    pub total_matches: usize,
    pub returned_matches: usize,
    pub truncated: bool,
    pub reference_digest: String,
    pub previous_event_digest: String,
    pub event_digest: String,
}

/// Checkpoint for a bounded, replayable local acquisition worker. Re-supplying the request and
/// unchanged validated snapshots is required to advance it; no hidden server state is used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionSession {
    pub schema_version: String,
    pub session_id: String,
    pub plan_digest: String,
    pub request_digest: String,
    pub specialty: Specialty,
    #[serde(default)]
    pub real_data_digest: Option<String>,
    #[serde(default)]
    pub public_literature_digest: Option<String>,
    #[serde(default)]
    pub case_asset_report_digest: Option<String>,
    #[serde(default)]
    pub case_asset_review_disposition_digest: Option<String>,
    pub next_sequence: u16,
    pub status: EvidenceAcquisitionSessionStatus,
    pub event_chain_digest: String,
    pub events: Vec<EvidenceAcquisitionEvent>,
}

/// Transient result for one or more replayed local acquisition steps. It is intentionally not
/// persisted in the checkpoint because references are caller-owned output, not worker state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionExecutionStep {
    pub sequence: u16,
    pub step_id: String,
    pub source: ResearchPlanSource,
    pub status: EvidenceAcquisitionStepStatus,
    pub total_matches: usize,
    pub returned_matches: usize,
    pub truncated: bool,
    pub references: Vec<ResearchPlanReference>,
}

/// Result of one bounded checkpoint advance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionAdvanceResult {
    pub schema_version: String,
    pub session: EvidenceAcquisitionSession,
    pub steps: Vec<EvidenceAcquisitionExecutionStep>,
    pub steps_executed: usize,
    pub complete: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

/// Final caller-owned execution receipt after every planned step has been replayed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionExecutionReport {
    pub schema_version: String,
    pub plan_digest: String,
    pub request_digest: String,
    pub specialty: Specialty,
    pub steps_executed: usize,
    pub event_count: usize,
    pub event_chain_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_report_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_disposition_digest: Option<String>,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

/// Start result: the caller receives both the immutable plan and its empty checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionStartResult {
    pub schema_version: String,
    pub plan: EvidenceAcquisitionReport,
    pub session: EvidenceAcquisitionSession,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
}

#[derive(Debug, Clone, Copy)]
struct AcquisitionSeed {
    trigger: EvidenceAcquisitionTrigger,
    observation_kind: Option<ObservationKind>,
}

type AcquisitionQueryResult = (
    EvidenceAcquisitionSourceQuery,
    bool,
    usize,
    usize,
    bool,
    Vec<ResearchPlanReference>,
);

/// Compile a bounded acquisition wave from the request audit and any validated public bundles.
/// Both evidence planes may be supplied; they remain independent and retain separate digests.
pub fn compile(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    query: &EvidenceAcquisitionQuery,
) -> Result<EvidenceAcquisitionReport, NeurosurgeryError> {
    compile_with_case_assets(request, real_data, public_literature, None, query)
}

/// Compile a bounded acquisition wave while carrying a digest-only projection of the caller's
/// multimodal asset review obligations. The projection is deliberately separate from source
/// queries: it tells a local worker which metadata must be reconciled, never how to read or
/// interpret imaging, pathology, molecular, operative, or other patient files.
pub fn compile_with_case_assets(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    case_asset_report: Option<&CaseAssetManifestReport>,
    query: &EvidenceAcquisitionQuery,
) -> Result<EvidenceAcquisitionReport, NeurosurgeryError> {
    if !(1..=MAX_EVIDENCE_ACQUISITION_STEPS).contains(&query.max_steps) {
        return Err(NeurosurgeryError::TooMany {
            field: "evidence_acquisition.max_steps",
            found: query.max_steps,
            max: MAX_EVIDENCE_ACQUISITION_STEPS,
        });
    }
    if !(1..=MAX_EVIDENCE_ACQUISITION_REFERENCES).contains(&query.max_references_per_step) {
        return Err(NeurosurgeryError::TooMany {
            field: "evidence_acquisition.max_references_per_step",
            found: query.max_references_per_step,
            max: MAX_EVIDENCE_ACQUISITION_REFERENCES,
        });
    }
    if real_data.is_some() && request.specialty != Specialty::Glioma {
        return Err(NeurosurgeryError::RealDataSpecialtyUnsupported {
            specialty: request.specialty,
        });
    }

    let audit = audit_evidence(request)?;
    // Validation is deliberately performed before any query projection, so a malformed or
    // synthetic bundle cannot be mistaken for an empty local source.
    let real_data_digest = real_data
        .map(|data| data.summary().map(|summary| summary.bundle_digest))
        .transpose()?;
    let public_literature_digest = public_literature
        .map(|literature| literature.summary().map(|summary| summary.bundle_digest))
        .transpose()?;
    let real_data_freshness = real_data
        .zip(query.freshness.as_ref())
        .map(|(data, freshness)| data.freshness_report(freshness))
        .transpose()?;
    let public_literature_freshness = public_literature
        .zip(query.freshness.as_ref())
        .map(|(literature, freshness)| literature.freshness_report(freshness))
        .transpose()?;
    let request_digest = digest_request(request)?;
    validate_case_asset_report(case_asset_report, request, &request_digest)?;

    let mut seeds = audit
        .items
        .iter()
        .filter_map(|item| {
            let trigger = match item.state {
                EvidenceState::Unmeasured => EvidenceAcquisitionTrigger::MissingObservation,
                EvidenceState::Uninterpretable => {
                    EvidenceAcquisitionTrigger::UninterpretableObservation
                }
                EvidenceState::Conflicting => EvidenceAcquisitionTrigger::ConflictingObservation,
                EvidenceState::Measured if item.provenance_complete_count < item.observed_count => {
                    EvidenceAcquisitionTrigger::MissingProvenance
                }
                EvidenceState::Measured => return None,
            };
            Some(AcquisitionSeed {
                trigger,
                observation_kind: Some(item.observation_kind),
            })
        })
        .collect::<Vec<_>>();
    if audit.evidence_record_count == 0 || audit.evidence_supporting_synthesis_count == 0 {
        seeds.push(AcquisitionSeed {
            trigger: EvidenceAcquisitionTrigger::MissingEvidenceRecord,
            observation_kind: None,
        });
    }
    // A baseline scan remains useful even when the caller supplied complete observations: it
    // gives a local worker a deterministic source checkpoint to compare after refresh.
    seeds.push(AcquisitionSeed {
        trigger: EvidenceAcquisitionTrigger::BaselineSpecialtyCoverage,
        observation_kind: None,
    });

    let source_count = real_data.is_some() as usize + public_literature.is_some() as usize;
    let candidate_step_count = seeds.len().saturating_mul(source_count);
    let omitted_step_count = candidate_step_count.saturating_sub(query.max_steps);
    let truncated = omitted_step_count > 0;
    let mut steps = Vec::new();
    let mut source_query_count = 0usize;
    let mut source_candidate_count = 0usize;
    for seed in seeds {
        for source in [
            real_data.map(|_| ResearchPlanSource::RealGliomaPopulation),
            public_literature.map(|_| ResearchPlanSource::PublicLiterature),
        ]
        .into_iter()
        .flatten()
        {
            if steps.len() >= query.max_steps {
                break;
            }
            let (source_query, fallback, total_matches, returned_matches, source_truncated, refs) =
                execute_source_query(
                    source,
                    request.specialty,
                    seed.observation_kind,
                    real_data,
                    public_literature,
                    query.max_references_per_step,
                )?;
            source_query_count = source_query_count.saturating_add(1);
            source_candidate_count = source_candidate_count.saturating_add(total_matches);
            let status = if source_truncated {
                EvidenceAcquisitionStepStatus::Truncated
            } else if total_matches == 0 {
                EvidenceAcquisitionStepStatus::NoLocalMatches
            } else {
                EvidenceAcquisitionStepStatus::CandidatesFound
            };
            let sequence =
                u16::try_from(steps.len() + 1).map_err(|_| NeurosurgeryError::TooMany {
                    field: "evidence_acquisition.steps",
                    found: steps.len() + 1,
                    max: u16::MAX as usize,
                })?;
            let step_id = digest_step(sequence, source, seed.trigger, seed.observation_kind);
            steps.push(EvidenceAcquisitionStep {
                sequence,
                step_id,
                source,
                trigger: seed.trigger,
                observation_kind: seed.observation_kind,
                query: source_query,
                fallback_to_specialty_scan: fallback,
                status,
                total_matches,
                returned_matches,
                truncated: source_truncated,
                references: refs,
            });
        }
    }

    let mut required_sources = Vec::new();
    if request.specialty == Specialty::Glioma && real_data.is_none() {
        required_sources.push(ResearchPlanSource::RealGliomaPopulation);
    }
    if public_literature.is_none() {
        required_sources.push(ResearchPlanSource::PublicLiterature);
    }
    let mut report = EvidenceAcquisitionReport {
        schema_version: EVIDENCE_ACQUISITION_SCHEMA_VERSION.to_string(),
        plan_digest: String::new(),
        request_digest,
        specialty: request.specialty,
        query: query.clone(),
        audit,
        steps,
        candidate_step_count,
        omitted_step_count,
        truncated,
        source_query_count,
        source_candidate_count,
        required_sources,
        real_data_digest,
        public_literature_digest,
        real_data_freshness,
        public_literature_freshness,
        case_asset_review_disposition_digest: None,
        case_asset_review_pending_item_count: None,
        case_asset_review_resolved_decision_count: None,
        case_asset_review_unresolved_decision_count: None,
        case_asset_report_digest: case_asset_report.map(|report| report.report_digest.clone()),
        case_asset_review_items: case_asset_report
            .map(|report| report.review_items.clone())
            .unwrap_or_default(),
        case_asset_omitted_review_item_count: case_asset_report
            .map(|report| report.omitted_review_item_count)
            .unwrap_or_default(),
        case_asset_review_truncated: case_asset_report
            .map(|report| report.truncated)
            .unwrap_or(false),
        ready_for_local_replay: source_count > 0,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: vec![
            "steps are caller-owned local research/review queries and never clinical instructions".to_string(),
            "population and citation matches remain separate from patient observations and do not establish diagnosis, prognosis, treatment, triage, urgency, or procedure".to_string(),
            "no-local-match is not negative evidence; it only describes the supplied validated snapshot".to_string(),
            "fallback_to_specialty_scan means a controlled observation term had no local rows and the bounded specialty facet was queried".to_string(),
            "the worker never fetches URLs, invokes a provider, opens credentials or case-asset bytes, or writes durable state".to_string(),
        ],
    };
    report.plan_digest = digest_report(&report)?;
    Ok(report)
}

/// Compile a plan while binding the exact persisted case-asset review ledger used by the caller.
/// Manifest digest and projection counts are checked before the plan is emitted, preventing a
/// stale ledger from changing an autonomous worker's obligations.
pub fn compile_with_case_assets_and_dispositions(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    case_asset_report: Option<&CaseAssetManifestReport>,
    dispositions: &crate::CaseAssetReviewDispositionReport,
    query: &EvidenceAcquisitionQuery,
) -> Result<EvidenceAcquisitionReport, NeurosurgeryError> {
    let asset_report = case_asset_report.ok_or_else(|| NeurosurgeryError::SessionRejected {
        reason: "case-asset dispositions require a projected case-asset manifest".to_string(),
    })?;
    dispositions.validate_integrity()?;
    let candidate_item_count = asset_report
        .review_items
        .len()
        .checked_add(asset_report.omitted_review_item_count)
        .ok_or_else(|| NeurosurgeryError::RealDataRejected {
            reason: "case-asset review candidate count overflows its bound".to_string(),
        })?;
    if dispositions.report_digest != asset_report.report_digest
        || dispositions.returned_item_count != asset_report.review_items.len()
        || dispositions.omitted_item_count != asset_report.omitted_review_item_count
        || dispositions.candidate_item_count != candidate_item_count
    {
        return Err(NeurosurgeryError::SessionRejected {
            reason: "disposition ledger does not bind to the supplied case-asset projection"
                .to_string(),
        });
    }
    let mut report = compile_with_case_assets(
        request,
        real_data,
        public_literature,
        Some(asset_report),
        query,
    )?;
    report.case_asset_review_disposition_digest = Some(dispositions.disposition_digest.clone());
    report.case_asset_review_pending_item_count = Some(dispositions.pending_item_count);
    report.case_asset_review_resolved_decision_count = Some(dispositions.resolved_decision_count);
    report.case_asset_review_unresolved_decision_count =
        Some(dispositions.unresolved_decision_count);
    report.plan_digest = digest_report(&report)?;
    Ok(report)
}

fn validate_case_asset_report(
    report: Option<&CaseAssetManifestReport>,
    request: &CaseRequest,
    request_digest: &str,
) -> Result<(), NeurosurgeryError> {
    let Some(report) = report else {
        return Ok(());
    };
    report.validate_integrity()?;
    if report.request_digest != request_digest || report.specialty != request.specialty {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "case-asset acquisition projection is not bound to the de-identified request"
                .to_string(),
        });
    }
    Ok(())
}

/// Start a digest-bound caller-persisted acquisition worker.
pub fn start(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    query: &EvidenceAcquisitionQuery,
) -> Result<EvidenceAcquisitionStartResult, NeurosurgeryError> {
    start_with_case_assets(request, real_data, public_literature, None, query)
}

/// Start an acquisition worker while binding its checkpoint to a case-asset review projection.
pub fn start_with_case_assets(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    case_asset_report: Option<&CaseAssetManifestReport>,
    query: &EvidenceAcquisitionQuery,
) -> Result<EvidenceAcquisitionStartResult, NeurosurgeryError> {
    let plan = compile_with_case_assets(
        request,
        real_data,
        public_literature,
        case_asset_report,
        query,
    )?;
    let session_id = format!("nsa-session-{}", &plan.plan_digest[..16]);
    let event_chain_digest = digest_value(&(
        session_id.as_str(),
        plan.plan_digest.as_str(),
        plan.request_digest.as_str(),
    ))?;
    let status = if !plan.required_sources.is_empty() {
        EvidenceAcquisitionSessionStatus::NeedsEvidence
    } else {
        EvidenceAcquisitionSessionStatus::Planned
    };
    let session = EvidenceAcquisitionSession {
        schema_version: EVIDENCE_ACQUISITION_SESSION_SCHEMA_VERSION.to_string(),
        session_id,
        plan_digest: plan.plan_digest.clone(),
        request_digest: plan.request_digest.clone(),
        specialty: plan.specialty,
        real_data_digest: plan.real_data_digest.clone(),
        public_literature_digest: plan.public_literature_digest.clone(),
        case_asset_report_digest: plan.case_asset_report_digest.clone(),
        case_asset_review_disposition_digest: plan.case_asset_review_disposition_digest.clone(),
        next_sequence: 1,
        status,
        event_chain_digest,
        events: Vec::new(),
    };
    Ok(EvidenceAcquisitionStartResult {
        schema_version: EVIDENCE_ACQUISITION_SESSION_SCHEMA_VERSION.to_string(),
        plan,
        session,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
    })
}

/// Start a worker whose plan and checkpoint are bound to a persisted case-asset review ledger.
pub fn start_with_case_assets_and_dispositions(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    case_asset_report: Option<&CaseAssetManifestReport>,
    dispositions: &crate::CaseAssetReviewDispositionReport,
    query: &EvidenceAcquisitionQuery,
) -> Result<EvidenceAcquisitionStartResult, NeurosurgeryError> {
    let plan = compile_with_case_assets_and_dispositions(
        request,
        real_data,
        public_literature,
        case_asset_report,
        dispositions,
        query,
    )?;
    let session_id = format!("nsa-session-{}", &plan.plan_digest[..16]);
    let event_chain_digest = digest_value(&(
        session_id.as_str(),
        plan.plan_digest.as_str(),
        plan.request_digest.as_str(),
    ))?;
    let status = if !plan.required_sources.is_empty() {
        EvidenceAcquisitionSessionStatus::NeedsEvidence
    } else {
        EvidenceAcquisitionSessionStatus::Planned
    };
    let session = EvidenceAcquisitionSession {
        schema_version: EVIDENCE_ACQUISITION_SESSION_SCHEMA_VERSION.to_string(),
        session_id,
        plan_digest: plan.plan_digest.clone(),
        request_digest: plan.request_digest.clone(),
        specialty: plan.specialty,
        real_data_digest: plan.real_data_digest.clone(),
        public_literature_digest: plan.public_literature_digest.clone(),
        case_asset_report_digest: plan.case_asset_report_digest.clone(),
        case_asset_review_disposition_digest: plan.case_asset_review_disposition_digest.clone(),
        next_sequence: 1,
        status,
        event_chain_digest,
        events: Vec::new(),
    };
    Ok(EvidenceAcquisitionStartResult {
        schema_version: EVIDENCE_ACQUISITION_SESSION_SCHEMA_VERSION.to_string(),
        plan,
        session,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
    })
}

/// Replay up to `max_steps` plan steps against unchanged validated local snapshots and return the
/// next checkpoint. The default SDK/MCP callers use one step per advance; batching is bounded for
/// queue workers that want a small local wave.
pub fn advance(
    session: &EvidenceAcquisitionSession,
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    query: &EvidenceAcquisitionQuery,
    max_steps: usize,
) -> Result<EvidenceAcquisitionAdvanceResult, NeurosurgeryError> {
    advance_with_case_assets(
        session,
        request,
        real_data,
        public_literature,
        None,
        query,
        max_steps,
    )
}

/// Replay a checkpoint against unchanged snapshots and the same case-asset projection.
pub fn advance_with_case_assets(
    session: &EvidenceAcquisitionSession,
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    case_asset_report: Option<&CaseAssetManifestReport>,
    query: &EvidenceAcquisitionQuery,
    max_steps: usize,
) -> Result<EvidenceAcquisitionAdvanceResult, NeurosurgeryError> {
    let plan = compile_with_case_assets(
        request,
        real_data,
        public_literature,
        case_asset_report,
        query,
    )?;
    advance_with_compiled_plan(session, &plan, real_data, public_literature, max_steps)
}

/// Replay a checkpoint whose plan carries a case-asset review disposition ledger.
#[allow(clippy::too_many_arguments)]
pub fn advance_with_case_assets_and_dispositions(
    session: &EvidenceAcquisitionSession,
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    case_asset_report: Option<&CaseAssetManifestReport>,
    dispositions: &crate::CaseAssetReviewDispositionReport,
    query: &EvidenceAcquisitionQuery,
    max_steps: usize,
) -> Result<EvidenceAcquisitionAdvanceResult, NeurosurgeryError> {
    let plan = compile_with_case_assets_and_dispositions(
        request,
        real_data,
        public_literature,
        case_asset_report,
        dispositions,
        query,
    )?;
    advance_with_compiled_plan(session, &plan, real_data, public_literature, max_steps)
}

fn advance_with_compiled_plan(
    session: &EvidenceAcquisitionSession,
    plan: &EvidenceAcquisitionReport,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    max_steps: usize,
) -> Result<EvidenceAcquisitionAdvanceResult, NeurosurgeryError> {
    if !(1..=MAX_EVIDENCE_ACQUISITION_ADVANCE_STEPS).contains(&max_steps) {
        return Err(NeurosurgeryError::TooMany {
            field: "evidence_acquisition.advance.max_steps",
            found: max_steps,
            max: MAX_EVIDENCE_ACQUISITION_ADVANCE_STEPS,
        });
    }
    validate_session(session, plan)?;
    validate_replayed_events(session, plan, real_data, public_literature)?;
    if session.next_sequence as usize > plan.steps.len() {
        return Err(NeurosurgeryError::SessionRejected {
            reason: "acquisition session has no remaining plan step".to_string(),
        });
    }
    let mut next = session.clone();
    let mut steps = Vec::new();
    for _ in 0..max_steps {
        let index = next.next_sequence as usize - 1;
        if index >= plan.steps.len() {
            break;
        }
        let planned = &plan.steps[index];
        let replayed = replay_step(planned, real_data, public_literature)?;
        let reference_digest = digest_value(&replayed.references)?;
        let event_digest = digest_value(&(
            next.event_chain_digest.as_str(),
            replayed.sequence,
            replayed.step_id.as_str(),
            replayed.source,
            replayed.status,
            replayed.total_matches,
            replayed.returned_matches,
            replayed.truncated,
            reference_digest.as_str(),
        ))?;
        next.events.push(EvidenceAcquisitionEvent {
            ordinal: next.next_sequence,
            sequence: replayed.sequence,
            step_id: replayed.step_id.clone(),
            source: replayed.source,
            status: replayed.status,
            total_matches: replayed.total_matches,
            returned_matches: replayed.returned_matches,
            truncated: replayed.truncated,
            reference_digest,
            previous_event_digest: next.event_chain_digest.clone(),
            event_digest: event_digest.clone(),
        });
        next.event_chain_digest = event_digest;
        next.next_sequence = next.next_sequence.saturating_add(1);
        steps.push(replayed);
    }
    next.status = if !plan.required_sources.is_empty() {
        EvidenceAcquisitionSessionStatus::NeedsEvidence
    } else if next.next_sequence as usize > plan.steps.len() {
        EvidenceAcquisitionSessionStatus::AwaitingHumanReview
    } else {
        EvidenceAcquisitionSessionStatus::Running
    };
    let complete = next.next_sequence as usize > plan.steps.len();
    Ok(EvidenceAcquisitionAdvanceResult {
        schema_version: EVIDENCE_ACQUISITION_EXECUTION_SCHEMA_VERSION.to_string(),
        session: next,
        steps_executed: steps.len(),
        complete,
        steps,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: execution_limitations(),
    })
}

/// Finish a fully replayed acquisition worker. Completion is refused when a required evidence
/// plane was absent or any plan step remains, so a caller cannot mistake an empty wave for success.
pub fn finish(
    session: &EvidenceAcquisitionSession,
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    query: &EvidenceAcquisitionQuery,
) -> Result<EvidenceAcquisitionExecutionReport, NeurosurgeryError> {
    finish_with_case_assets(session, request, real_data, public_literature, None, query)
}

/// Finish a checkpoint whose acquisition plan includes a case-asset review projection.
pub fn finish_with_case_assets(
    session: &EvidenceAcquisitionSession,
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    case_asset_report: Option<&CaseAssetManifestReport>,
    query: &EvidenceAcquisitionQuery,
) -> Result<EvidenceAcquisitionExecutionReport, NeurosurgeryError> {
    let plan = compile_with_case_assets(
        request,
        real_data,
        public_literature,
        case_asset_report,
        query,
    )?;
    finish_with_compiled_plan(session, &plan, real_data, public_literature)
}

/// Finish a checkpoint whose plan carries a case-asset review disposition ledger.
pub fn finish_with_case_assets_and_dispositions(
    session: &EvidenceAcquisitionSession,
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    case_asset_report: Option<&CaseAssetManifestReport>,
    dispositions: &crate::CaseAssetReviewDispositionReport,
    query: &EvidenceAcquisitionQuery,
) -> Result<EvidenceAcquisitionExecutionReport, NeurosurgeryError> {
    let plan = compile_with_case_assets_and_dispositions(
        request,
        real_data,
        public_literature,
        case_asset_report,
        dispositions,
        query,
    )?;
    finish_with_compiled_plan(session, &plan, real_data, public_literature)
}

fn finish_with_compiled_plan(
    session: &EvidenceAcquisitionSession,
    plan: &EvidenceAcquisitionReport,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
) -> Result<EvidenceAcquisitionExecutionReport, NeurosurgeryError> {
    validate_session(session, plan)?;
    validate_replayed_events(session, plan, real_data, public_literature)?;
    if !plan.required_sources.is_empty() {
        return Err(NeurosurgeryError::SessionRejected {
            reason: format!(
                "acquisition cannot finish while required source planes are missing: {:?}",
                plan.required_sources
            ),
        });
    }
    if session.next_sequence as usize <= plan.steps.len() {
        return Err(NeurosurgeryError::SessionRejected {
            reason: "acquisition session must replay every plan step before finish".to_string(),
        });
    }
    Ok(EvidenceAcquisitionExecutionReport {
        schema_version: EVIDENCE_ACQUISITION_EXECUTION_SCHEMA_VERSION.to_string(),
        plan_digest: plan.plan_digest.clone(),
        request_digest: plan.request_digest.clone(),
        specialty: plan.specialty,
        steps_executed: session.events.len(),
        event_count: session.events.len(),
        event_chain_digest: session.event_chain_digest.clone(),
        case_asset_report_digest: plan.case_asset_report_digest.clone(),
        case_asset_review_disposition_digest: plan.case_asset_review_disposition_digest.clone(),
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: execution_limitations(),
    })
}

fn replay_step(
    planned: &EvidenceAcquisitionStep,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
) -> Result<EvidenceAcquisitionExecutionStep, NeurosurgeryError> {
    let (source, total_matches, returned_matches, truncated, references) = match &planned.query {
        EvidenceAcquisitionSourceQuery::RealGliomaPopulation(query) => {
            let data = real_data.ok_or_else(|| NeurosurgeryError::SessionRejected {
                reason: "acquisition replay requires the bound real glioma snapshot".to_string(),
            })?;
            let result = data.query(query)?;
            let references = result
                .hits
                .into_iter()
                .map(|hit| ResearchPlanReference {
                    source: ResearchPlanSource::RealGliomaPopulation,
                    source_id: hit.source_id,
                    record_id: hit.record_id,
                    title: hit.title,
                    uri: hit.source_uri,
                })
                .collect::<Vec<_>>();
            (
                ResearchPlanSource::RealGliomaPopulation,
                result.total_matches,
                result.returned_matches,
                result.truncated,
                references,
            )
        }
        EvidenceAcquisitionSourceQuery::PublicLiterature(query) => {
            let literature =
                public_literature.ok_or_else(|| NeurosurgeryError::SessionRejected {
                    reason: "acquisition replay requires the bound public literature snapshot"
                        .to_string(),
                })?;
            let result = literature.query(query)?;
            let references = result
                .hits
                .into_iter()
                .map(|hit| ResearchPlanReference {
                    source: ResearchPlanSource::PublicLiterature,
                    source_id: hit.source_id,
                    record_id: hit.pmid,
                    title: hit.title,
                    uri: hit.record_uri,
                })
                .collect::<Vec<_>>();
            (
                ResearchPlanSource::PublicLiterature,
                result.total_matches,
                result.returned_matches,
                result.truncated,
                references,
            )
        }
    };
    let status = if truncated {
        EvidenceAcquisitionStepStatus::Truncated
    } else if total_matches == 0 {
        EvidenceAcquisitionStepStatus::NoLocalMatches
    } else {
        EvidenceAcquisitionStepStatus::CandidatesFound
    };
    if source != planned.source
        || total_matches != planned.total_matches
        || returned_matches != planned.returned_matches
        || truncated != planned.truncated
        || status != planned.status
    {
        return Err(NeurosurgeryError::SessionRejected {
            reason: format!(
                "acquisition replay drift detected for step {}",
                planned.step_id
            ),
        });
    }
    Ok(EvidenceAcquisitionExecutionStep {
        sequence: planned.sequence,
        step_id: planned.step_id.clone(),
        source,
        status,
        total_matches,
        returned_matches,
        truncated,
        references,
    })
}

fn validate_session(
    session: &EvidenceAcquisitionSession,
    plan: &EvidenceAcquisitionReport,
) -> Result<(), NeurosurgeryError> {
    if session.schema_version != EVIDENCE_ACQUISITION_SESSION_SCHEMA_VERSION
        || session.session_id != format!("nsa-session-{}", &plan.plan_digest[..16])
        || session.plan_digest != plan.plan_digest
        || session.request_digest != plan.request_digest
        || session.specialty != plan.specialty
        || session.real_data_digest != plan.real_data_digest
        || session.public_literature_digest != plan.public_literature_digest
        || session.case_asset_report_digest != plan.case_asset_report_digest
        || session.case_asset_review_disposition_digest != plan.case_asset_review_disposition_digest
        || session.next_sequence as usize != session.events.len() + 1
        || session.events.len() > plan.steps.len()
    {
        return Err(NeurosurgeryError::SessionRejected {
            reason: "acquisition session envelope or bound input is invalid".to_string(),
        });
    }
    let initial_chain = digest_value(&(
        session.session_id.as_str(),
        plan.plan_digest.as_str(),
        plan.request_digest.as_str(),
    ))?;
    let mut previous = initial_chain;
    for (index, event) in session.events.iter().enumerate() {
        let planned = &plan.steps[index];
        if event.ordinal as usize != index + 1
            || event.sequence != planned.sequence
            || event.step_id != planned.step_id
            || event.source != planned.source
            || event.status != planned.status
            || event.total_matches != planned.total_matches
            || event.returned_matches != planned.returned_matches
            || event.truncated != planned.truncated
            || event.previous_event_digest != previous
            || event.reference_digest.len() != 64
            || event.event_digest
                != digest_value(&(
                    previous.as_str(),
                    event.sequence,
                    event.step_id.as_str(),
                    event.source,
                    event.status,
                    event.total_matches,
                    event.returned_matches,
                    event.truncated,
                    event.reference_digest.as_str(),
                ))?
        {
            return Err(NeurosurgeryError::SessionRejected {
                reason: "acquisition event chain is invalid or tampered".to_string(),
            });
        }
        previous = event.event_digest.clone();
    }
    if session.event_chain_digest != previous {
        return Err(NeurosurgeryError::SessionRejected {
            reason: "acquisition event-chain digest does not match events".to_string(),
        });
    }
    Ok(())
}

fn validate_replayed_events(
    session: &EvidenceAcquisitionSession,
    plan: &EvidenceAcquisitionReport,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
) -> Result<(), NeurosurgeryError> {
    for (index, event) in session.events.iter().enumerate() {
        let replayed = replay_step(&plan.steps[index], real_data, public_literature)?;
        let reference_digest = digest_value(&replayed.references)?;
        if event.reference_digest != reference_digest {
            return Err(NeurosurgeryError::SessionRejected {
                reason: format!(
                    "acquisition reference digest does not match replay for step {}",
                    event.step_id
                ),
            });
        }
    }
    Ok(())
}

fn execution_limitations() -> Vec<String> {
    vec![
        "execution replays only caller-supplied validated local snapshots and never fetches a source".to_string(),
        "references are citation/population metadata and never patient findings or clinical conclusions".to_string(),
        "the checkpoint retains digests and bounded event metadata; the caller owns all source records and durable persistence".to_string(),
        "human review remains required after every completed acquisition wave".to_string(),
    ]
}

fn execute_source_query(
    source: ResearchPlanSource,
    specialty: Specialty,
    observation_kind: Option<ObservationKind>,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    limit: usize,
) -> Result<AcquisitionQueryResult, NeurosurgeryError> {
    let text = observation_kind.map(|kind| acquisition_term(specialty, kind).to_string());
    match source {
        ResearchPlanSource::RealGliomaPopulation => {
            let data = real_data.expect("real source is present when selected");
            let record_kind = match observation_kind {
                Some(ObservationKind::Molecular) => {
                    Some(RealDataRecordKind::PortalMolecularProfile)
                }
                Some(_) => Some(RealDataRecordKind::LiteratureArticle),
                None => None,
            };
            let mut query = RealDataQuery {
                text: text.clone(),
                status: None,
                trial_phase: None,
                trial_study_type: None,
                trial_updated_from: None,
                trial_updated_to: None,
                molecular_alteration_type: None,
                molecular_datatype: None,
                genomic_data_type: None,
                publication_type: None,
                mesh_term: None,
                publication_date_from: None,
                publication_date_to: None,
                record_kind,
                source_id: None,
                related_record_id: None,
                limit,
            };
            let mut result = data.query(&query)?;
            let mut fallback = false;
            if result.total_matches == 0 && query.text.is_some() {
                query.text = None;
                result = data.query(&query)?;
                fallback = true;
            }
            let refs = result
                .hits
                .into_iter()
                .map(|hit| ResearchPlanReference {
                    source,
                    source_id: hit.source_id,
                    record_id: hit.record_id,
                    title: hit.title,
                    uri: hit.source_uri,
                })
                .collect();
            Ok((
                EvidenceAcquisitionSourceQuery::RealGliomaPopulation(Box::new(query)),
                fallback,
                result.total_matches,
                result.returned_matches,
                result.truncated,
                refs,
            ))
        }
        ResearchPlanSource::PublicLiterature => {
            let literature = public_literature.expect("literature source is present when selected");
            let mut query = PublicLiteratureQuery {
                specialty: Some(specialty),
                text: text.clone(),
                publication_type: None,
                mesh_term: None,
                from_date: None,
                to_date: None,
                limit,
            };
            let mut result = literature.query(&query)?;
            let mut fallback = false;
            if result.total_matches == 0 && query.text.is_some() {
                query.text = None;
                result = literature.query(&query)?;
                fallback = true;
            }
            let refs = result
                .hits
                .into_iter()
                .map(|hit| ResearchPlanReference {
                    source,
                    source_id: hit.source_id,
                    record_id: hit.pmid,
                    title: hit.title,
                    uri: hit.record_uri,
                })
                .collect();
            Ok((
                EvidenceAcquisitionSourceQuery::PublicLiterature(query),
                fallback,
                result.total_matches,
                result.returned_matches,
                result.truncated,
                refs,
            ))
        }
    }
}

fn acquisition_term(specialty: Specialty, kind: ObservationKind) -> &'static str {
    match (specialty, kind) {
        (Specialty::Glioma, ObservationKind::Molecular) => "molecular",
        (Specialty::Glioma, ObservationKind::Histology) => "histology",
        (Specialty::Glioma, ObservationKind::Imaging) => "imaging",
        (Specialty::CranialBase, ObservationKind::Neuroanatomy) => "skull base anatomy",
        (Specialty::CranialBase, ObservationKind::Imaging) => "skull base imaging",
        (Specialty::Craniosynostosis, ObservationKind::DevelopmentalTrajectory) => "developmental",
        (Specialty::Craniosynostosis, ObservationKind::Imaging) => "craniosynostosis imaging",
        (Specialty::Encephalocele, ObservationKind::DevelopmentalTrajectory) => {
            "encephalocele development"
        }
        (Specialty::Encephalocele, ObservationKind::Imaging) => "encephalocele imaging",
        (Specialty::SpinaBifida, ObservationKind::SpinalDysraphism) => "spinal dysraphism",
        (Specialty::SpinaBifida, ObservationKind::NeurologicFunction) => "neurologic function",
        (Specialty::ChiariMalformation, ObservationKind::CraniocervicalJunction) => {
            "craniocervical junction"
        }
        (Specialty::ChiariMalformation, ObservationKind::Imaging) => "Chiari imaging",
        (_, ObservationKind::Imaging) => "imaging",
        (_, ObservationKind::Histology) => "histology",
        (_, ObservationKind::Molecular) => "molecular",
        (_, ObservationKind::Neuroanatomy) => "neuroanatomy",
        (_, ObservationKind::NeurologicFunction) => "neurologic function",
        (_, ObservationKind::DevelopmentalTrajectory) => "developmental trajectory",
        (_, ObservationKind::SpinalDysraphism) => "spinal dysraphism",
        (_, ObservationKind::CraniocervicalJunction) => "craniocervical junction",
        (_, ObservationKind::SurgicalHistory) => "surgical history",
        (_, ObservationKind::LongitudinalOutcome) => "outcome",
    }
}

fn digest_step(
    sequence: u16,
    source: ResearchPlanSource,
    trigger: EvidenceAcquisitionTrigger,
    observation_kind: Option<ObservationKind>,
) -> String {
    let value = (
        EVIDENCE_ACQUISITION_SCHEMA_VERSION,
        sequence,
        source,
        trigger,
        observation_kind,
    );
    let bytes = serde_json::to_vec(&value).expect("acquisition step digest input serialises");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("acquisition-step-{:x}", hasher.finalize())
}

fn digest_request(request: &CaseRequest) -> Result<String, NeurosurgeryError> {
    let bytes = serde_json::to_vec(request)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn digest_value<T: Serialize>(value: &T) -> Result<String, NeurosurgeryError> {
    let bytes =
        serde_json::to_vec(value).map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn digest_report(report: &EvidenceAcquisitionReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.plan_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}
