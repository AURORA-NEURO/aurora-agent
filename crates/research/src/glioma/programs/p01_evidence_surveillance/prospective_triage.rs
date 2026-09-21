//! Prospective evidence-triage workbench for high-throughput preclinical glioma surveillance.
//!
//! The evidence stream already computes claim-level support, contradiction, coverage, and trend.
//! This feature turns that frontier into a researcher-facing queue with a deterministic urgency
//! function, reviewer-capacity assignment, freshness pressure, budget admission, and explicit
//! suppression/defer states. It is an interaction and workflow capability, not an evidence
//! generator: raw sources stay local, and a queued review never becomes a typed claim by itself.

use super::evidence_stream::{
    EvidenceStreamClaim, EvidenceStreamClaimTrend, EvidenceStreamSnapshot,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F19";
pub const OUTPUT_SCHEMA: &str = "GliomaProspectiveEvidenceTriage1@1";
pub const MAX_CLAIMS: usize = 4_096;
pub const MAX_REVIEWS: usize = 4_096;
pub const MAX_REVIEWERS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTriageActionKind {
    ResolveContradiction,
    RefreshStaleEvidence,
    AcquireCoverage,
    ReconcileNegativeEvidence,
    ReviewUncertainty,
    ConfirmSupportedClaim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTriageReviewState {
    Unreviewed,
    Open,
    Resolved,
    Suppressed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTriageStatus {
    Queued,
    Held,
    Deferred,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTriageDisposition {
    Ready,
    Partial,
    CapacityBlocked,
    NoOpenClaims,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProspectiveTriagePolicy {
    pub minimum_priority_milli: u16,
    pub minimum_coverage_milli: u16,
    pub maximum_age_epochs: u64,
    pub maximum_queue_depth: usize,
    pub budget_units: u64,
    pub item_cost_units: u64,
    pub require_reviewer_for_contradiction: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceTriageReviewObservation {
    pub claim_key: String,
    pub last_reviewed_epoch: u64,
    pub state: EvidenceTriageReviewState,
    pub reviewer_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceTriageReviewerCapacity {
    pub reviewer_id: String,
    pub role: String,
    pub capacity_items: usize,
    pub occupied_items: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProspectiveTriageRequest {
    pub objective: String,
    pub snapshot: EvidenceStreamSnapshot,
    pub policy: EvidenceProspectiveTriagePolicy,
    pub reviews: Vec<EvidenceTriageReviewObservation>,
    pub reviewers: Vec<EvidenceTriageReviewerCapacity>,
    pub current_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceTriageDecision {
    pub claim_key: String,
    pub action: EvidenceTriageActionKind,
    pub priority_milli: u16,
    pub status: EvidenceTriageStatus,
    pub age_epochs: u64,
    pub reviewer_id: Option<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceTriageReviewerLoad {
    pub reviewer_id: String,
    pub role: String,
    pub capacity_items: usize,
    pub occupied_items: usize,
    pub assigned_items: usize,
    pub remaining_items: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProspectiveTriagePlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub snapshot_digest: ContentHash,
    pub current_epoch: u64,
    pub claim_order: Vec<String>,
    pub queue_order: Vec<String>,
    pub held_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub resolved_order: Vec<String>,
    pub decisions: Vec<EvidenceTriageDecision>,
    pub reviewer_loads: Vec<EvidenceTriageReviewerLoad>,
    pub total_cost_units: u64,
    pub budget_units: u64,
    pub budget_remaining_units: u64,
    pub queued_priority_milli: u16,
    pub contradiction_count: usize,
    pub coverage_milli: u16,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: EvidenceTriageDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceProspectiveTriageError {
    #[error("prospective evidence triage request is invalid: {0}")]
    InvalidRequest(String),
    #[error("prospective evidence triage input is invalid: {0}")]
    InvalidInput(String),
    #[error("prospective evidence triage output is invalid: {0}")]
    InvalidOutput(String),
    #[error("prospective evidence triage digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(output: &EvidenceProspectiveTriagePlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "snapshot_digest": output.snapshot_digest,
        "current_epoch": output.current_epoch,
        "claim_order": output.claim_order,
        "queue_order": output.queue_order,
        "held_order": output.held_order,
        "deferred_order": output.deferred_order,
        "resolved_order": output.resolved_order,
        "decisions": output.decisions,
        "reviewer_loads": output.reviewer_loads,
        "total_cost_units": output.total_cost_units,
        "budget_units": output.budget_units,
        "budget_remaining_units": output.budget_remaining_units,
        "queued_priority_milli": output.queued_priority_milli,
        "contradiction_count": output.contradiction_count,
        "coverage_milli": output.coverage_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_route": output.next_route,
    })
}

fn validate_request(
    request: &EvidenceProspectiveTriageRequest,
) -> Result<(), EvidenceProspectiveTriageError> {
    if request.objective.trim().is_empty()
        || request.objective != request.snapshot.objective
        || request.current_epoch == 0
        || request.snapshot.claim_order.is_empty()
        || request.snapshot.claim_order.len() > MAX_CLAIMS
        || request.reviews.len() > MAX_REVIEWS
        || request.reviewers.len() > MAX_REVIEWERS
    {
        return Err(EvidenceProspectiveTriageError::InvalidRequest(
            "objective/snapshot binding, positive epoch, and bounded claim/review/reviewer inputs are required".into(),
        ));
    }
    request
        .snapshot
        .validate()
        .map_err(|error| EvidenceProspectiveTriageError::InvalidInput(error.to_string()))?;
    let policy = &request.policy;
    if policy.minimum_priority_milli > 1_000
        || policy.minimum_coverage_milli > 1_000
        || policy.maximum_age_epochs == 0
        || policy.maximum_queue_depth == 0
        || policy.maximum_queue_depth > MAX_CLAIMS
        || policy.budget_units == 0
        || policy.item_cost_units == 0
    {
        return Err(EvidenceProspectiveTriageError::InvalidInput(
            "priority, coverage, age, queue, and budget policy bounds are invalid".into(),
        ));
    }
    let claim_ids = request
        .snapshot
        .claim_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut review_ids = BTreeSet::new();
    for review in &request.reviews {
        if !claim_ids.contains(&review.claim_key)
            || review.claim_key.trim().is_empty()
            || review.last_reviewed_epoch > request.current_epoch
            || !review_ids.insert(review.claim_key.clone())
            || (review.state != EvidenceTriageReviewState::Unreviewed
                && review
                    .reviewer_id
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .is_empty())
        {
            return Err(EvidenceProspectiveTriageError::InvalidInput(
                "review observations must uniquely cover known claims with bounded epochs and reviewer identity".into(),
            ));
        }
    }
    let mut reviewer_ids = BTreeSet::new();
    for reviewer in &request.reviewers {
        if reviewer.reviewer_id.trim().is_empty()
            || reviewer.role.trim().is_empty()
            || reviewer.capacity_items == 0
            || reviewer.occupied_items > reviewer.capacity_items
            || !reviewer_ids.insert(reviewer.reviewer_id.clone())
        {
            return Err(EvidenceProspectiveTriageError::InvalidInput(
                "reviewer capacities must be unique, named, positive, and non-overcommitted".into(),
            ));
        }
    }
    Ok(())
}

fn age_pressure(age_epochs: u64, maximum_age_epochs: u64) -> u16 {
    ((age_epochs.saturating_mul(1_000) / maximum_age_epochs).min(1_000)) as u16
}

fn priority_for_claim(
    claim: &EvidenceStreamClaim,
    age_epochs: u64,
    policy: &EvidenceProspectiveTriagePolicy,
) -> u16 {
    let trend_pressure = match claim.trend {
        EvidenceStreamClaimTrend::Declining => claim.trend_milli.unsigned_abs().min(1_000),
        EvidenceStreamClaimTrend::Unresolved => 850,
        EvidenceStreamClaimTrend::Rising | EvidenceStreamClaimTrend::Stable => 0,
    };
    let source_debt =
        1_000_u16.saturating_sub((claim.source_kind_order.len().min(4) as u16).saturating_mul(250));
    let weighted = u32::from(claim.contradiction_milli) * 300
        + u32::from(claim.unknown_milli) * 220
        + u32::from(1_000_u16.saturating_sub(claim.coverage_milli)) * 180
        + u32::from(trend_pressure) * 120
        + u32::from(age_pressure(age_epochs, policy.maximum_age_epochs)) * 100
        + u32::from(source_debt) * 80;
    (weighted / 1_000).min(1_000) as u16
}

fn action_for_claim(
    claim: &EvidenceStreamClaim,
    age_epochs: u64,
    policy: &EvidenceProspectiveTriagePolicy,
) -> EvidenceTriageActionKind {
    if claim.contradiction_milli > 0 {
        EvidenceTriageActionKind::ResolveContradiction
    } else if age_epochs > policy.maximum_age_epochs
        || claim.trend == EvidenceStreamClaimTrend::Declining
    {
        EvidenceTriageActionKind::RefreshStaleEvidence
    } else if claim.coverage_milli < policy.minimum_coverage_milli {
        EvidenceTriageActionKind::AcquireCoverage
    } else if claim.negative_milli > claim.support_milli {
        EvidenceTriageActionKind::ReconcileNegativeEvidence
    } else if claim.unknown_milli > 0 || claim.trend == EvidenceStreamClaimTrend::Unresolved {
        EvidenceTriageActionKind::ReviewUncertainty
    } else {
        EvidenceTriageActionKind::ConfirmSupportedClaim
    }
}

/// Convert a live evidence stream frontier into a bounded researcher review queue.
pub fn plan_glioma_prospective_evidence_triage(
    request: &EvidenceProspectiveTriageRequest,
) -> Result<EvidenceProspectiveTriagePlan, EvidenceProspectiveTriageError> {
    validate_request(request)?;
    let claims = request
        .snapshot
        .claims
        .iter()
        .map(|claim| (claim.claim_key.clone(), claim))
        .collect::<BTreeMap<_, _>>();
    let reviews = request
        .reviews
        .iter()
        .map(|review| (review.claim_key.clone(), review))
        .collect::<BTreeMap<_, _>>();
    let reviewer_map = request
        .reviewers
        .iter()
        .map(|reviewer| (reviewer.reviewer_id.clone(), reviewer))
        .collect::<BTreeMap<_, _>>();
    let mut assigned = request
        .reviewers
        .iter()
        .map(|reviewer| (reviewer.reviewer_id.clone(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut candidates = Vec::<(String, u16, EvidenceTriageActionKind, u64, String)>::new();
    let mut decisions = BTreeMap::<String, EvidenceTriageDecision>::new();
    let mut held = Vec::new();
    let mut deferred = Vec::new();
    let mut resolved = Vec::new();
    let mut negative_evidence = request.snapshot.negative_evidence.clone();
    let mut uncertainty = request.snapshot.uncertainty.clone();
    let mut contradiction_count = 0_usize;
    for claim_key in &request.snapshot.claim_order {
        let claim = claims.get(claim_key).expect("validated claim coverage");
        let age = request.current_epoch.saturating_sub(claim.latest_epoch);
        let action = action_for_claim(claim, age, &request.policy);
        let priority = priority_for_claim(claim, age, &request.policy);
        if claim.contradiction_milli > 0 {
            contradiction_count += 1;
            negative_evidence.push(format!("{claim_key}:contradiction-pressure"));
        }
        let review = reviews.get(claim_key).copied();
        if let Some(review) = review {
            match review.state {
                EvidenceTriageReviewState::Suppressed => {
                    held.push(claim_key.clone());
                    decisions.insert(
                        claim_key.clone(),
                        EvidenceTriageDecision {
                            claim_key: claim_key.clone(),
                            action,
                            priority_milli: priority,
                            status: EvidenceTriageStatus::Held,
                            age_epochs: age,
                            reviewer_id: review.reviewer_id.clone(),
                            rationale: "claim is explicitly suppressed by a researcher".into(),
                        },
                    );
                    continue;
                }
                EvidenceTriageReviewState::Resolved
                    if review.last_reviewed_epoch >= claim.latest_epoch
                        && claim.contradiction_milli == 0 =>
                {
                    resolved.push(claim_key.clone());
                    decisions.insert(
                        claim_key.clone(),
                        EvidenceTriageDecision {
                            claim_key: claim_key.clone(),
                            action,
                            priority_milli: priority,
                            status: EvidenceTriageStatus::Resolved,
                            age_epochs: age,
                            reviewer_id: review.reviewer_id.clone(),
                            rationale: "researcher review is current for the observed claim epoch"
                                .into(),
                        },
                    );
                    continue;
                }
                EvidenceTriageReviewState::Resolved | EvidenceTriageReviewState::Open => {}
                EvidenceTriageReviewState::Unreviewed => {}
            }
        }
        if age > request.policy.maximum_age_epochs {
            uncertainty.push(format!("{claim_key}:age-epochs={age}"));
        }
        if priority < request.policy.minimum_priority_milli {
            held.push(claim_key.clone());
            decisions.insert(
                claim_key.clone(),
                EvidenceTriageDecision {
                    claim_key: claim_key.clone(),
                    action,
                    priority_milli: priority,
                    status: EvidenceTriageStatus::Held,
                    age_epochs: age,
                    reviewer_id: None,
                    rationale: "priority remains below the researcher queue threshold".into(),
                },
            );
            continue;
        }
        candidates.push((
            claim_key.clone(),
            priority,
            action,
            age,
            if claim.contradiction_milli > 0 {
                "contradiction requires explicit researcher reconciliation".into()
            } else if claim.coverage_milli < request.policy.minimum_coverage_milli {
                "modality/model coverage debt requires targeted acquisition".into()
            } else if age > request.policy.maximum_age_epochs {
                "evidence is stale for the prospective surveillance window".into()
            } else {
                "prospective trend or uncertainty requires researcher review".into()
            },
        ));
    }
    candidates.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let mut queue = Vec::new();
    let mut total_cost = 0_u64;
    for (claim_key, priority, action, age, rationale) in candidates {
        if queue.len() >= request.policy.maximum_queue_depth
            || total_cost.saturating_add(request.policy.item_cost_units)
                > request.policy.budget_units
        {
            deferred.push(claim_key.clone());
            uncertainty.push(format!("{claim_key}:queue-or-budget-deferred"));
            decisions.insert(
                claim_key.clone(),
                EvidenceTriageDecision {
                    claim_key,
                    action,
                    priority_milli: priority,
                    status: EvidenceTriageStatus::Deferred,
                    age_epochs: age,
                    reviewer_id: None,
                    rationale: "queue depth or review budget is exhausted".into(),
                },
            );
            continue;
        }
        let requires_reviewer = request.policy.require_reviewer_for_contradiction
            && action == EvidenceTriageActionKind::ResolveContradiction;
        let mut selected_reviewer = None;
        if requires_reviewer || !reviewer_map.is_empty() {
            let mut eligible = reviewer_map
                .values()
                .filter(|reviewer| {
                    reviewer
                        .occupied_items
                        .saturating_add(*assigned.get(&reviewer.reviewer_id).unwrap_or(&0))
                        < reviewer.capacity_items
                })
                .map(|reviewer| {
                    (
                        reviewer
                            .occupied_items
                            .saturating_add(*assigned.get(&reviewer.reviewer_id).unwrap_or(&0)),
                        reviewer.reviewer_id.clone(),
                    )
                })
                .collect::<Vec<_>>();
            eligible.sort();
            selected_reviewer = eligible.first().map(|(_, reviewer_id)| reviewer_id.clone());
        }
        if requires_reviewer && selected_reviewer.is_none() {
            held.push(claim_key.clone());
            uncertainty.push(format!("{claim_key}:reviewer-capacity-required"));
            decisions.insert(
                claim_key.clone(),
                EvidenceTriageDecision {
                    claim_key,
                    action,
                    priority_milli: priority,
                    status: EvidenceTriageStatus::Held,
                    age_epochs: age,
                    reviewer_id: None,
                    rationale:
                        "contradiction requires a researcher reviewer but capacity is unavailable"
                            .into(),
                },
            );
            continue;
        }
        if let Some(reviewer_id) = &selected_reviewer {
            *assigned.entry(reviewer_id.clone()).or_default() += 1;
        }
        total_cost = total_cost.saturating_add(request.policy.item_cost_units);
        queue.push(claim_key.clone());
        decisions.insert(
            claim_key.clone(),
            EvidenceTriageDecision {
                claim_key,
                action,
                priority_milli: priority,
                status: EvidenceTriageStatus::Queued,
                age_epochs: age,
                reviewer_id: selected_reviewer,
                rationale,
            },
        );
    }
    held.sort();
    deferred.sort();
    resolved.sort();
    let claim_order = request.snapshot.claim_order.clone();
    let decisions = claim_order
        .iter()
        .map(|claim_key| decisions.remove(claim_key).expect("decision coverage"))
        .collect::<Vec<_>>();
    let mut reviewer_loads = request
        .reviewers
        .iter()
        .map(|reviewer| {
            let assigned_items = assigned.get(&reviewer.reviewer_id).copied().unwrap_or(0);
            EvidenceTriageReviewerLoad {
                reviewer_id: reviewer.reviewer_id.clone(),
                role: reviewer.role.clone(),
                capacity_items: reviewer.capacity_items,
                occupied_items: reviewer.occupied_items,
                assigned_items,
                remaining_items: reviewer
                    .capacity_items
                    .saturating_sub(reviewer.occupied_items.saturating_add(assigned_items)),
            }
        })
        .collect::<Vec<_>>();
    reviewer_loads.sort_by(|left, right| left.reviewer_id.cmp(&right.reviewer_id));
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let queued_priority_milli = queue
        .iter()
        .filter_map(|claim_key| {
            decisions
                .iter()
                .find(|decision| &decision.claim_key == claim_key)
                .map(|decision| decision.priority_milli)
        })
        .max()
        .unwrap_or(0);
    let coverage_milli = if request.snapshot.claims.is_empty() {
        0
    } else {
        (request
            .snapshot
            .claims
            .iter()
            .map(|claim| u64::from(claim.coverage_milli))
            .sum::<u64>()
            / request.snapshot.claims.len() as u64) as u16
    };
    let disposition = if queue.is_empty() {
        let capacity_hold = decisions.iter().any(|decision| {
            decision.status == EvidenceTriageStatus::Held && decision.rationale.contains("capacity")
        });
        if !deferred.is_empty() || capacity_hold {
            EvidenceTriageDisposition::CapacityBlocked
        } else {
            EvidenceTriageDisposition::NoOpenClaims
        }
    } else if held.is_empty() && deferred.is_empty() {
        EvidenceTriageDisposition::Ready
    } else {
        EvidenceTriageDisposition::Partial
    };
    let next_route = if !queue.is_empty() {
        "glioma_knowledge_protocol_gateway"
    } else {
        "glioma_evidence_operating_cycle"
    };
    let mut output = EvidenceProspectiveTriagePlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        snapshot_digest: request.snapshot.digest.clone(),
        current_epoch: request.current_epoch,
        claim_order,
        queue_order: queue,
        held_order: held,
        deferred_order: deferred,
        resolved_order: resolved,
        decisions,
        reviewer_loads,
        total_cost_units: total_cost,
        budget_units: request.policy.budget_units,
        budget_remaining_units: request.policy.budget_units.saturating_sub(total_cost),
        queued_priority_milli,
        contradiction_count,
        coverage_milli,
        negative_evidence,
        uncertainty,
        disposition,
        next_route: next_route.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-prospective-evidence-triage"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceProspectiveTriageError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl EvidenceProspectiveTriagePlan {
    pub fn validate(&self) -> Result<(), EvidenceProspectiveTriageError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.snapshot_digest.as_str().len() != 64
            || self.current_epoch == 0
            || !canonical(&self.claim_order)
            || !unique_nonempty(&self.claim_order)
            || !canonical(&self.queue_order)
            || !canonical(&self.held_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.resolved_order)
            || !unique_nonempty(&self.queue_order)
            || !unique_nonempty(&self.held_order)
            || !unique_nonempty(&self.deferred_order)
            || !unique_nonempty(&self.resolved_order)
            || self.decisions.len() != self.claim_order.len()
            || self
                .decisions
                .iter()
                .map(|decision| decision.claim_key.clone())
                .collect::<Vec<_>>()
                != self.claim_order
            || !canonical(
                &self
                    .reviewer_loads
                    .iter()
                    .map(|load| load.reviewer_id.clone())
                    .collect::<Vec<_>>(),
            )
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.total_cost_units > self.budget_units
            || self
                .total_cost_units
                .saturating_add(self.budget_remaining_units)
                != self.budget_units
            || self.queued_priority_milli > 1_000
            || self.contradiction_count > self.claim_order.len()
            || self.coverage_milli > 1_000
            || self.next_route.trim().is_empty()
        {
            return Err(EvidenceProspectiveTriageError::InvalidOutput(
                "identity, ordering, partition, budget, score, or digest shape is invalid".into(),
            ));
        }
        let all_ids = self.claim_order.iter().cloned().collect::<BTreeSet<_>>();
        let queue = self.queue_order.iter().cloned().collect::<BTreeSet<_>>();
        let held = self.held_order.iter().cloned().collect::<BTreeSet<_>>();
        let deferred = self.deferred_order.iter().cloned().collect::<BTreeSet<_>>();
        let resolved = self.resolved_order.iter().cloned().collect::<BTreeSet<_>>();
        let partition = queue
            .union(&held)
            .cloned()
            .collect::<BTreeSet<_>>()
            .union(&deferred)
            .cloned()
            .collect::<BTreeSet<_>>()
            .union(&resolved)
            .cloned()
            .collect::<BTreeSet<_>>();
        let status_ids = |status: EvidenceTriageStatus| {
            self.decisions
                .iter()
                .filter(|decision| decision.status == status)
                .map(|decision| decision.claim_key.clone())
                .collect::<BTreeSet<_>>()
        };
        if partition != all_ids
            || queue.len() + held.len() + deferred.len() + resolved.len() != partition.len()
            || status_ids(EvidenceTriageStatus::Queued) != queue
            || status_ids(EvidenceTriageStatus::Held) != held
            || status_ids(EvidenceTriageStatus::Deferred) != deferred
            || status_ids(EvidenceTriageStatus::Resolved) != resolved
            || self.decisions.iter().any(|decision| {
                decision.claim_key.trim().is_empty()
                    || decision.priority_milli > 1_000
                    || decision.rationale.trim().is_empty()
            })
            || self.reviewer_loads.iter().any(|load| {
                load.reviewer_id.trim().is_empty()
                    || load.role.trim().is_empty()
                    || load.capacity_items == 0
                    || load.occupied_items > load.capacity_items
                    || load.assigned_items > load.capacity_items.saturating_sub(load.occupied_items)
                    || load.remaining_items
                        != load
                            .capacity_items
                            .saturating_sub(load.occupied_items.saturating_add(load.assigned_items))
            })
        {
            return Err(EvidenceProspectiveTriageError::InvalidOutput(
                "claim status partition, decision coverage, or reviewer load does not reconcile"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceProspectiveTriageError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceProspectiveTriageError::Digest(
                "prospective evidence triage digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::EvidenceSourceKind;
    use crate::glioma::programs::p01_evidence_surveillance::evidence_stream::{
        EvidenceStreamClaim, EvidenceStreamClaimTrend, EvidenceStreamDisposition,
    };

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"seed": seed})).unwrap()
    }

    fn claim(
        key: &str,
        contradiction_milli: u16,
        coverage_milli: u16,
        trend: EvidenceStreamClaimTrend,
    ) -> EvidenceStreamClaim {
        EvidenceStreamClaim {
            claim_key: key.into(),
            event_order: vec![format!("event-{key}")],
            modality_order: vec![],
            model_system_order: vec![],
            source_kind_order: vec![EvidenceSourceKind::Literature],
            first_epoch: 1,
            latest_epoch: 4,
            support_milli: 600,
            negative_milli: 50,
            contradiction_milli,
            unknown_milli: 100,
            coverage_milli,
            trend_milli: if trend == EvidenceStreamClaimTrend::Declining {
                800
            } else {
                0
            },
            trend,
            next_action: "review".into(),
        }
    }

    fn snapshot() -> EvidenceStreamSnapshot {
        let mut output = EvidenceStreamSnapshot {
            feature_id: super::super::evidence_stream::FEATURE_ID.into(),
            output_schema: super::super::evidence_stream::OUTPUT_SCHEMA.into(),
            objective: "triage glioma evidence".into(),
            accepted_order: vec!["event-a".into(), "event-b".into()],
            duplicate_order: vec![],
            late_order: vec![],
            rejected_order: vec![],
            claim_order: vec!["claim-a".into(), "claim-b".into()],
            claims: vec![
                claim("claim-a", 800, 400, EvidenceStreamClaimTrend::Declining),
                claim("claim-b", 0, 950, EvidenceStreamClaimTrend::Stable),
            ],
            frontier_order: vec!["claim-a".into()],
            negative_evidence: vec![],
            uncertainty: vec![],
            disposition: EvidenceStreamDisposition::Ready,
            next_step: "triage".into(),
            digest: hash("unsealed"),
        };
        output.digest =
            ContentHash::of_value(&super::super::evidence_stream::digest_input(&output)).unwrap();
        output.validate().unwrap();
        output
    }

    fn policy() -> EvidenceProspectiveTriagePolicy {
        EvidenceProspectiveTriagePolicy {
            minimum_priority_milli: 200,
            minimum_coverage_milli: 800,
            maximum_age_epochs: 2,
            maximum_queue_depth: 4,
            budget_units: 4,
            item_cost_units: 1,
            require_reviewer_for_contradiction: true,
        }
    }

    #[test]
    fn ranks_contradiction_and_assigns_researcher_capacity() {
        let request = EvidenceProspectiveTriageRequest {
            objective: "triage glioma evidence".into(),
            snapshot: snapshot(),
            policy: policy(),
            reviews: vec![],
            reviewers: vec![EvidenceTriageReviewerCapacity {
                reviewer_id: "researcher-a".into(),
                role: "mechanism scientist".into(),
                capacity_items: 2,
                occupied_items: 0,
            }],
            current_epoch: 5,
        };
        let plan = plan_glioma_prospective_evidence_triage(&request).unwrap();
        assert_eq!(plan.queue_order, vec!["claim-a"]);
        assert_eq!(
            plan.decisions[0].action,
            EvidenceTriageActionKind::ResolveContradiction
        );
        assert_eq!(
            plan.decisions[0].reviewer_id.as_deref(),
            Some("researcher-a")
        );
        assert_eq!(plan.disposition, EvidenceTriageDisposition::Partial);
        plan.validate().unwrap();
    }

    #[test]
    fn budget_and_reviewer_capacity_defer_open_claims() {
        let mut request = EvidenceProspectiveTriageRequest {
            objective: "triage glioma evidence".into(),
            snapshot: snapshot(),
            policy: policy(),
            reviews: vec![],
            reviewers: vec![],
            current_epoch: 5,
        };
        request.policy.budget_units = 1;
        request.policy.maximum_queue_depth = 1;
        request.policy.minimum_priority_milli = 100;
        request.policy.require_reviewer_for_contradiction = false;
        let plan = plan_glioma_prospective_evidence_triage(&request).unwrap();
        assert_eq!(plan.queue_order, vec!["claim-a"]);
        assert_eq!(plan.deferred_order, vec!["claim-b"]);
        assert_eq!(plan.next_route, "glioma_knowledge_protocol_gateway");
        plan.validate().unwrap();
    }

    #[test]
    fn current_resolution_and_suppression_are_retained() {
        let request = EvidenceProspectiveTriageRequest {
            objective: "triage glioma evidence".into(),
            snapshot: snapshot(),
            policy: policy(),
            reviews: vec![
                EvidenceTriageReviewObservation {
                    claim_key: "claim-a".into(),
                    last_reviewed_epoch: 4,
                    state: EvidenceTriageReviewState::Suppressed,
                    reviewer_id: Some("researcher-a".into()),
                },
                EvidenceTriageReviewObservation {
                    claim_key: "claim-b".into(),
                    last_reviewed_epoch: 4,
                    state: EvidenceTriageReviewState::Resolved,
                    reviewer_id: Some("researcher-a".into()),
                },
            ],
            reviewers: vec![],
            current_epoch: 5,
        };
        let plan = plan_glioma_prospective_evidence_triage(&request).unwrap();
        assert_eq!(plan.held_order, vec!["claim-a"]);
        assert_eq!(plan.resolved_order, vec!["claim-b"]);
        assert_eq!(plan.disposition, EvidenceTriageDisposition::NoOpenClaims);
        plan.validate().unwrap();
    }
}
