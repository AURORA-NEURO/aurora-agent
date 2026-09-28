//! Multi-study release readiness orchestration for preclinical glioma research.
//!
//! GAF-GLIOMA-P11-F14 reconciles completed local P11-F13 workflows within one research
//! portfolio. Study evidence remains separately content-addressed; this module emits only opaque
//! study commitments, manifest/workflow digests, aggregate counts, and release states. It neither
//! merges study artifacts nor authenticates the caller's commitments or review identities.

use super::local_release_workflow::{
    GliomaLocalReleaseWorkflow, GliomaLocalReleaseWorkflowDisposition,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F14";
pub const INPUT_SCHEMA: &str = "GliomaResearchObjectMultiStudyReleaseRequest1@1";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectMultiStudyRelease1@1";
pub const MIN_STUDIES: usize = 2;
pub const MAX_STUDIES: usize = 64;
pub const MAX_WORKFLOW_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_TOTAL_WORKFLOW_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyReleaseInput {
    /// Caller-supplied opaque commitment; use a keyed commitment if study identifiers are sensitive.
    pub study_commitment: ContentHash,
    pub workflow: GliomaLocalReleaseWorkflow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyReleaseRequest {
    pub research_id: String,
    /// Sorted and complete cohort membership expected in the release portfolio.
    pub expected_study_commitments: Vec<ContentHash>,
    /// Completed P11-F13 workflows. Missing expected commitments remain explicit in the report.
    pub studies: Vec<MultiStudyReleaseInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyReleaseDisposition {
    ReadyForSigning,
    Insufficient,
    Hold,
    Unresolved,
    Blocked,
    NonReproducible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyEvidenceDisposition {
    ReadyForSigning,
    Hold,
    Unresolved,
    Blocked,
    NonReproducible,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyReleaseEvidence {
    pub study_commitment: ContentHash,
    pub disposition: MultiStudyEvidenceDisposition,
    pub workflow_digest: Option<ContentHash>,
    pub manifest_digest: Option<ContentHash>,
    pub previous_digest: ContentHash,
    pub entry_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyReleaseCounts {
    pub expected_studies: u16,
    pub observed_studies: u16,
    pub ready_for_signing: u16,
    pub held: u16,
    pub unresolved: u16,
    pub blocked: u16,
    pub non_reproducible: u16,
    pub missing: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyReleaseReport {
    pub feature_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub research_id: String,
    pub expected_study_commitments: Vec<ContentHash>,
    /// Entries are in expected commitment order and contain no raw study identifiers or artifacts.
    pub evidence: Vec<MultiStudyReleaseEvidence>,
    pub counts: MultiStudyReleaseCounts,
    pub disposition: MultiStudyReleaseDisposition,
    pub next_operator_action: String,
    pub chain_head: ContentHash,
    pub report_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultiStudyReleaseError {
    #[error("multi-study release request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-study release report is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-study release digest failed: {0}")]
    Digest(String),
}

fn digest<T: Serialize>(value: &T) -> Result<ContentHash, MultiStudyReleaseError> {
    ContentHash::of_serializable(value)
        .map_err(|error| MultiStudyReleaseError::Digest(error.to_string()))
}

fn seed_digest(
    research_id: &str,
    expected_study_commitments: &[ContentHash],
) -> Result<ContentHash, MultiStudyReleaseError> {
    digest(&(
        FEATURE_ID,
        INPUT_SCHEMA,
        OUTPUT_SCHEMA,
        research_id,
        expected_study_commitments,
    ))
}

fn evidence_digest(
    item: &MultiStudyReleaseEvidence,
) -> Result<ContentHash, MultiStudyReleaseError> {
    digest(&(
        FEATURE_ID,
        OUTPUT_SCHEMA,
        &item.study_commitment,
        item.disposition,
        &item.workflow_digest,
        &item.manifest_digest,
        &item.previous_digest,
    ))
}

fn evidence_disposition(
    disposition: Option<GliomaLocalReleaseWorkflowDisposition>,
) -> MultiStudyEvidenceDisposition {
    match disposition {
        Some(GliomaLocalReleaseWorkflowDisposition::ReadyForSigning) => {
            MultiStudyEvidenceDisposition::ReadyForSigning
        }
        Some(GliomaLocalReleaseWorkflowDisposition::Hold) => MultiStudyEvidenceDisposition::Hold,
        Some(GliomaLocalReleaseWorkflowDisposition::Unresolved) => {
            MultiStudyEvidenceDisposition::Unresolved
        }
        Some(GliomaLocalReleaseWorkflowDisposition::Blocked) => {
            MultiStudyEvidenceDisposition::Blocked
        }
        Some(GliomaLocalReleaseWorkflowDisposition::NonReproducible) => {
            MultiStudyEvidenceDisposition::NonReproducible
        }
        None => MultiStudyEvidenceDisposition::Missing,
    }
}

fn counts(evidence: &[MultiStudyReleaseEvidence], expected: usize) -> MultiStudyReleaseCounts {
    let count = |disposition| {
        evidence
            .iter()
            .filter(|item| item.disposition == disposition)
            .count() as u16
    };
    MultiStudyReleaseCounts {
        expected_studies: expected as u16,
        observed_studies: evidence
            .iter()
            .filter(|item| item.disposition != MultiStudyEvidenceDisposition::Missing)
            .count() as u16,
        ready_for_signing: count(MultiStudyEvidenceDisposition::ReadyForSigning),
        held: count(MultiStudyEvidenceDisposition::Hold),
        unresolved: count(MultiStudyEvidenceDisposition::Unresolved),
        blocked: count(MultiStudyEvidenceDisposition::Blocked),
        non_reproducible: count(MultiStudyEvidenceDisposition::NonReproducible),
        missing: count(MultiStudyEvidenceDisposition::Missing),
    }
}

fn overall_disposition(counts: &MultiStudyReleaseCounts) -> MultiStudyReleaseDisposition {
    if counts.non_reproducible > 0 {
        MultiStudyReleaseDisposition::NonReproducible
    } else if counts.blocked > 0 {
        MultiStudyReleaseDisposition::Blocked
    } else if counts.unresolved > 0 {
        MultiStudyReleaseDisposition::Unresolved
    } else if counts.held > 0 {
        MultiStudyReleaseDisposition::Hold
    } else if counts.missing > 0 || counts.observed_studies < counts.expected_studies {
        MultiStudyReleaseDisposition::Insufficient
    } else {
        MultiStudyReleaseDisposition::ReadyForSigning
    }
}

fn next_action(disposition: MultiStudyReleaseDisposition) -> &'static str {
    match disposition {
        MultiStudyReleaseDisposition::ReadyForSigning => {
            "route each study object through the institution's accountable signing process"
        }
        MultiStudyReleaseDisposition::Insufficient => {
            "collect the missing expected study workflows before portfolio signing review"
        }
        MultiStudyReleaseDisposition::Hold => {
            "resolve held study workflows and regenerate the portfolio report"
        }
        MultiStudyReleaseDisposition::Unresolved => {
            "resolve unavailable evidence and keep the affected study objects out of signing"
        }
        MultiStudyReleaseDisposition::Blocked => {
            "repair blocked study packages or dependency closures before signing review"
        }
        MultiStudyReleaseDisposition::NonReproducible => {
            "preserve divergent study outcomes and do not issue a portfolio reproducibility claim"
        }
    }
}

fn validate_request(request: &MultiStudyReleaseRequest) -> Result<(), MultiStudyReleaseError> {
    if request.research_id.trim().is_empty()
        || request.research_id.len() > 256
        || request.expected_study_commitments.len() < MIN_STUDIES
        || request.expected_study_commitments.len() > MAX_STUDIES
        || request
            .expected_study_commitments
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.studies.len() > request.expected_study_commitments.len()
    {
        return Err(MultiStudyReleaseError::InvalidRequest(
            "research identity, two or more ordered expected studies, and bounded non-duplicate inputs are required".into(),
        ));
    }
    let expected = request
        .expected_study_commitments
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut supplied = BTreeSet::new();
    let mut study_ids = BTreeSet::new();
    let mut total_bytes = 0_usize;
    for input in &request.studies {
        if !expected.contains(&input.study_commitment)
            || !supplied.insert(input.study_commitment.clone())
            || !study_ids.insert(input.workflow.study_id.clone())
            || input.workflow.research_id != request.research_id
        {
            return Err(MultiStudyReleaseError::InvalidRequest(
                "study commitments must be expected and unique, study identities must be unique, and every workflow must share the research identity".into(),
            ));
        }
        input
            .workflow
            .validate()
            .map_err(|error| MultiStudyReleaseError::InvalidRequest(error.to_string()))?;
        let encoded = serde_json::to_vec(&input.workflow)
            .map_err(|error| MultiStudyReleaseError::InvalidRequest(error.to_string()))?;
        if encoded.len() > MAX_WORKFLOW_BYTES {
            return Err(MultiStudyReleaseError::InvalidRequest(
                "a study workflow exceeds the per-study aggregation size bound".into(),
            ));
        }
        total_bytes = total_bytes.saturating_add(encoded.len());
        if total_bytes > MAX_TOTAL_WORKFLOW_BYTES {
            return Err(MultiStudyReleaseError::InvalidRequest(
                "combined study workflows exceed the portfolio aggregation size bound".into(),
            ));
        }
    }
    Ok(())
}

fn report_digest(report: &MultiStudyReleaseReport) -> Result<ContentHash, MultiStudyReleaseError> {
    digest(&(
        &report.feature_id,
        &report.input_schema,
        &report.output_schema,
        &report.research_id,
        &report.expected_study_commitments,
        &report.evidence,
        &report.counts,
        report.disposition,
        &report.next_operator_action,
        &report.chain_head,
    ))
}

/// Reconcile completed per-study release workflows without merging their artifacts.
pub fn reconcile_glioma_multistudy_release(
    request: &MultiStudyReleaseRequest,
) -> Result<MultiStudyReleaseReport, MultiStudyReleaseError> {
    validate_request(request)?;
    let workflows = request
        .studies
        .iter()
        .map(|input| (input.study_commitment.clone(), &input.workflow))
        .collect::<BTreeMap<_, _>>();
    let mut previous_digest =
        seed_digest(&request.research_id, &request.expected_study_commitments)?;
    let mut evidence = Vec::with_capacity(request.expected_study_commitments.len());
    for study_commitment in &request.expected_study_commitments {
        let workflow = workflows.get(study_commitment);
        let mut item = MultiStudyReleaseEvidence {
            study_commitment: study_commitment.clone(),
            disposition: evidence_disposition(workflow.map(|item| item.disposition)),
            workflow_digest: workflow.map(|item| item.digest.clone()),
            manifest_digest: workflow.map(|item| item.bundle.manifest.manifest_digest.clone()),
            previous_digest: previous_digest.clone(),
            entry_digest: ContentHash::of_bytes(&[]),
        };
        item.entry_digest = evidence_digest(&item)?;
        previous_digest = item.entry_digest.clone();
        evidence.push(item);
    }
    let counts = counts(&evidence, request.expected_study_commitments.len());
    let disposition = overall_disposition(&counts);
    let mut report = MultiStudyReleaseReport {
        feature_id: FEATURE_ID.into(),
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: request.research_id.clone(),
        expected_study_commitments: request.expected_study_commitments.clone(),
        evidence,
        counts,
        disposition,
        next_operator_action: next_action(disposition).into(),
        chain_head: previous_digest,
        report_digest: ContentHash::of_bytes(&[]),
    };
    report.report_digest = report_digest(&report)?;
    report.validate()?;
    Ok(report)
}

impl MultiStudyReleaseReport {
    /// Validate cohort coverage, per-study evidence bindings, the digest chain, and report digest.
    pub fn validate(&self) -> Result<(), MultiStudyReleaseError> {
        if self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.research_id.trim().is_empty()
            || self.research_id.len() > 256
            || self.expected_study_commitments.len() < MIN_STUDIES
            || self.expected_study_commitments.len() > MAX_STUDIES
            || self
                .expected_study_commitments
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.evidence.len() != self.expected_study_commitments.len()
        {
            return Err(MultiStudyReleaseError::InvalidOutput(
                "report identity, cohort ordering, or evidence cardinality is invalid".into(),
            ));
        }
        let mut previous_digest = seed_digest(&self.research_id, &self.expected_study_commitments)?;
        for (expected, item) in self.expected_study_commitments.iter().zip(&self.evidence) {
            let missing = item.disposition == MultiStudyEvidenceDisposition::Missing;
            if &item.study_commitment != expected
                || item.previous_digest != previous_digest
                || item.entry_digest != evidence_digest(item)?
                || item.workflow_digest.is_some() == missing
                || item.manifest_digest.is_some() == missing
            {
                return Err(MultiStudyReleaseError::InvalidOutput(
                    "study order, presence partitions, or evidence digest chain is invalid".into(),
                ));
            }
            previous_digest = item.entry_digest.clone();
        }
        let expected_counts = counts(&self.evidence, self.expected_study_commitments.len());
        let expected_disposition = overall_disposition(&expected_counts);
        if self.counts != expected_counts
            || self.disposition != expected_disposition
            || self.next_operator_action != next_action(expected_disposition)
            || self.chain_head != previous_digest
            || self.report_digest != report_digest(self)?
        {
            return Err(MultiStudyReleaseError::InvalidOutput(
                "counts, portfolio disposition, operator handoff, or report digest is inconsistent"
                    .into(),
            ));
        }
        Ok(())
    }
}
