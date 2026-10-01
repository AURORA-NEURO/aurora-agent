//! Prospective human-review queue over bounded local release batches.
//!
//! GAF-GLIOMA-P11-F19 joins a validated P11-F15 batch report to available P11-F17 review packets.
//! It preserves deterministic queue order, binds each review packet to the produced job digest,
//! and keeps budget deferrals and execution failures visible. Reviewer commitments are not
//! authenticated, and queue completion does not authorize, sign, or publish a research object.

use super::local_release_workflow::GliomaLocalReleaseWorkflowDisposition;
use super::release_batch::{
    MAX_BATCH_ITEMS, ReleaseBatchDisposition as SourceBatchDisposition,
    ReleaseBatchItemDisposition, ReleaseBatchReport,
};
use super::review_workbench::{LocalReleaseReviewPacket, LocalReleaseReviewPacketDisposition};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = super::release_queue_console::FEATURE_ID;
pub const INPUT_SCHEMA: &str = "GliomaReleaseBatchReviewWorkbenchRequest1@1";
pub const OUTPUT_SCHEMA: &str = "GliomaReleaseBatchReviewWorkbench1@1";
pub const MAX_REVIEW_PACKET_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_TOTAL_REVIEW_PACKET_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseBatchReviewPacketInput {
    pub job_id: String,
    pub packet: LocalReleaseReviewPacket,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseBatchReviewWorkbenchRequest {
    pub batch: ReleaseBatchReport,
    /// Packets can be partial; every submitted job remains in the output in batch order.
    pub review_packets: Vec<ReleaseBatchReviewPacketInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseBatchReviewItemDisposition {
    ReviewPending,
    ReviewComplete,
    ActionRequired,
    Unresolved,
    Hold,
    Blocked,
    NonReproducible,
    DeferredBudget,
    DeferredWorkflowLimit,
    WorkflowFailed,
    NotAttempted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseBatchReviewEvidence {
    pub job_id: String,
    pub priority: u16,
    pub submission_order: u64,
    pub batch_item_disposition: ReleaseBatchItemDisposition,
    pub workflow_digest: Option<ContentHash>,
    pub workflow_disposition: Option<GliomaLocalReleaseWorkflowDisposition>,
    pub review_packet_digest: Option<ContentHash>,
    pub review_disposition: Option<LocalReleaseReviewPacketDisposition>,
    pub disposition: ReleaseBatchReviewItemDisposition,
    pub previous_digest: ContentHash,
    pub entry_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseBatchReviewCounts {
    pub submitted: u16,
    pub workflows_produced: u16,
    pub review_complete: u16,
    pub review_pending: u16,
    pub action_required: u16,
    pub unresolved: u16,
    pub held: u16,
    pub blocked: u16,
    pub non_reproducible: u16,
    pub deferred_budget: u16,
    pub deferred_workflow_limit: u16,
    pub workflow_failed: u16,
    pub not_attempted: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseBatchReviewDisposition {
    ReviewComplete,
    ReviewPending,
    Partial,
    ActionRequired,
    Unresolved,
    Hold,
    Blocked,
    NonReproducible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseBatchReviewWorkbenchReport {
    pub feature_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub batch_id: String,
    pub batch_report_digest: ContentHash,
    pub batch_disposition: SourceBatchDisposition,
    pub budget_accounting_complete: bool,
    /// Entries preserve the already-prioritized order from P11-F15.
    pub evidence: Vec<ReleaseBatchReviewEvidence>,
    pub counts: ReleaseBatchReviewCounts,
    pub disposition: ReleaseBatchReviewDisposition,
    pub next_operator_action: String,
    /// Always false: reviewer commitments and independence claims are not authenticated here.
    pub reviewer_commitments_authenticated: bool,
    /// Always false: review queue completion does not authorize release.
    pub release_authorized: bool,
    pub chain_head: ContentHash,
    pub report_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseBatchReviewWorkbenchError {
    #[error("release batch review request is invalid: {0}")]
    InvalidRequest(String),
    #[error("release batch report is invalid: {0}")]
    InvalidBatch(String),
    #[error("local review packet is invalid: {0}")]
    InvalidPacket(String),
    #[error("release batch review report is invalid: {0}")]
    InvalidOutput(String),
    #[error("release batch review digest failed: {0}")]
    Digest(String),
}

fn digest<T: Serialize>(value: &T) -> Result<ContentHash, ReleaseBatchReviewWorkbenchError> {
    ContentHash::of_serializable(value)
        .map_err(|error| ReleaseBatchReviewWorkbenchError::Digest(error.to_string()))
}

fn seed_digest(
    batch_id: &str,
    batch_digest: &ContentHash,
    budget_accounting_complete: bool,
) -> Result<ContentHash, ReleaseBatchReviewWorkbenchError> {
    digest(&(
        FEATURE_ID,
        INPUT_SCHEMA,
        OUTPUT_SCHEMA,
        batch_id,
        batch_digest,
        budget_accounting_complete,
    ))
}

fn entry_digest(
    item: &ReleaseBatchReviewEvidence,
) -> Result<ContentHash, ReleaseBatchReviewWorkbenchError> {
    digest(&(
        FEATURE_ID,
        OUTPUT_SCHEMA,
        &item.job_id,
        item.priority,
        item.submission_order,
        item.batch_item_disposition,
        &item.workflow_digest,
        item.workflow_disposition,
        &item.review_packet_digest,
        item.review_disposition,
        item.disposition,
        &item.previous_digest,
    ))
}

fn report_digest(
    report: &ReleaseBatchReviewWorkbenchReport,
) -> Result<ContentHash, ReleaseBatchReviewWorkbenchError> {
    digest(&(
        &report.feature_id,
        &report.input_schema,
        &report.output_schema,
        &report.batch_id,
        &report.batch_report_digest,
        report.batch_disposition,
        report.budget_accounting_complete,
        &report.evidence,
        &report.counts,
        report.disposition,
        &report.next_operator_action,
        report.reviewer_commitments_authenticated,
        report.release_authorized,
        &report.chain_head,
    ))
}

fn produced_disposition(
    workflow: GliomaLocalReleaseWorkflowDisposition,
    review: Option<LocalReleaseReviewPacketDisposition>,
) -> ReleaseBatchReviewItemDisposition {
    match workflow {
        GliomaLocalReleaseWorkflowDisposition::NonReproducible => {
            ReleaseBatchReviewItemDisposition::NonReproducible
        }
        GliomaLocalReleaseWorkflowDisposition::Blocked => {
            ReleaseBatchReviewItemDisposition::Blocked
        }
        GliomaLocalReleaseWorkflowDisposition::Hold => ReleaseBatchReviewItemDisposition::Hold,
        GliomaLocalReleaseWorkflowDisposition::Unresolved => {
            ReleaseBatchReviewItemDisposition::Unresolved
        }
        GliomaLocalReleaseWorkflowDisposition::ReadyForSigning => match review {
            None | Some(LocalReleaseReviewPacketDisposition::ReviewPending) => {
                ReleaseBatchReviewItemDisposition::ReviewPending
            }
            Some(LocalReleaseReviewPacketDisposition::ReviewComplete) => {
                ReleaseBatchReviewItemDisposition::ReviewComplete
            }
            Some(LocalReleaseReviewPacketDisposition::ActionRequired) => {
                ReleaseBatchReviewItemDisposition::ActionRequired
            }
            Some(LocalReleaseReviewPacketDisposition::Unresolved) => {
                ReleaseBatchReviewItemDisposition::Unresolved
            }
            Some(LocalReleaseReviewPacketDisposition::Hold) => {
                ReleaseBatchReviewItemDisposition::Hold
            }
            Some(LocalReleaseReviewPacketDisposition::Blocked) => {
                ReleaseBatchReviewItemDisposition::Blocked
            }
            Some(LocalReleaseReviewPacketDisposition::NonReproducible) => {
                ReleaseBatchReviewItemDisposition::NonReproducible
            }
        },
    }
}

fn item_disposition(
    batch_item: ReleaseBatchItemDisposition,
    workflow: Option<GliomaLocalReleaseWorkflowDisposition>,
    review: Option<LocalReleaseReviewPacketDisposition>,
) -> ReleaseBatchReviewItemDisposition {
    match batch_item {
        ReleaseBatchItemDisposition::WorkflowProduced => produced_disposition(
            workflow.expect("validated produced result includes workflow disposition"),
            review,
        ),
        ReleaseBatchItemDisposition::DeferredBudget => {
            ReleaseBatchReviewItemDisposition::DeferredBudget
        }
        ReleaseBatchItemDisposition::DeferredWorkflowLimit => {
            ReleaseBatchReviewItemDisposition::DeferredWorkflowLimit
        }
        ReleaseBatchItemDisposition::WorkflowFailed => {
            ReleaseBatchReviewItemDisposition::WorkflowFailed
        }
        ReleaseBatchItemDisposition::NotAttemptedAfterFailure => {
            ReleaseBatchReviewItemDisposition::NotAttempted
        }
    }
}

fn counts(evidence: &[ReleaseBatchReviewEvidence]) -> ReleaseBatchReviewCounts {
    let count = |disposition| {
        evidence
            .iter()
            .filter(|item| item.disposition == disposition)
            .count() as u16
    };
    ReleaseBatchReviewCounts {
        submitted: evidence.len() as u16,
        workflows_produced: evidence
            .iter()
            .filter(|item| {
                item.batch_item_disposition == ReleaseBatchItemDisposition::WorkflowProduced
            })
            .count() as u16,
        review_complete: count(ReleaseBatchReviewItemDisposition::ReviewComplete),
        review_pending: count(ReleaseBatchReviewItemDisposition::ReviewPending),
        action_required: count(ReleaseBatchReviewItemDisposition::ActionRequired),
        unresolved: count(ReleaseBatchReviewItemDisposition::Unresolved),
        held: count(ReleaseBatchReviewItemDisposition::Hold),
        blocked: count(ReleaseBatchReviewItemDisposition::Blocked),
        non_reproducible: count(ReleaseBatchReviewItemDisposition::NonReproducible),
        deferred_budget: count(ReleaseBatchReviewItemDisposition::DeferredBudget),
        deferred_workflow_limit: count(ReleaseBatchReviewItemDisposition::DeferredWorkflowLimit),
        workflow_failed: count(ReleaseBatchReviewItemDisposition::WorkflowFailed),
        not_attempted: count(ReleaseBatchReviewItemDisposition::NotAttempted),
    }
}

fn overall_disposition(
    counts: &ReleaseBatchReviewCounts,
    batch_disposition: SourceBatchDisposition,
    budget_accounting_complete: bool,
) -> ReleaseBatchReviewDisposition {
    if counts.non_reproducible > 0 {
        ReleaseBatchReviewDisposition::NonReproducible
    } else if counts.blocked > 0 {
        ReleaseBatchReviewDisposition::Blocked
    } else if counts.held > 0 {
        ReleaseBatchReviewDisposition::Hold
    } else if counts.unresolved > 0
        || counts.workflow_failed > 0
        || counts.not_attempted > 0
        || !budget_accounting_complete
    {
        ReleaseBatchReviewDisposition::Unresolved
    } else if counts.action_required > 0 {
        ReleaseBatchReviewDisposition::ActionRequired
    } else if batch_disposition == SourceBatchDisposition::Partial {
        ReleaseBatchReviewDisposition::Partial
    } else if counts.review_pending > 0 {
        ReleaseBatchReviewDisposition::ReviewPending
    } else {
        ReleaseBatchReviewDisposition::ReviewComplete
    }
}

fn next_action(disposition: ReleaseBatchReviewDisposition) -> &'static str {
    match disposition {
        ReleaseBatchReviewDisposition::ReviewComplete => {
            "route each completed job packet through the institution's P11-F20 approval and signing workflow"
        }
        ReleaseBatchReviewDisposition::ReviewPending => {
            "complete every pending local checklist before batch signing review"
        }
        ReleaseBatchReviewDisposition::Partial => {
            "resolve budget or workflow-limit deferrals and submit a follow-up release batch"
        }
        ReleaseBatchReviewDisposition::ActionRequired => {
            "resolve requested workflow corrections and compile fresh local review packets"
        }
        ReleaseBatchReviewDisposition::Unresolved => {
            "resolve failed or unaccounted workflows and preserve every unattempted queue item"
        }
        ReleaseBatchReviewDisposition::Hold => {
            "resolve held local release reviews before batch signing review"
        }
        ReleaseBatchReviewDisposition::Blocked => {
            "repair blocked packages or dependency closures before review"
        }
        ReleaseBatchReviewDisposition::NonReproducible => {
            "preserve divergent workflow outcomes and withhold reproducibility claims"
        }
    }
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-@".contains(&byte))
}

fn workflow_disposition_matches(
    workflow: GliomaLocalReleaseWorkflowDisposition,
    packet: &LocalReleaseReviewPacket,
) -> bool {
    workflow == packet.workflow_disposition
}

fn review_disposition_matches_workflow(
    workflow: GliomaLocalReleaseWorkflowDisposition,
    review: LocalReleaseReviewPacketDisposition,
) -> bool {
    match workflow {
        GliomaLocalReleaseWorkflowDisposition::ReadyForSigning => matches!(
            review,
            LocalReleaseReviewPacketDisposition::ReviewPending
                | LocalReleaseReviewPacketDisposition::ReviewComplete
                | LocalReleaseReviewPacketDisposition::ActionRequired
                | LocalReleaseReviewPacketDisposition::Unresolved
        ),
        GliomaLocalReleaseWorkflowDisposition::Hold => {
            review == LocalReleaseReviewPacketDisposition::Hold
        }
        GliomaLocalReleaseWorkflowDisposition::Unresolved => {
            review == LocalReleaseReviewPacketDisposition::Unresolved
        }
        GliomaLocalReleaseWorkflowDisposition::Blocked => {
            review == LocalReleaseReviewPacketDisposition::Blocked
        }
        GliomaLocalReleaseWorkflowDisposition::NonReproducible => {
            review == LocalReleaseReviewPacketDisposition::NonReproducible
        }
    }
}

/// Reconcile bounded local review packets against a deterministic P11-F15 batch queue.
pub fn reconcile_glioma_release_batch_review_workbench(
    request: &ReleaseBatchReviewWorkbenchRequest,
) -> Result<ReleaseBatchReviewWorkbenchReport, ReleaseBatchReviewWorkbenchError> {
    request
        .batch
        .validate()
        .map_err(|error| ReleaseBatchReviewWorkbenchError::InvalidBatch(error.to_string()))?;
    if request.batch.results.len() > MAX_BATCH_ITEMS
        || request.review_packets.len() > MAX_BATCH_ITEMS
    {
        return Err(ReleaseBatchReviewWorkbenchError::InvalidRequest(
            "release batch or review packet count exceeds the workbench limit".into(),
        ));
    }
    let batch_by_job = request
        .batch
        .results
        .iter()
        .map(|item| (item.job_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let mut packets = BTreeMap::new();
    let mut total_packet_bytes = 0_usize;
    for input in &request.review_packets {
        if !valid_id(&input.job_id) {
            return Err(ReleaseBatchReviewWorkbenchError::InvalidRequest(
                "review packet job IDs must be valid bounded identifiers".into(),
            ));
        }
        let result = batch_by_job.get(input.job_id.as_str()).ok_or_else(|| {
            ReleaseBatchReviewWorkbenchError::InvalidRequest(
                "every review packet must map to a submitted batch job".into(),
            )
        })?;
        input
            .packet
            .validate()
            .map_err(|error| ReleaseBatchReviewWorkbenchError::InvalidPacket(error.to_string()))?;
        let bytes = serde_json::to_vec(&input.packet)
            .map_err(|error| ReleaseBatchReviewWorkbenchError::InvalidRequest(error.to_string()))?
            .len();
        total_packet_bytes = total_packet_bytes.saturating_add(bytes);
        if bytes > MAX_REVIEW_PACKET_BYTES
            || total_packet_bytes > MAX_TOTAL_REVIEW_PACKET_BYTES
            || result.disposition != ReleaseBatchItemDisposition::WorkflowProduced
            || result.workflow_digest.as_ref() != Some(&input.packet.workflow_digest)
            || result
                .workflow_disposition
                .is_none_or(|workflow| !workflow_disposition_matches(workflow, &input.packet))
            || packets
                .insert(input.job_id.clone(), &input.packet)
                .is_some()
        {
            return Err(ReleaseBatchReviewWorkbenchError::InvalidRequest(
                "packet job, workflow digest/disposition, uniqueness, or payload bound is invalid"
                    .into(),
            ));
        }
    }

    let mut previous_digest = seed_digest(
        &request.batch.batch_id,
        &request.batch.digest,
        request.batch.budget_accounting_complete,
    )?;
    let mut evidence = Vec::with_capacity(request.batch.results.len());
    for result in &request.batch.results {
        let packet = packets.get(&result.job_id);
        let mut item = ReleaseBatchReviewEvidence {
            job_id: result.job_id.clone(),
            priority: result.priority,
            submission_order: result.submission_order,
            batch_item_disposition: result.disposition,
            workflow_digest: result.workflow_digest.clone(),
            workflow_disposition: result.workflow_disposition,
            review_packet_digest: packet.map(|packet| packet.packet_digest.clone()),
            review_disposition: packet.map(|packet| packet.disposition),
            disposition: item_disposition(
                result.disposition,
                result.workflow_disposition,
                packet.map(|packet| packet.disposition),
            ),
            previous_digest: previous_digest.clone(),
            entry_digest: ContentHash::of_bytes(b"unsealed-glioma-release-batch-review-entry"),
        };
        item.entry_digest = entry_digest(&item)?;
        previous_digest = item.entry_digest.clone();
        evidence.push(item);
    }
    let counts = counts(&evidence);
    let disposition = overall_disposition(
        &counts,
        request.batch.disposition,
        request.batch.budget_accounting_complete,
    );
    let mut report = ReleaseBatchReviewWorkbenchReport {
        feature_id: FEATURE_ID.into(),
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        batch_id: request.batch.batch_id.clone(),
        batch_report_digest: request.batch.digest.clone(),
        batch_disposition: request.batch.disposition,
        budget_accounting_complete: request.batch.budget_accounting_complete,
        evidence,
        counts,
        disposition,
        next_operator_action: next_action(disposition).into(),
        reviewer_commitments_authenticated: false,
        release_authorized: false,
        chain_head: previous_digest,
        report_digest: ContentHash::of_bytes(b"unsealed-glioma-release-batch-review-report"),
    };
    report.report_digest = report_digest(&report)?;
    report.validate()?;
    Ok(report)
}

impl ReleaseBatchReviewWorkbenchReport {
    /// Validate queue ordering, packet/workflow state agreement, counts, and digests.
    pub fn validate(&self) -> Result<(), ReleaseBatchReviewWorkbenchError> {
        if self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_id(&self.batch_id)
            || self.evidence.is_empty()
            || self.evidence.len() > MAX_BATCH_ITEMS
            || self.reviewer_commitments_authenticated
            || self.release_authorized
            || self.next_operator_action != next_action(self.disposition)
        {
            return Err(ReleaseBatchReviewWorkbenchError::InvalidOutput(
                "report identity, queue bounds, authorization flags, or operator action is invalid"
                    .into(),
            ));
        }
        let mut previous_digest = seed_digest(
            &self.batch_id,
            &self.batch_report_digest,
            self.budget_accounting_complete,
        )?;
        let mut job_ids = BTreeSet::new();
        let mut previous_order: Option<(Reverse<u16>, u64, String)> = None;
        for item in &self.evidence {
            let order = (
                Reverse(item.priority),
                item.submission_order,
                item.job_id.clone(),
            );
            if !valid_id(&item.job_id)
                || !job_ids.insert(item.job_id.clone())
                || previous_order
                    .as_ref()
                    .is_some_and(|previous| previous >= &order)
                || item.previous_digest != previous_digest
                || item.entry_digest != entry_digest(item)?
            {
                return Err(ReleaseBatchReviewWorkbenchError::InvalidOutput(
                    "job uniqueness/order or review queue digest chain is invalid".into(),
                ));
            }
            let produced =
                item.batch_item_disposition == ReleaseBatchItemDisposition::WorkflowProduced;
            let fields_present = if produced {
                item.workflow_digest.is_some() && item.workflow_disposition.is_some()
            } else {
                item.workflow_digest.is_none()
                    && item.workflow_disposition.is_none()
                    && item.review_packet_digest.is_none()
                    && item.review_disposition.is_none()
            };
            let packet_partition =
                item.review_packet_digest.is_some() == item.review_disposition.is_some();
            if !fields_present
                || !packet_partition
                || (item.review_packet_digest.is_some()
                    && item.batch_item_disposition != ReleaseBatchItemDisposition::WorkflowProduced)
                || item.review_disposition.is_some_and(|review| {
                    item.workflow_disposition.is_none_or(|workflow| {
                        !review_disposition_matches_workflow(workflow, review)
                    })
                })
            {
                return Err(ReleaseBatchReviewWorkbenchError::InvalidOutput(
                    "batch, workflow, and review packet presence partition is invalid".into(),
                ));
            }
            if item.disposition
                != item_disposition(
                    item.batch_item_disposition,
                    item.workflow_disposition,
                    item.review_disposition,
                )
            {
                return Err(ReleaseBatchReviewWorkbenchError::InvalidOutput(
                    "job review disposition does not match its batch/workflow evidence".into(),
                ));
            }
            previous_order = Some(order);
            previous_digest = item.entry_digest.clone();
        }
        let expected_counts = counts(&self.evidence);
        if self.chain_head != previous_digest
            || self.counts != expected_counts
            || self.disposition
                != overall_disposition(
                    &expected_counts,
                    self.batch_disposition,
                    self.budget_accounting_complete,
                )
            || self.report_digest != report_digest(self)?
        {
            return Err(ReleaseBatchReviewWorkbenchError::InvalidOutput(
                "review queue counts, disposition, chain head, or report digest is inconsistent"
                    .into(),
            ));
        }
        Ok(())
    }
}
