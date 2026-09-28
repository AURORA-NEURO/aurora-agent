//! Local reproducibility-steward checklist for a completed single-study release workflow.
//!
//! GAF-GLIOMA-P11-F17 turns a validated P11-F13 workflow into a fixed, digest-bound human review
//! packet. Responses are keyed to checklist commitments and preserve pending, corrective, and
//! unresolved states. This is an interaction record, not a release gate: reviewer identity and
//! independence are caller assertions, and the packet never authorizes, signs, or publishes.

use super::local_release_workflow::{
    GliomaLocalReleaseWorkflow, GliomaLocalReleaseWorkflowDisposition,
    GliomaLocalReleaseWorkflowError,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F17";
pub const INPUT_SCHEMA: &str = "GliomaLocalReleaseReviewPacketRequest1@1";
pub const OUTPUT_SCHEMA: &str = "GliomaLocalReleaseReviewPacket1@1";
pub const CHECKLIST_ITEM_COUNT: usize = 5;
pub const MAX_WORKFLOW_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseReviewCategory {
    PackageAndProvenance,
    DependencyClosure,
    ReplayReproducibility,
    ReleaseGate,
    LimitationsAndNegativeEvidence,
}

impl ReleaseReviewCategory {
    const ORDERED: [Self; CHECKLIST_ITEM_COUNT] = [
        Self::PackageAndProvenance,
        Self::DependencyClosure,
        Self::ReplayReproducibility,
        Self::ReleaseGate,
        Self::LimitationsAndNegativeEvidence,
    ];

    const fn key(self) -> &'static str {
        match self {
            Self::PackageAndProvenance => "package_and_provenance",
            Self::DependencyClosure => "dependency_closure",
            Self::ReplayReproducibility => "replay_reproducibility",
            Self::ReleaseGate => "release_gate",
            Self::LimitationsAndNegativeEvidence => "limitations_and_negative_evidence",
        }
    }

    const fn prompt(self) -> &'static str {
        match self {
            Self::PackageAndProvenance => {
                "Review the packaged manifest, source provenance, and modality coverage."
            }
            Self::DependencyClosure => {
                "Review transitive dependencies, required program coverage, and closure findings."
            }
            Self::ReplayReproducibility => {
                "Review replay coverage, exact-hash results, divergences, and unavailable tasks."
            }
            Self::ReleaseGate => {
                "Review release blockers, warnings, independent review requirements, and remediations."
            }
            Self::LimitationsAndNegativeEvidence => {
                "Review declared limitations, uncertainty, omissions, and negative evidence."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseReviewRole {
    ReproducibilitySteward,
    DataSteward,
    MethodsReviewer,
    IndependentReviewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChecklistDecision {
    Acknowledge,
    NeedsAction,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseChecklistResponse {
    pub item_commitment: ContentHash,
    pub decision: ReleaseChecklistDecision,
    /// Opaque commitment to the review response or requested correction evidence.
    pub response_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalReleaseReviewPacketRequest {
    pub workflow: GliomaLocalReleaseWorkflow,
    /// Caller-supplied opaque reviewer identity commitment. It is not authenticated here.
    pub reviewer_commitment: ContentHash,
    pub reviewer_role: ReleaseReviewRole,
    /// Caller assertion only; no reviewer credential or independence check is performed.
    pub independent_reviewer_claim: bool,
    /// Omitted checklist responses remain explicitly pending in the packet.
    pub responses: Vec<ReleaseChecklistResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseReviewChecklistItem {
    pub item_key: String,
    pub category: ReleaseReviewCategory,
    pub prompt: String,
    pub evidence_available: bool,
    pub evidence_digest: Option<ContentHash>,
    pub item_commitment: ContentHash,
    pub decision: Option<ReleaseChecklistDecision>,
    pub response_digest: Option<ContentHash>,
    pub previous_digest: ContentHash,
    pub entry_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseReviewPacketCounts {
    pub required_items: u16,
    pub acknowledged: u16,
    pub needs_action: u16,
    pub unresolved: u16,
    pub pending: u16,
    pub evidence_unavailable: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalReleaseReviewPacketDisposition {
    ReviewPending,
    ReviewComplete,
    ActionRequired,
    Unresolved,
    Hold,
    Blocked,
    NonReproducible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalReleaseReviewPacket {
    pub feature_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub research_id: String,
    pub study_id: String,
    pub workflow_digest: ContentHash,
    pub workflow_disposition: GliomaLocalReleaseWorkflowDisposition,
    pub reviewer_commitment: ContentHash,
    pub reviewer_role: ReleaseReviewRole,
    pub independent_reviewer_claim: bool,
    /// Always false: reviewer identity and role claims are caller-supplied.
    pub reviewer_authenticated: bool,
    /// Always false: completing this checklist does not satisfy a release/signing authorization.
    pub release_authorized: bool,
    /// Always false: this workbench never signs or publishes research objects.
    pub signed_or_published: bool,
    pub checklist: Vec<ReleaseReviewChecklistItem>,
    pub counts: ReleaseReviewPacketCounts,
    pub disposition: LocalReleaseReviewPacketDisposition,
    pub next_operator_action: String,
    pub chain_head: ContentHash,
    pub packet_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LocalReleaseReviewPacketError {
    #[error("local release review request is invalid: {0}")]
    InvalidRequest(String),
    #[error("local release workflow is invalid: {0}")]
    Workflow(#[from] GliomaLocalReleaseWorkflowError),
    #[error("local release review packet is invalid: {0}")]
    InvalidOutput(String),
    #[error("local release review digest failed: {0}")]
    Digest(String),
}

fn digest<T: Serialize>(value: &T) -> Result<ContentHash, LocalReleaseReviewPacketError> {
    ContentHash::of_serializable(value)
        .map_err(|error| LocalReleaseReviewPacketError::Digest(error.to_string()))
}

fn evidence_digest(
    workflow: &GliomaLocalReleaseWorkflow,
    category: ReleaseReviewCategory,
) -> Result<Option<ContentHash>, LocalReleaseReviewPacketError> {
    let evidence = match category {
        ReleaseReviewCategory::PackageAndProvenance => Some(workflow.bundle.digest.clone()),
        ReleaseReviewCategory::DependencyClosure => {
            Some(workflow.dependency_closure.digest.clone())
        }
        ReleaseReviewCategory::ReplayReproducibility => workflow
            .campaign
            .as_ref()
            .map(|campaign| campaign.digest.clone()),
        ReleaseReviewCategory::ReleaseGate => {
            workflow.gate.as_ref().map(|gate| gate.digest.clone())
        }
        ReleaseReviewCategory::LimitationsAndNegativeEvidence => Some(digest(&(
            &workflow.bundle.limitations,
            &workflow.bundle.negative_evidence,
            &workflow.negative_evidence,
            &workflow.uncertainty,
        ))?),
    };
    Ok(evidence)
}

fn item_commitment(
    workflow_digest: &ContentHash,
    category: ReleaseReviewCategory,
    evidence: &Option<ContentHash>,
) -> Result<ContentHash, LocalReleaseReviewPacketError> {
    digest(&(
        FEATURE_ID,
        OUTPUT_SCHEMA,
        workflow_digest,
        category,
        category.key(),
        category.prompt(),
        evidence,
    ))
}

fn packet_seed(
    request: &LocalReleaseReviewPacketRequest,
) -> Result<ContentHash, LocalReleaseReviewPacketError> {
    digest(&(
        FEATURE_ID,
        INPUT_SCHEMA,
        OUTPUT_SCHEMA,
        &request.workflow.research_id,
        &request.workflow.study_id,
        &request.workflow.digest,
        &request.reviewer_commitment,
        request.reviewer_role,
        request.independent_reviewer_claim,
    ))
}

fn item_entry_digest(
    item: &ReleaseReviewChecklistItem,
) -> Result<ContentHash, LocalReleaseReviewPacketError> {
    digest(&(
        FEATURE_ID,
        OUTPUT_SCHEMA,
        &item.item_key,
        item.category,
        &item.prompt,
        item.evidence_available,
        &item.evidence_digest,
        &item.item_commitment,
        item.decision,
        &item.response_digest,
        &item.previous_digest,
    ))
}

fn packet_digest(
    packet: &LocalReleaseReviewPacket,
) -> Result<ContentHash, LocalReleaseReviewPacketError> {
    digest(&(
        (
            &packet.feature_id,
            &packet.input_schema,
            &packet.output_schema,
            &packet.research_id,
            &packet.study_id,
            &packet.workflow_digest,
            packet.workflow_disposition,
            &packet.reviewer_commitment,
            packet.reviewer_role,
            packet.independent_reviewer_claim,
        ),
        (
            packet.reviewer_authenticated,
            packet.release_authorized,
            packet.signed_or_published,
            &packet.checklist,
            &packet.counts,
            packet.disposition,
            &packet.next_operator_action,
            &packet.chain_head,
        ),
    ))
}

fn counts(checklist: &[ReleaseReviewChecklistItem]) -> ReleaseReviewPacketCounts {
    let count = |decision| {
        checklist
            .iter()
            .filter(|item| item.decision == Some(decision))
            .count() as u16
    };
    ReleaseReviewPacketCounts {
        required_items: checklist.len() as u16,
        acknowledged: count(ReleaseChecklistDecision::Acknowledge),
        needs_action: count(ReleaseChecklistDecision::NeedsAction),
        unresolved: count(ReleaseChecklistDecision::Unresolved),
        pending: checklist
            .iter()
            .filter(|item| item.decision.is_none())
            .count() as u16,
        evidence_unavailable: checklist
            .iter()
            .filter(|item| !item.evidence_available)
            .count() as u16,
    }
}

fn disposition(
    workflow: GliomaLocalReleaseWorkflowDisposition,
    counts: &ReleaseReviewPacketCounts,
) -> LocalReleaseReviewPacketDisposition {
    match workflow {
        GliomaLocalReleaseWorkflowDisposition::NonReproducible => {
            LocalReleaseReviewPacketDisposition::NonReproducible
        }
        GliomaLocalReleaseWorkflowDisposition::Blocked => {
            LocalReleaseReviewPacketDisposition::Blocked
        }
        GliomaLocalReleaseWorkflowDisposition::Hold => LocalReleaseReviewPacketDisposition::Hold,
        GliomaLocalReleaseWorkflowDisposition::Unresolved => {
            LocalReleaseReviewPacketDisposition::Unresolved
        }
        GliomaLocalReleaseWorkflowDisposition::ReadyForSigning => {
            if counts.needs_action > 0 {
                LocalReleaseReviewPacketDisposition::ActionRequired
            } else if counts.unresolved > 0 {
                LocalReleaseReviewPacketDisposition::Unresolved
            } else if counts.pending > 0 || counts.evidence_unavailable > 0 {
                LocalReleaseReviewPacketDisposition::ReviewPending
            } else {
                LocalReleaseReviewPacketDisposition::ReviewComplete
            }
        }
    }
}

fn next_action(disposition: LocalReleaseReviewPacketDisposition) -> &'static str {
    match disposition {
        LocalReleaseReviewPacketDisposition::ReviewPending => {
            "review each available checklist item and submit a digest-bound response"
        }
        LocalReleaseReviewPacketDisposition::ReviewComplete => {
            "route the completed checklist to the institution's P11-F20 approval and signing workflow"
        }
        LocalReleaseReviewPacketDisposition::ActionRequired => {
            "resolve requested corrections, regenerate the local workflow, and compile a fresh packet"
        }
        LocalReleaseReviewPacketDisposition::Unresolved => {
            "restore unavailable evidence or resolve reviewer uncertainty before release review"
        }
        LocalReleaseReviewPacketDisposition::Hold => {
            "resolve upstream workflow holds before another human review packet is compiled"
        }
        LocalReleaseReviewPacketDisposition::Blocked => {
            "repair blocked packaging or dependency closure before requesting human review"
        }
        LocalReleaseReviewPacketDisposition::NonReproducible => {
            "preserve the replay divergence and do not make a reproducibility claim"
        }
    }
}

/// Compile a fixed local-steward checklist and bind supplied responses to exact evidence items.
pub fn compile_glioma_local_release_review_packet(
    request: &LocalReleaseReviewPacketRequest,
) -> Result<LocalReleaseReviewPacket, LocalReleaseReviewPacketError> {
    request.workflow.validate()?;
    let workflow_bytes = serde_json::to_vec(&request.workflow)
        .map_err(|error| LocalReleaseReviewPacketError::InvalidRequest(error.to_string()))?
        .len();
    if workflow_bytes > MAX_WORKFLOW_BYTES || request.responses.len() > CHECKLIST_ITEM_COUNT {
        return Err(LocalReleaseReviewPacketError::InvalidRequest(
            "workflow, evidence, or response count exceeds the local review workbench bounds"
                .into(),
        ));
    }

    let mut base_items = Vec::with_capacity(CHECKLIST_ITEM_COUNT);
    for category in ReleaseReviewCategory::ORDERED {
        let evidence = evidence_digest(&request.workflow, category)?;
        base_items.push((category, evidence));
    }
    let item_commitments = base_items
        .iter()
        .map(|(category, evidence)| {
            item_commitment(&request.workflow.digest, *category, evidence)
                .map(|commitment| (*category, commitment, evidence.is_some()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut responses = BTreeMap::new();
    let mut response_keys = BTreeSet::new();
    for response in &request.responses {
        let matching = item_commitments
            .iter()
            .find(|(_, commitment, _)| commitment == &response.item_commitment);
        let Some((_, _, evidence_available)) = matching else {
            return Err(LocalReleaseReviewPacketError::InvalidRequest(
                "every response must reference one checklist item in this workflow packet".into(),
            ));
        };
        if !response_keys.insert(response.item_commitment.clone())
            || (response.decision == ReleaseChecklistDecision::Acknowledge && !evidence_available)
        {
            return Err(LocalReleaseReviewPacketError::InvalidRequest(
                "checklist responses must be unique and cannot acknowledge unavailable evidence"
                    .into(),
            ));
        }
        responses.insert(response.item_commitment.clone(), response);
    }

    let mut previous_digest = packet_seed(request)?;
    let mut checklist = Vec::with_capacity(CHECKLIST_ITEM_COUNT);
    for ((category, evidence), (_, commitment, available)) in
        base_items.into_iter().zip(item_commitments)
    {
        let response = responses.get(&commitment).copied();
        let mut item = ReleaseReviewChecklistItem {
            item_key: category.key().into(),
            category,
            prompt: category.prompt().into(),
            evidence_available: available,
            evidence_digest: evidence,
            item_commitment: commitment,
            decision: response.map(|item| item.decision),
            response_digest: response.map(|item| item.response_digest.clone()),
            previous_digest: previous_digest.clone(),
            entry_digest: ContentHash::of_bytes(b"unsealed-glioma-release-review-item"),
        };
        item.entry_digest = item_entry_digest(&item)?;
        previous_digest = item.entry_digest.clone();
        checklist.push(item);
    }
    let counts = counts(&checklist);
    let packet_disposition = disposition(request.workflow.disposition, &counts);
    let mut packet = LocalReleaseReviewPacket {
        feature_id: FEATURE_ID.into(),
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: request.workflow.research_id.clone(),
        study_id: request.workflow.study_id.clone(),
        workflow_digest: request.workflow.digest.clone(),
        workflow_disposition: request.workflow.disposition,
        reviewer_commitment: request.reviewer_commitment.clone(),
        reviewer_role: request.reviewer_role,
        independent_reviewer_claim: request.independent_reviewer_claim,
        reviewer_authenticated: false,
        release_authorized: false,
        signed_or_published: false,
        checklist,
        counts,
        disposition: packet_disposition,
        next_operator_action: next_action(packet_disposition).into(),
        chain_head: previous_digest,
        packet_digest: ContentHash::of_bytes(b"unsealed-glioma-release-review-packet"),
    };
    packet.packet_digest = packet_digest(&packet)?;
    packet.validate()?;
    Ok(packet)
}

impl LocalReleaseReviewPacket {
    /// Validate item order, response binding, reviewer limitations, and content-addressed chains.
    pub fn validate(&self) -> Result<(), LocalReleaseReviewPacketError> {
        if self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.research_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.reviewer_authenticated
            || self.release_authorized
            || self.signed_or_published
            || self.checklist.len() != CHECKLIST_ITEM_COUNT
            || self.next_operator_action != next_action(self.disposition)
        {
            return Err(LocalReleaseReviewPacketError::InvalidOutput(
                "packet identity, checklist cardinality, authorization flags, or operator action is invalid".into(),
            ));
        }
        let mut previous_digest = digest(&(
            FEATURE_ID,
            INPUT_SCHEMA,
            OUTPUT_SCHEMA,
            &self.research_id,
            &self.study_id,
            &self.workflow_digest,
            &self.reviewer_commitment,
            self.reviewer_role,
            self.independent_reviewer_claim,
        ))?;
        for (item, category) in self.checklist.iter().zip(ReleaseReviewCategory::ORDERED) {
            let expected_commitment =
                item_commitment(&self.workflow_digest, category, &item.evidence_digest)?;
            if item.category != category
                || item.item_key != category.key()
                || item.prompt != category.prompt()
                || item.evidence_available != item.evidence_digest.is_some()
                || item.item_commitment != expected_commitment
                || item.previous_digest != previous_digest
                || item.entry_digest != item_entry_digest(item)?
                || item.decision.is_some() != item.response_digest.is_some()
                || (item.decision == Some(ReleaseChecklistDecision::Acknowledge)
                    && !item.evidence_available)
            {
                return Err(LocalReleaseReviewPacketError::InvalidOutput(
                    "checklist order, evidence availability, response binding, or digest chain is invalid".into(),
                ));
            }
            previous_digest = item.entry_digest.clone();
        }
        let expected_counts = counts(&self.checklist);
        if self.chain_head != previous_digest
            || self.counts != expected_counts
            || self.disposition != disposition(self.workflow_disposition, &expected_counts)
            || self.packet_digest != packet_digest(self)?
        {
            return Err(LocalReleaseReviewPacketError::InvalidOutput(
                "packet counts, disposition, chain head, or digest is inconsistent".into(),
            ));
        }
        Ok(())
    }
}
