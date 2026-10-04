//! Longitudinal change control for federated research-object release snapshots.
//!
//! GAF-GLIOMA-P11-F16 coordinates validated P11-F14 portfolio reports over ordered release
//! epochs. It tracks per-study manifest continuity and requires caller-supplied, digest-bound
//! change reviews before a changed snapshot can be considered ready for signing review. Reviewer
//! commitments and evidence are opaque inputs: this module does not authenticate them, sign,
//! publish, transfer data, or contact federation members.

use super::multistudy_release::{
    MAX_STUDIES, MultiStudyEvidenceDisposition, MultiStudyReleaseReport,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = super::federated_release_bundle::FEATURE_ID;
pub const INPUT_SCHEMA: &str = "GliomaFederatedContinualReleaseRequest1@1";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedContinualRelease1@1";
pub const MIN_EPOCHS: usize = 2;
pub const MAX_EPOCHS: usize = 32;
pub const MAX_REVIEWS_PER_CHANGE: usize = 16;
pub const MAX_TOTAL_REVIEWS: usize = 4_096;
pub const MAX_REPORT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_TOTAL_REPORT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReleaseEpoch {
    pub epoch_index: u32,
    pub portfolio: MultiStudyReleaseReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChangeReviewDecision {
    Approve,
    Hold,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReleaseChangeReview {
    pub epoch_index: u32,
    pub study_commitment: ContentHash,
    pub previous_manifest_digest: ContentHash,
    pub current_manifest_digest: ContentHash,
    /// Caller-supplied opaque reviewer commitment; this module does not authenticate it.
    pub reviewer_commitment: ContentHash,
    /// Commitment to the review record/evidence supplied by the accountable institution.
    pub evidence_digest: ContentHash,
    pub decision: ReleaseChangeReviewDecision,
    pub independent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualReleaseRequest {
    pub research_id: String,
    /// Strictly ordered release epochs. Missing epoch reports remain visible in the output.
    pub expected_epoch_order: Vec<u32>,
    /// Stable sorted cohort membership expected in every P11-F14 portfolio report.
    pub expected_study_commitments: Vec<ContentHash>,
    pub min_independent_change_approvals: u8,
    pub epochs: Vec<FederatedReleaseEpoch>,
    pub change_reviews: Vec<FederatedReleaseChangeReview>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedReleaseChangeState {
    Baseline,
    Unchanged,
    Approved,
    ReviewRequired,
    Held,
    InsufficientIndependentReview,
    ContinuityGap,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReleaseReviewEvidence {
    pub epoch_index: u32,
    pub study_commitment: ContentHash,
    pub previous_manifest_digest: ContentHash,
    pub current_manifest_digest: ContentHash,
    pub reviewer_commitment: ContentHash,
    pub evidence_digest: ContentHash,
    pub decision: ReleaseChangeReviewDecision,
    pub independent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReleaseStudyTransition {
    pub epoch_index: u32,
    pub study_commitment: ContentHash,
    pub workflow_disposition: MultiStudyEvidenceDisposition,
    pub previous_manifest_digest: Option<ContentHash>,
    pub current_manifest_digest: Option<ContentHash>,
    pub change_state: FederatedReleaseChangeState,
    pub independent_approvals: u16,
    pub review_holds: u16,
    pub non_independent_approvals: u16,
    pub reviews: Vec<FederatedReleaseReviewEvidence>,
    pub previous_digest: ContentHash,
    pub entry_digest: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedReleaseEpochDisposition {
    ReadyForSigning,
    Insufficient,
    ChangeReviewRequired,
    Hold,
    Unresolved,
    Blocked,
    NonReproducible,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReleaseEpochCounts {
    pub expected_studies: u16,
    pub ready_for_signing: u16,
    pub held: u16,
    pub unresolved: u16,
    pub blocked: u16,
    pub non_reproducible: u16,
    pub missing: u16,
    pub changed_manifests: u16,
    pub approved_changes: u16,
    pub changes_requiring_review: u16,
    pub held_changes: u16,
    pub continuity_gaps: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReleaseEpochSummary {
    pub epoch_index: u32,
    pub portfolio_digest: Option<ContentHash>,
    pub counts: FederatedReleaseEpochCounts,
    pub disposition: FederatedReleaseEpochDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContinualReleaseDisposition {
    ReadyForSigning,
    Insufficient,
    ChangeReviewRequired,
    Hold,
    Unresolved,
    Blocked,
    NonReproducible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualReleaseReport {
    pub feature_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub research_id: String,
    pub expected_epoch_order: Vec<u32>,
    pub expected_study_commitments: Vec<ContentHash>,
    pub min_independent_change_approvals: u8,
    /// Always false: site commitments are caller-supplied and are not authenticated here.
    pub site_commitments_authenticated: bool,
    /// Always false: reviewer commitments and independence claims are caller-supplied.
    pub reviewer_commitments_authenticated: bool,
    /// Always false: this feature only reconciles evidence and never signs or publishes.
    pub signed_or_published: bool,
    /// Contains opaque study/reviewer commitments, manifest digests, and bounded workflow states.
    pub transitions: Vec<FederatedReleaseStudyTransition>,
    pub epochs: Vec<FederatedReleaseEpochSummary>,
    pub disposition: FederatedContinualReleaseDisposition,
    pub next_operator_action: String,
    pub chain_head: ContentHash,
    pub report_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedContinualReleaseError {
    #[error("federated continual release request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated continual release portfolio is invalid: {0}")]
    InvalidPortfolio(String),
    #[error("federated continual release report is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated continual release digest failed: {0}")]
    Digest(String),
}

fn digest<T: Serialize>(value: &T) -> Result<ContentHash, FederatedContinualReleaseError> {
    ContentHash::of_serializable(value)
        .map_err(|error| FederatedContinualReleaseError::Digest(error.to_string()))
}

fn report_seed(
    report: &FederatedContinualReleaseReport,
) -> Result<ContentHash, FederatedContinualReleaseError> {
    digest(&(
        FEATURE_ID,
        INPUT_SCHEMA,
        OUTPUT_SCHEMA,
        &report.research_id,
        &report.expected_epoch_order,
        &report.expected_study_commitments,
        report.min_independent_change_approvals,
    ))
}

fn transition_digest(
    transition: &FederatedReleaseStudyTransition,
) -> Result<ContentHash, FederatedContinualReleaseError> {
    digest(&(
        FEATURE_ID,
        OUTPUT_SCHEMA,
        transition.epoch_index,
        &transition.study_commitment,
        transition.workflow_disposition,
        &transition.previous_manifest_digest,
        &transition.current_manifest_digest,
        transition.change_state,
        transition.independent_approvals,
        transition.review_holds,
        transition.non_independent_approvals,
        &transition.reviews,
        &transition.previous_digest,
    ))
}

fn report_digest(
    report: &FederatedContinualReleaseReport,
) -> Result<ContentHash, FederatedContinualReleaseError> {
    digest(&(
        &report.feature_id,
        &report.input_schema,
        &report.output_schema,
        &report.research_id,
        &report.expected_epoch_order,
        &report.expected_study_commitments,
        report.min_independent_change_approvals,
        report.site_commitments_authenticated,
        report.reviewer_commitments_authenticated,
        report.signed_or_published,
        &report.transitions,
        &report.epochs,
        report.disposition,
        &report.next_operator_action,
        &report.chain_head,
    ))
}

fn valid_request(
    request: &FederatedContinualReleaseRequest,
) -> Result<(), FederatedContinualReleaseError> {
    if request.research_id.trim().is_empty()
        || request.research_id.len() > 256
        || !(MIN_EPOCHS..=MAX_EPOCHS).contains(&request.expected_epoch_order.len())
        || request
            .expected_epoch_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || !(2..=MAX_STUDIES).contains(&request.expected_study_commitments.len())
        || request
            .expected_study_commitments
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.min_independent_change_approvals == 0
        || usize::from(request.min_independent_change_approvals) > MAX_REVIEWS_PER_CHANGE
        || request.epochs.len() > request.expected_epoch_order.len()
        || request.change_reviews.len() > MAX_TOTAL_REVIEWS
    {
        return Err(FederatedContinualReleaseError::InvalidRequest(
            "research identity, ordered epochs, stable sorted cohort, bounded review quorum, and bounded reports are required".into(),
        ));
    }

    let expected_epochs = request
        .expected_epoch_order
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut previous_epoch = None;
    let mut total_report_bytes = 0_usize;
    for epoch in &request.epochs {
        if !expected_epochs.contains(&epoch.epoch_index)
            || previous_epoch.is_some_and(|previous| previous >= epoch.epoch_index)
            || epoch.portfolio.research_id != request.research_id
            || epoch.portfolio.expected_study_commitments != request.expected_study_commitments
        {
            return Err(FederatedContinualReleaseError::InvalidRequest(
                "portfolio epochs must be unique and ordered, with one research identity and the exact expected cohort".into(),
            ));
        }
        epoch
            .portfolio
            .validate()
            .map_err(|error| FederatedContinualReleaseError::InvalidPortfolio(error.to_string()))?;
        let bytes = serde_json::to_vec(&epoch.portfolio)
            .map_err(|error| FederatedContinualReleaseError::InvalidRequest(error.to_string()))?
            .len();
        if bytes > MAX_REPORT_BYTES {
            return Err(FederatedContinualReleaseError::InvalidRequest(
                "a portfolio report exceeds the per-report aggregation bound".into(),
            ));
        }
        total_report_bytes = total_report_bytes.saturating_add(bytes);
        if total_report_bytes > MAX_TOTAL_REPORT_BYTES {
            return Err(FederatedContinualReleaseError::InvalidRequest(
                "portfolio reports exceed the combined aggregation bound".into(),
            ));
        }
        previous_epoch = Some(epoch.epoch_index);
    }

    let mut review_keys = BTreeSet::new();
    let mut reviews_per_change = BTreeMap::<(u32, ContentHash), usize>::new();
    for review in &request.change_reviews {
        let key = (review.epoch_index, review.study_commitment.clone());
        let count = reviews_per_change.entry(key).or_default();
        *count += 1;
        if !expected_epochs.contains(&review.epoch_index)
            || request.expected_epoch_order.first() == Some(&review.epoch_index)
            || !request
                .expected_study_commitments
                .contains(&review.study_commitment)
            || review.previous_manifest_digest == review.current_manifest_digest
            || !review_keys.insert((
                review.epoch_index,
                review.study_commitment.clone(),
                review.reviewer_commitment.clone(),
            ))
            || *count > MAX_REVIEWS_PER_CHANGE
        {
            return Err(FederatedContinualReleaseError::InvalidRequest(
                "change reviews must target a later expected epoch, a cohort member, a real manifest change, and a unique bounded reviewer commitment".into(),
            ));
        }
    }
    Ok(())
}

fn review_evidence(
    epoch_index: u32,
    study_commitment: &ContentHash,
    reviews: &[FederatedReleaseChangeReview],
) -> Vec<FederatedReleaseReviewEvidence> {
    let mut result = reviews
        .iter()
        .filter(|review| {
            review.epoch_index == epoch_index && &review.study_commitment == study_commitment
        })
        .map(|review| FederatedReleaseReviewEvidence {
            epoch_index: review.epoch_index,
            study_commitment: review.study_commitment.clone(),
            previous_manifest_digest: review.previous_manifest_digest.clone(),
            current_manifest_digest: review.current_manifest_digest.clone(),
            reviewer_commitment: review.reviewer_commitment.clone(),
            evidence_digest: review.evidence_digest.clone(),
            decision: review.decision,
            independent: review.independent,
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| left.reviewer_commitment.cmp(&right.reviewer_commitment));
    result
}

fn summarize_change(
    previous: Option<&ContentHash>,
    current: Option<&ContentHash>,
    has_previous_epoch: bool,
    reviews: &[FederatedReleaseReviewEvidence],
    min_approvals: u8,
) -> (FederatedReleaseChangeState, u16, u16, u16) {
    let (Some(previous), Some(current)) = (previous, current) else {
        return (
            if current.is_none() {
                FederatedReleaseChangeState::Missing
            } else if has_previous_epoch {
                FederatedReleaseChangeState::ContinuityGap
            } else {
                FederatedReleaseChangeState::Baseline
            },
            0,
            0,
            0,
        );
    };
    if previous == current {
        return (FederatedReleaseChangeState::Unchanged, 0, 0, 0);
    }
    let approvals = reviews
        .iter()
        .filter(|review| {
            review.decision == ReleaseChangeReviewDecision::Approve && review.independent
        })
        .count() as u16;
    let holds = reviews
        .iter()
        .filter(|review| review.decision == ReleaseChangeReviewDecision::Hold)
        .count() as u16;
    let non_independent = reviews
        .iter()
        .filter(|review| {
            review.decision == ReleaseChangeReviewDecision::Approve && !review.independent
        })
        .count() as u16;
    let state = if holds > 0 {
        FederatedReleaseChangeState::Held
    } else if approvals >= u16::from(min_approvals) {
        FederatedReleaseChangeState::Approved
    } else if non_independent > 0 {
        FederatedReleaseChangeState::InsufficientIndependentReview
    } else {
        FederatedReleaseChangeState::ReviewRequired
    };
    (state, approvals, holds, non_independent)
}

fn workflow_counts(
    transitions: &[FederatedReleaseStudyTransition],
) -> (u16, u16, u16, u16, u16, u16) {
    let count = |disposition| {
        transitions
            .iter()
            .filter(|transition| transition.workflow_disposition == disposition)
            .count() as u16
    };
    (
        count(MultiStudyEvidenceDisposition::ReadyForSigning),
        count(MultiStudyEvidenceDisposition::Hold),
        count(MultiStudyEvidenceDisposition::Unresolved),
        count(MultiStudyEvidenceDisposition::Blocked),
        count(MultiStudyEvidenceDisposition::NonReproducible),
        count(MultiStudyEvidenceDisposition::Missing),
    )
}

fn epoch_disposition(counts: &FederatedReleaseEpochCounts) -> FederatedReleaseEpochDisposition {
    if counts.non_reproducible > 0 {
        FederatedReleaseEpochDisposition::NonReproducible
    } else if counts.blocked > 0 {
        FederatedReleaseEpochDisposition::Blocked
    } else if counts.held_changes > 0 || counts.held > 0 {
        FederatedReleaseEpochDisposition::Hold
    } else if counts.unresolved > 0 || counts.continuity_gaps > 0 {
        FederatedReleaseEpochDisposition::Unresolved
    } else if counts.missing > 0 {
        FederatedReleaseEpochDisposition::Missing
    } else if counts.changes_requiring_review > 0 {
        FederatedReleaseEpochDisposition::ChangeReviewRequired
    } else if counts.ready_for_signing < counts.expected_studies {
        FederatedReleaseEpochDisposition::Insufficient
    } else {
        FederatedReleaseEpochDisposition::ReadyForSigning
    }
}

fn epoch_summaries(
    request: &FederatedContinualReleaseRequest,
    transitions: &[FederatedReleaseStudyTransition],
    portfolios: &BTreeMap<u32, &MultiStudyReleaseReport>,
) -> Vec<FederatedReleaseEpochSummary> {
    request
        .expected_epoch_order
        .iter()
        .map(|epoch_index| {
            let rows = transitions
                .iter()
                .filter(|transition| transition.epoch_index == *epoch_index)
                .cloned()
                .collect::<Vec<_>>();
            let (ready, held, unresolved, blocked, non_reproducible, missing) =
                workflow_counts(&rows);
            let counts = FederatedReleaseEpochCounts {
                expected_studies: request.expected_study_commitments.len() as u16,
                ready_for_signing: ready,
                held,
                unresolved,
                blocked,
                non_reproducible,
                missing,
                changed_manifests: rows
                    .iter()
                    .filter(|row| {
                        row.previous_manifest_digest.is_some()
                            && row.current_manifest_digest.is_some()
                            && row.previous_manifest_digest != row.current_manifest_digest
                    })
                    .count() as u16,
                approved_changes: rows
                    .iter()
                    .filter(|row| row.change_state == FederatedReleaseChangeState::Approved)
                    .count() as u16,
                changes_requiring_review: rows
                    .iter()
                    .filter(|row| {
                        matches!(
                            row.change_state,
                            FederatedReleaseChangeState::ReviewRequired
                                | FederatedReleaseChangeState::InsufficientIndependentReview
                        )
                    })
                    .count() as u16,
                held_changes: rows
                    .iter()
                    .filter(|row| row.change_state == FederatedReleaseChangeState::Held)
                    .count() as u16,
                continuity_gaps: rows
                    .iter()
                    .filter(|row| row.change_state == FederatedReleaseChangeState::ContinuityGap)
                    .count() as u16,
            };
            FederatedReleaseEpochSummary {
                epoch_index: *epoch_index,
                portfolio_digest: portfolios
                    .get(epoch_index)
                    .map(|report| report.report_digest.clone()),
                disposition: epoch_disposition(&counts),
                counts,
            }
        })
        .collect()
}

fn overall_disposition(
    epochs: &[FederatedReleaseEpochSummary],
) -> FederatedContinualReleaseDisposition {
    if epochs
        .iter()
        .any(|epoch| epoch.disposition == FederatedReleaseEpochDisposition::NonReproducible)
    {
        FederatedContinualReleaseDisposition::NonReproducible
    } else if epochs
        .iter()
        .any(|epoch| epoch.disposition == FederatedReleaseEpochDisposition::Blocked)
    {
        FederatedContinualReleaseDisposition::Blocked
    } else if epochs
        .iter()
        .any(|epoch| epoch.disposition == FederatedReleaseEpochDisposition::Hold)
    {
        FederatedContinualReleaseDisposition::Hold
    } else if epochs
        .iter()
        .any(|epoch| epoch.disposition == FederatedReleaseEpochDisposition::Unresolved)
    {
        FederatedContinualReleaseDisposition::Unresolved
    } else if epochs.iter().any(|epoch| {
        epoch.disposition == FederatedReleaseEpochDisposition::Missing
            || epoch.disposition == FederatedReleaseEpochDisposition::Insufficient
    }) {
        FederatedContinualReleaseDisposition::Insufficient
    } else if epochs
        .iter()
        .any(|epoch| epoch.disposition == FederatedReleaseEpochDisposition::ChangeReviewRequired)
    {
        FederatedContinualReleaseDisposition::ChangeReviewRequired
    } else {
        FederatedContinualReleaseDisposition::ReadyForSigning
    }
}

fn next_action(disposition: FederatedContinualReleaseDisposition) -> &'static str {
    match disposition {
        FederatedContinualReleaseDisposition::ReadyForSigning => {
            "route each epoch's complete portfolio through the institution's accountable signing workflow"
        }
        FederatedContinualReleaseDisposition::Insufficient => {
            "collect the missing expected epoch or study portfolios before release review"
        }
        FederatedContinualReleaseDisposition::ChangeReviewRequired => {
            "obtain independent, change-bound reviews for every modified study manifest"
        }
        FederatedContinualReleaseDisposition::Hold => {
            "resolve portfolio or change-review holds and recompute the release history"
        }
        FederatedContinualReleaseDisposition::Unresolved => {
            "restore manifest continuity and resolve unavailable workflow or review evidence"
        }
        FederatedContinualReleaseDisposition::Blocked => {
            "repair blocked study workflows before release review"
        }
        FederatedContinualReleaseDisposition::NonReproducible => {
            "preserve divergent study outcomes and withhold reproducibility claims"
        }
    }
}

/// Reconcile bounded portfolio reports and change reviews over successive federation epochs.
pub fn reconcile_glioma_federated_continual_release(
    request: &FederatedContinualReleaseRequest,
) -> Result<FederatedContinualReleaseReport, FederatedContinualReleaseError> {
    valid_request(request)?;
    let portfolios = request
        .epochs
        .iter()
        .map(|epoch| (epoch.epoch_index, &epoch.portfolio))
        .collect::<BTreeMap<_, _>>();
    let mut report = FederatedContinualReleaseReport {
        feature_id: FEATURE_ID.into(),
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: request.research_id.clone(),
        expected_epoch_order: request.expected_epoch_order.clone(),
        expected_study_commitments: request.expected_study_commitments.clone(),
        min_independent_change_approvals: request.min_independent_change_approvals,
        site_commitments_authenticated: false,
        reviewer_commitments_authenticated: false,
        signed_or_published: false,
        transitions: Vec::with_capacity(
            request.expected_epoch_order.len() * request.expected_study_commitments.len(),
        ),
        epochs: Vec::new(),
        disposition: FederatedContinualReleaseDisposition::Insufficient,
        next_operator_action: String::new(),
        chain_head: ContentHash::of_bytes(b"unsealed-glioma-federated-continual-release"),
        report_digest: ContentHash::of_bytes(b"unsealed-glioma-federated-continual-release-report"),
    };
    let mut previous_digest = report_seed(&report)?;
    let mut previous_epoch_manifest = BTreeMap::<ContentHash, Option<ContentHash>>::new();
    for (epoch_position, epoch_index) in request.expected_epoch_order.iter().enumerate() {
        let portfolio = portfolios.get(epoch_index).copied();
        for study_commitment in &request.expected_study_commitments {
            let evidence = portfolio.and_then(|portfolio| {
                portfolio
                    .evidence
                    .iter()
                    .find(|item| &item.study_commitment == study_commitment)
            });
            let current_manifest = evidence.and_then(|item| item.manifest_digest.clone());
            let prior_manifest = (epoch_position > 0)
                .then(|| {
                    previous_epoch_manifest
                        .get(study_commitment)
                        .cloned()
                        .flatten()
                })
                .flatten();
            let reviews = match (&prior_manifest, &current_manifest) {
                (Some(previous), Some(current)) if previous != current => {
                    review_evidence(*epoch_index, study_commitment, &request.change_reviews)
                }
                _ => Vec::new(),
            };
            let (change_state, independent_approvals, review_holds, non_independent_approvals) =
                summarize_change(
                    prior_manifest.as_ref(),
                    current_manifest.as_ref(),
                    epoch_position > 0,
                    &reviews,
                    request.min_independent_change_approvals,
                );
            let mut transition = FederatedReleaseStudyTransition {
                epoch_index: *epoch_index,
                study_commitment: study_commitment.clone(),
                workflow_disposition: evidence
                    .map_or(MultiStudyEvidenceDisposition::Missing, |item| {
                        item.disposition
                    }),
                previous_manifest_digest: prior_manifest,
                current_manifest_digest: current_manifest.clone(),
                change_state,
                independent_approvals,
                review_holds,
                non_independent_approvals,
                reviews,
                previous_digest: previous_digest.clone(),
                entry_digest: ContentHash::of_bytes(
                    b"unsealed-glioma-federated-release-transition",
                ),
            };
            transition.entry_digest = transition_digest(&transition)?;
            previous_digest = transition.entry_digest.clone();
            report.transitions.push(transition);
            previous_epoch_manifest.insert(study_commitment.clone(), current_manifest);
        }
    }
    validate_change_reviews(request, &report.transitions)?;
    report.chain_head = previous_digest;
    report.epochs = epoch_summaries(request, &report.transitions, &portfolios);
    report.disposition = overall_disposition(&report.epochs);
    report.next_operator_action = next_action(report.disposition).into();
    report.report_digest = report_digest(&report)?;
    report.validate()?;
    Ok(report)
}

fn validate_change_reviews(
    request: &FederatedContinualReleaseRequest,
    transitions: &[FederatedReleaseStudyTransition],
) -> Result<(), FederatedContinualReleaseError> {
    for review in &request.change_reviews {
        let transition = transitions.iter().find(|transition| {
            transition.epoch_index == review.epoch_index
                && transition.study_commitment == review.study_commitment
        });
        if transition.is_none_or(|transition| {
            transition.previous_manifest_digest.as_ref() != Some(&review.previous_manifest_digest)
                || transition.current_manifest_digest.as_ref()
                    != Some(&review.current_manifest_digest)
                || transition.previous_manifest_digest == transition.current_manifest_digest
        }) {
            return Err(FederatedContinualReleaseError::InvalidRequest(
                "every change review must bind the immediately preceding and current manifest digests for an actual changed study".into(),
            ));
        }
    }
    Ok(())
}

impl FederatedContinualReleaseReport {
    /// Validate ordered epoch coverage, review bindings, state summaries, and digest chains.
    pub fn validate(&self) -> Result<(), FederatedContinualReleaseError> {
        if self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.research_id.trim().is_empty()
            || !(MIN_EPOCHS..=MAX_EPOCHS).contains(&self.expected_epoch_order.len())
            || self
                .expected_epoch_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || !(2..=MAX_STUDIES).contains(&self.expected_study_commitments.len())
            || self
                .expected_study_commitments
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.min_independent_change_approvals == 0
            || usize::from(self.min_independent_change_approvals) > MAX_REVIEWS_PER_CHANGE
            || self.site_commitments_authenticated
            || self.reviewer_commitments_authenticated
            || self.signed_or_published
            || self.transitions.len()
                != self.expected_epoch_order.len() * self.expected_study_commitments.len()
            || self
                .transitions
                .iter()
                .map(|transition| transition.reviews.len())
                .sum::<usize>()
                > MAX_TOTAL_REVIEWS
            || self.epochs.len() != self.expected_epoch_order.len()
            || self.next_operator_action != next_action(self.disposition)
        {
            return Err(FederatedContinualReleaseError::InvalidOutput(
                "report identity, ordered cohort, bounds, transition coverage, or operator action is invalid".into(),
            ));
        }
        let mut previous_digest = report_seed(self)?;
        for (index, transition) in self.transitions.iter().enumerate() {
            let epoch_position = index / self.expected_study_commitments.len();
            let study_position = index % self.expected_study_commitments.len();
            let expected_epoch = self.expected_epoch_order[epoch_position];
            let expected_study = &self.expected_study_commitments[study_position];
            let expected_previous_manifest = if epoch_position == 0 {
                None
            } else {
                self.transitions[index - self.expected_study_commitments.len()]
                    .current_manifest_digest
                    .as_ref()
            };
            if transition.epoch_index != expected_epoch
                || &transition.study_commitment != expected_study
                || transition.previous_manifest_digest.as_ref() != expected_previous_manifest
                || transition.previous_digest != previous_digest
                || transition.entry_digest != transition_digest(transition)?
                || transition
                    .reviews
                    .windows(2)
                    .any(|pair| pair[0].reviewer_commitment >= pair[1].reviewer_commitment)
                || transition.reviews.len() > MAX_REVIEWS_PER_CHANGE
                || (transition.workflow_disposition == MultiStudyEvidenceDisposition::Missing)
                    != transition.current_manifest_digest.is_none()
            {
                return Err(FederatedContinualReleaseError::InvalidOutput(
                    "transition ordering, reviewer ordering, or digest chain is invalid".into(),
                ));
            }
            let changed = transition.previous_manifest_digest.is_some()
                && transition.current_manifest_digest.is_some()
                && transition.previous_manifest_digest != transition.current_manifest_digest;
            let (expected_change_state, approvals, holds, non_independent) = summarize_change(
                transition.previous_manifest_digest.as_ref(),
                transition.current_manifest_digest.as_ref(),
                index / self.expected_study_commitments.len() > 0,
                &transition.reviews,
                self.min_independent_change_approvals,
            );
            if (!transition.reviews.is_empty() && !changed)
                || transition.reviews.iter().any(|review| {
                    review.epoch_index != transition.epoch_index
                        || review.study_commitment != transition.study_commitment
                        || Some(&review.previous_manifest_digest)
                            != transition.previous_manifest_digest.as_ref()
                        || Some(&review.current_manifest_digest)
                            != transition.current_manifest_digest.as_ref()
                })
                || transition.change_state != expected_change_state
                || transition.independent_approvals != approvals
                || transition.review_holds != holds
                || transition.non_independent_approvals != non_independent
            {
                return Err(FederatedContinualReleaseError::InvalidOutput(
                    "review evidence is not bound to a changed manifest transition".into(),
                ));
            }
            previous_digest = transition.entry_digest.clone();
        }
        if self.chain_head != previous_digest {
            return Err(FederatedContinualReleaseError::InvalidOutput(
                "transition chain head is inconsistent".into(),
            ));
        }
        let expected_epochs = self
            .expected_epoch_order
            .iter()
            .enumerate()
            .map(|(position, epoch_index)| {
                let start = position * self.expected_study_commitments.len();
                let rows = &self.transitions[start..start + self.expected_study_commitments.len()];
                let (ready, held, unresolved, blocked, non_reproducible, missing) =
                    workflow_counts(rows);
                let counts = FederatedReleaseEpochCounts {
                    expected_studies: self.expected_study_commitments.len() as u16,
                    ready_for_signing: ready,
                    held,
                    unresolved,
                    blocked,
                    non_reproducible,
                    missing,
                    changed_manifests: rows
                        .iter()
                        .filter(|row| {
                            row.previous_manifest_digest.is_some()
                                && row.current_manifest_digest.is_some()
                                && row.previous_manifest_digest != row.current_manifest_digest
                        })
                        .count() as u16,
                    approved_changes: rows
                        .iter()
                        .filter(|row| row.change_state == FederatedReleaseChangeState::Approved)
                        .count() as u16,
                    changes_requiring_review: rows
                        .iter()
                        .filter(|row| {
                            matches!(
                                row.change_state,
                                FederatedReleaseChangeState::ReviewRequired
                                    | FederatedReleaseChangeState::InsufficientIndependentReview
                            )
                        })
                        .count() as u16,
                    held_changes: rows
                        .iter()
                        .filter(|row| row.change_state == FederatedReleaseChangeState::Held)
                        .count() as u16,
                    continuity_gaps: rows
                        .iter()
                        .filter(|row| {
                            row.change_state == FederatedReleaseChangeState::ContinuityGap
                        })
                        .count() as u16,
                };
                FederatedReleaseEpochSummary {
                    epoch_index: *epoch_index,
                    portfolio_digest: self
                        .epochs
                        .get(position)
                        .and_then(|epoch| epoch.portfolio_digest.clone()),
                    disposition: epoch_disposition(&counts),
                    counts,
                }
            })
            .collect::<Vec<_>>();
        if self
            .epochs
            .iter()
            .zip(&expected_epochs)
            .any(|(actual, expected)| {
                actual.epoch_index != expected.epoch_index
                    || actual.counts != expected.counts
                    || actual.disposition != expected.disposition
                    || actual.portfolio_digest != expected.portfolio_digest
            })
            || self.disposition != overall_disposition(&self.epochs)
            || self.report_digest != report_digest(self)?
        {
            return Err(FederatedContinualReleaseError::InvalidOutput(
                "epoch counts, dispositions, portfolio commitments, or report digest are inconsistent".into(),
            ));
        }
        Ok(())
    }
}
