//! Multi-study steward session over separately completed local review packets.
//!
//! GAF-GLIOMA-P11-F18 reconciles one P11-F14 expected cohort with its P11-F17 local review
//! packets. It binds each packet to the exact per-study workflow digest and disposition, keeps
//! every study separate, and makes missing or incomplete checklist work visible. Study and
//! reviewer commitments remain caller-supplied; this module cannot authenticate identities or
//! authorize, sign, or publish a portfolio.

use super::multistudy_release::{
    MAX_STUDIES, MultiStudyEvidenceDisposition, MultiStudyReleaseReport,
};
use super::review_workbench::{LocalReleaseReviewPacket, LocalReleaseReviewPacketDisposition};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = super::comparative_release_explorer::FEATURE_ID;
pub const INPUT_SCHEMA: &str = "GliomaPortfolioReviewWorkbenchRequest1@1";
pub const OUTPUT_SCHEMA: &str = "GliomaPortfolioReviewWorkbench1@1";
pub const MAX_REVIEW_PACKET_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_TOTAL_REVIEW_PACKET_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortfolioReviewPacketInput {
    pub study_commitment: ContentHash,
    pub packet: LocalReleaseReviewPacket,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortfolioReviewWorkbenchRequest {
    pub portfolio: MultiStudyReleaseReport,
    /// Packets may be partial; every expected cohort member remains in the output.
    pub review_packets: Vec<PortfolioReviewPacketInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioStudyReviewDisposition {
    MissingStudy,
    ReviewPending,
    ReviewComplete,
    ActionRequired,
    Unresolved,
    Hold,
    Blocked,
    NonReproducible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortfolioStudyReviewEvidence {
    pub study_commitment: ContentHash,
    pub workflow_disposition: MultiStudyEvidenceDisposition,
    pub workflow_digest: Option<ContentHash>,
    pub review_packet_digest: Option<ContentHash>,
    pub review_disposition: Option<LocalReleaseReviewPacketDisposition>,
    pub disposition: PortfolioStudyReviewDisposition,
    pub previous_digest: ContentHash,
    pub entry_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortfolioReviewCounts {
    pub expected_studies: u16,
    pub workflows_ready: u16,
    pub review_complete: u16,
    pub review_pending: u16,
    pub action_required: u16,
    pub unresolved: u16,
    pub held: u16,
    pub blocked: u16,
    pub non_reproducible: u16,
    pub missing: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioReviewDisposition {
    ReviewComplete,
    ReviewPending,
    ActionRequired,
    Insufficient,
    Unresolved,
    Hold,
    Blocked,
    NonReproducible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortfolioReviewWorkbenchReport {
    pub feature_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub research_id: String,
    pub portfolio_digest: ContentHash,
    pub expected_study_commitments: Vec<ContentHash>,
    /// Entries are in exact expected-cohort order; no artifacts or raw study IDs are copied.
    pub evidence: Vec<PortfolioStudyReviewEvidence>,
    pub counts: PortfolioReviewCounts,
    pub disposition: PortfolioReviewDisposition,
    pub next_operator_action: String,
    /// Always false: neither study nor reviewer commitments are authenticated by this report.
    pub identities_authenticated: bool,
    /// Always false: checklist aggregation does not satisfy signing or publication authority.
    pub release_authorized: bool,
    pub chain_head: ContentHash,
    pub report_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PortfolioReviewWorkbenchError {
    #[error("portfolio review workbench request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-study portfolio is invalid: {0}")]
    InvalidPortfolio(String),
    #[error("local review packet is invalid: {0}")]
    InvalidPacket(String),
    #[error("portfolio review workbench report is invalid: {0}")]
    InvalidOutput(String),
    #[error("portfolio review workbench digest failed: {0}")]
    Digest(String),
}

fn digest<T: Serialize>(value: &T) -> Result<ContentHash, PortfolioReviewWorkbenchError> {
    ContentHash::of_serializable(value)
        .map_err(|error| PortfolioReviewWorkbenchError::Digest(error.to_string()))
}

fn seed_digest(
    research_id: &str,
    portfolio_digest: &ContentHash,
    expected_study_commitments: &[ContentHash],
) -> Result<ContentHash, PortfolioReviewWorkbenchError> {
    digest(&(
        FEATURE_ID,
        INPUT_SCHEMA,
        OUTPUT_SCHEMA,
        research_id,
        portfolio_digest,
        expected_study_commitments,
    ))
}

fn entry_digest(
    entry: &PortfolioStudyReviewEvidence,
) -> Result<ContentHash, PortfolioReviewWorkbenchError> {
    digest(&(
        FEATURE_ID,
        OUTPUT_SCHEMA,
        &entry.study_commitment,
        entry.workflow_disposition,
        &entry.workflow_digest,
        &entry.review_packet_digest,
        entry.review_disposition,
        entry.disposition,
        &entry.previous_digest,
    ))
}

fn report_digest(
    report: &PortfolioReviewWorkbenchReport,
) -> Result<ContentHash, PortfolioReviewWorkbenchError> {
    digest(&(
        &report.feature_id,
        &report.input_schema,
        &report.output_schema,
        &report.research_id,
        &report.portfolio_digest,
        &report.expected_study_commitments,
        &report.evidence,
        &report.counts,
        report.disposition,
        &report.next_operator_action,
        report.identities_authenticated,
        report.release_authorized,
        &report.chain_head,
    ))
}

fn study_disposition(
    workflow: MultiStudyEvidenceDisposition,
    review: Option<LocalReleaseReviewPacketDisposition>,
) -> PortfolioStudyReviewDisposition {
    match workflow {
        MultiStudyEvidenceDisposition::Missing => PortfolioStudyReviewDisposition::MissingStudy,
        MultiStudyEvidenceDisposition::NonReproducible => {
            PortfolioStudyReviewDisposition::NonReproducible
        }
        MultiStudyEvidenceDisposition::Blocked => PortfolioStudyReviewDisposition::Blocked,
        MultiStudyEvidenceDisposition::Hold => PortfolioStudyReviewDisposition::Hold,
        MultiStudyEvidenceDisposition::Unresolved => PortfolioStudyReviewDisposition::Unresolved,
        MultiStudyEvidenceDisposition::ReadyForSigning => match review {
            None | Some(LocalReleaseReviewPacketDisposition::ReviewPending) => {
                PortfolioStudyReviewDisposition::ReviewPending
            }
            Some(LocalReleaseReviewPacketDisposition::ReviewComplete) => {
                PortfolioStudyReviewDisposition::ReviewComplete
            }
            Some(LocalReleaseReviewPacketDisposition::ActionRequired) => {
                PortfolioStudyReviewDisposition::ActionRequired
            }
            Some(LocalReleaseReviewPacketDisposition::Unresolved) => {
                PortfolioStudyReviewDisposition::Unresolved
            }
            Some(LocalReleaseReviewPacketDisposition::Hold) => {
                PortfolioStudyReviewDisposition::Hold
            }
            Some(LocalReleaseReviewPacketDisposition::Blocked) => {
                PortfolioStudyReviewDisposition::Blocked
            }
            Some(LocalReleaseReviewPacketDisposition::NonReproducible) => {
                PortfolioStudyReviewDisposition::NonReproducible
            }
        },
    }
}

fn counts(evidence: &[PortfolioStudyReviewEvidence]) -> PortfolioReviewCounts {
    let count = |disposition| {
        evidence
            .iter()
            .filter(|item| item.disposition == disposition)
            .count() as u16
    };
    PortfolioReviewCounts {
        expected_studies: evidence.len() as u16,
        workflows_ready: evidence
            .iter()
            .filter(|item| {
                item.workflow_disposition == MultiStudyEvidenceDisposition::ReadyForSigning
            })
            .count() as u16,
        review_complete: count(PortfolioStudyReviewDisposition::ReviewComplete),
        review_pending: count(PortfolioStudyReviewDisposition::ReviewPending),
        action_required: count(PortfolioStudyReviewDisposition::ActionRequired),
        unresolved: count(PortfolioStudyReviewDisposition::Unresolved),
        held: count(PortfolioStudyReviewDisposition::Hold),
        blocked: count(PortfolioStudyReviewDisposition::Blocked),
        non_reproducible: count(PortfolioStudyReviewDisposition::NonReproducible),
        missing: count(PortfolioStudyReviewDisposition::MissingStudy),
    }
}

fn overall_disposition(counts: &PortfolioReviewCounts) -> PortfolioReviewDisposition {
    if counts.non_reproducible > 0 {
        PortfolioReviewDisposition::NonReproducible
    } else if counts.blocked > 0 {
        PortfolioReviewDisposition::Blocked
    } else if counts.held > 0 {
        PortfolioReviewDisposition::Hold
    } else if counts.unresolved > 0 {
        PortfolioReviewDisposition::Unresolved
    } else if counts.missing > 0 {
        PortfolioReviewDisposition::Insufficient
    } else if counts.action_required > 0 {
        PortfolioReviewDisposition::ActionRequired
    } else if counts.review_pending > 0 {
        PortfolioReviewDisposition::ReviewPending
    } else {
        PortfolioReviewDisposition::ReviewComplete
    }
}

fn next_action(disposition: PortfolioReviewDisposition) -> &'static str {
    match disposition {
        PortfolioReviewDisposition::ReviewComplete => {
            "route completed study packets through the institution's P11-F20 accountable approval and signing workflow"
        }
        PortfolioReviewDisposition::ReviewPending => {
            "complete the pending P11-F17 checklist for every expected study"
        }
        PortfolioReviewDisposition::ActionRequired => {
            "resolve requested study corrections and regenerate the affected local workflow and checklist"
        }
        PortfolioReviewDisposition::Insufficient => {
            "collect the missing expected study workflow and review packet before portfolio review"
        }
        PortfolioReviewDisposition::Unresolved => {
            "resolve unavailable workflow or reviewer evidence for the affected studies"
        }
        PortfolioReviewDisposition::Hold => {
            "resolve held local release workflows before portfolio signing review"
        }
        PortfolioReviewDisposition::Blocked => {
            "repair blocked study packages or dependency closures before portfolio review"
        }
        PortfolioReviewDisposition::NonReproducible => {
            "preserve divergent study outcomes and withhold a portfolio reproducibility claim"
        }
    }
}

fn workflow_disposition_matches(
    workflow: MultiStudyEvidenceDisposition,
    packet: &LocalReleaseReviewPacket,
) -> bool {
    matches!(
        (workflow, packet.workflow_disposition),
        (
            MultiStudyEvidenceDisposition::ReadyForSigning,
            super::local_release_workflow::GliomaLocalReleaseWorkflowDisposition::ReadyForSigning
        ) | (
            MultiStudyEvidenceDisposition::Hold,
            super::local_release_workflow::GliomaLocalReleaseWorkflowDisposition::Hold
        ) | (
            MultiStudyEvidenceDisposition::Unresolved,
            super::local_release_workflow::GliomaLocalReleaseWorkflowDisposition::Unresolved
        ) | (
            MultiStudyEvidenceDisposition::Blocked,
            super::local_release_workflow::GliomaLocalReleaseWorkflowDisposition::Blocked
        ) | (
            MultiStudyEvidenceDisposition::NonReproducible,
            super::local_release_workflow::GliomaLocalReleaseWorkflowDisposition::NonReproducible
        )
    )
}

fn packet_disposition_matches(
    workflow: MultiStudyEvidenceDisposition,
    packet: Option<LocalReleaseReviewPacketDisposition>,
) -> bool {
    match workflow {
        MultiStudyEvidenceDisposition::Missing => packet.is_none(),
        MultiStudyEvidenceDisposition::ReadyForSigning => matches!(
            packet,
            None | Some(LocalReleaseReviewPacketDisposition::ReviewPending)
                | Some(LocalReleaseReviewPacketDisposition::ReviewComplete)
                | Some(LocalReleaseReviewPacketDisposition::ActionRequired)
                | Some(LocalReleaseReviewPacketDisposition::Unresolved)
        ),
        MultiStudyEvidenceDisposition::Hold => {
            packet.is_none() || packet == Some(LocalReleaseReviewPacketDisposition::Hold)
        }
        MultiStudyEvidenceDisposition::Unresolved => {
            packet.is_none() || packet == Some(LocalReleaseReviewPacketDisposition::Unresolved)
        }
        MultiStudyEvidenceDisposition::Blocked => {
            packet.is_none() || packet == Some(LocalReleaseReviewPacketDisposition::Blocked)
        }
        MultiStudyEvidenceDisposition::NonReproducible => {
            packet.is_none() || packet == Some(LocalReleaseReviewPacketDisposition::NonReproducible)
        }
    }
}

/// Reconcile local reviewer packets against the exact expected multi-study portfolio cohort.
pub fn reconcile_glioma_portfolio_review_workbench(
    request: &PortfolioReviewWorkbenchRequest,
) -> Result<PortfolioReviewWorkbenchReport, PortfolioReviewWorkbenchError> {
    request
        .portfolio
        .validate()
        .map_err(|error| PortfolioReviewWorkbenchError::InvalidPortfolio(error.to_string()))?;
    if request.review_packets.len() > request.portfolio.expected_study_commitments.len() {
        return Err(PortfolioReviewWorkbenchError::InvalidRequest(
            "review packets cannot exceed the expected study cohort".into(),
        ));
    }
    let portfolio_evidence = request
        .portfolio
        .evidence
        .iter()
        .map(|item| (item.study_commitment.clone(), item))
        .collect::<BTreeMap<_, _>>();
    let mut packets = BTreeMap::new();
    let mut study_ids = BTreeSet::new();
    let mut total_packet_bytes = 0_usize;
    for input in &request.review_packets {
        let evidence = portfolio_evidence
            .get(&input.study_commitment)
            .ok_or_else(|| {
                PortfolioReviewWorkbenchError::InvalidRequest(
                    "every packet must map to one expected study commitment".into(),
                )
            })?;
        input
            .packet
            .validate()
            .map_err(|error| PortfolioReviewWorkbenchError::InvalidPacket(error.to_string()))?;
        let bytes = serde_json::to_vec(&input.packet)
            .map_err(|error| PortfolioReviewWorkbenchError::InvalidRequest(error.to_string()))?
            .len();
        if bytes > MAX_REVIEW_PACKET_BYTES {
            return Err(PortfolioReviewWorkbenchError::InvalidRequest(
                "a local review packet exceeds the per-study size bound".into(),
            ));
        }
        total_packet_bytes = total_packet_bytes.saturating_add(bytes);
        if total_packet_bytes > MAX_TOTAL_REVIEW_PACKET_BYTES
            || evidence.disposition == MultiStudyEvidenceDisposition::Missing
            || evidence.workflow_digest.as_ref() != Some(&input.packet.workflow_digest)
            || input.packet.research_id != request.portfolio.research_id
            || !workflow_disposition_matches(evidence.disposition, &input.packet)
            || !study_ids.insert(input.packet.study_id.clone())
            || packets
                .insert(input.study_commitment.clone(), &input.packet)
                .is_some()
        {
            return Err(PortfolioReviewWorkbenchError::InvalidRequest(
                "packet mapping, workflow digest/disposition, identity, uniqueness, or total payload bound is invalid".into(),
            ));
        }
    }

    let mut previous_digest = seed_digest(
        &request.portfolio.research_id,
        &request.portfolio.report_digest,
        &request.portfolio.expected_study_commitments,
    )?;
    let mut evidence = Vec::with_capacity(request.portfolio.expected_study_commitments.len());
    for commitment in &request.portfolio.expected_study_commitments {
        let source = portfolio_evidence
            .get(commitment)
            .expect("validated portfolio has one row per expected study");
        let packet = packets.get(commitment);
        let mut entry = PortfolioStudyReviewEvidence {
            study_commitment: commitment.clone(),
            workflow_disposition: source.disposition,
            workflow_digest: source.workflow_digest.clone(),
            review_packet_digest: packet.map(|packet| packet.packet_digest.clone()),
            review_disposition: packet.map(|packet| packet.disposition),
            disposition: study_disposition(
                source.disposition,
                packet.map(|packet| packet.disposition),
            ),
            previous_digest: previous_digest.clone(),
            entry_digest: ContentHash::of_bytes(b"unsealed-glioma-portfolio-review-entry"),
        };
        entry.entry_digest = entry_digest(&entry)?;
        previous_digest = entry.entry_digest.clone();
        evidence.push(entry);
    }
    let counts = counts(&evidence);
    let disposition = overall_disposition(&counts);
    let mut report = PortfolioReviewWorkbenchReport {
        feature_id: FEATURE_ID.into(),
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: request.portfolio.research_id.clone(),
        portfolio_digest: request.portfolio.report_digest.clone(),
        expected_study_commitments: request.portfolio.expected_study_commitments.clone(),
        evidence,
        counts,
        disposition,
        next_operator_action: next_action(disposition).into(),
        identities_authenticated: false,
        release_authorized: false,
        chain_head: previous_digest,
        report_digest: ContentHash::of_bytes(b"unsealed-glioma-portfolio-review-report"),
    };
    report.report_digest = report_digest(&report)?;
    report.validate()?;
    Ok(report)
}

impl PortfolioReviewWorkbenchReport {
    /// Validate exact cohort coverage, packet states, counts, and the digest chain.
    pub fn validate(&self) -> Result<(), PortfolioReviewWorkbenchError> {
        if self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.research_id.trim().is_empty()
            || !(2..=MAX_STUDIES).contains(&self.expected_study_commitments.len())
            || self
                .expected_study_commitments
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.evidence.len() != self.expected_study_commitments.len()
            || self.identities_authenticated
            || self.release_authorized
            || self.next_operator_action != next_action(self.disposition)
        {
            return Err(PortfolioReviewWorkbenchError::InvalidOutput(
                "report identity, cohort ordering, evidence coverage, authorization flags, or operator action is invalid".into(),
            ));
        }
        let mut previous_digest = seed_digest(
            &self.research_id,
            &self.portfolio_digest,
            &self.expected_study_commitments,
        )?;
        for (expected, item) in self.expected_study_commitments.iter().zip(&self.evidence) {
            let has_workflow = item.workflow_disposition != MultiStudyEvidenceDisposition::Missing;
            let has_packet =
                item.review_packet_digest.is_some() == item.review_disposition.is_some();
            if &item.study_commitment != expected
                || item.previous_digest != previous_digest
                || item.entry_digest != entry_digest(item)?
                || item.workflow_digest.is_some() != has_workflow
                || !has_packet
                || (item.workflow_disposition == MultiStudyEvidenceDisposition::Missing
                    && item.review_disposition.is_some())
                || !packet_disposition_matches(item.workflow_disposition, item.review_disposition)
                || item.disposition
                    != study_disposition(item.workflow_disposition, item.review_disposition)
            {
                return Err(PortfolioReviewWorkbenchError::InvalidOutput(
                    "study identity, packet/workflow presence, disposition, or digest chain is invalid".into(),
                ));
            }
            previous_digest = item.entry_digest.clone();
        }
        let expected_counts = counts(&self.evidence);
        if self.chain_head != previous_digest
            || self.counts != expected_counts
            || self.disposition != overall_disposition(&expected_counts)
            || self.report_digest != report_digest(self)?
        {
            return Err(PortfolioReviewWorkbenchError::InvalidOutput(
                "portfolio counts, disposition, chain head, or report digest is inconsistent"
                    .into(),
            ));
        }
        Ok(())
    }
}
