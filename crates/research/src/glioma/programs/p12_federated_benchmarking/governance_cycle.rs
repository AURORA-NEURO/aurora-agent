//! Consortium benchmark-governance cycle for autonomous preclinical glioma federation.
//!
//! The cycle is a typed state machine for proposal, site review, privacy approval, analysis,
//! dissent reconciliation, release, and correction.  It never turns absent votes into approvals:
//! every transition has an authorized actor, policy version, rationale, and local scope.  Policy
//! changes invalidate affected decisions and dissent remains part of the immutable record.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F16";
pub const OUTPUT_SCHEMA: &str = "GliomaBenchmarkGovernanceCycle1@1";
pub const MAX_TRANSITIONS: usize = 256;
pub const MAX_VOTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceStage {
    Proposal,
    SiteReview,
    PrivacyApproval,
    Analysis,
    DissentReconciliation,
    Release,
    Correction,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceTransitionDecision {
    Advance,
    Hold,
    Reject,
    Correct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceVoteDecision {
    Approve,
    Reject,
    Abstain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceCycleStatus {
    InProgress,
    Held,
    Blocked,
    Corrective,
    Released,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceTransition {
    pub transition_id: String,
    pub from_stage: GovernanceStage,
    pub to_stage: GovernanceStage,
    pub actor_id: String,
    pub actor_role: String,
    pub site_id: Option<String>,
    pub policy_version: String,
    pub authorized: bool,
    pub decision: GovernanceTransitionDecision,
    pub rationale_digest: ContentHash,
    pub epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceVote {
    pub vote_id: String,
    pub site_id: String,
    pub actor_id: String,
    pub policy_version: String,
    pub authorized: bool,
    pub decision: GovernanceVoteDecision,
    pub rationale_digest: ContentHash,
    pub epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceCycleRequest {
    pub proposal_id: String,
    pub benchmark_id: String,
    pub policy_version: String,
    pub proposal_digest: ContentHash,
    pub current_epoch: u64,
    pub required_quorum: u16,
    pub transitions: Vec<GovernanceTransition>,
    pub votes: Vec<GovernanceVote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceCycleRecord {
    pub feature_id: String,
    pub output_schema: String,
    pub proposal_id: String,
    pub benchmark_id: String,
    pub policy_version: String,
    pub proposal_digest: ContentHash,
    pub transition_order: Vec<String>,
    pub authorized_transition_order: Vec<String>,
    pub approved_vote_order: Vec<String>,
    pub rejected_vote_order: Vec<String>,
    pub abstained_vote_order: Vec<String>,
    pub omitted_vote_order: Vec<String>,
    pub dissent_order: Vec<String>,
    pub next_review_trigger_order: Vec<String>,
    pub current_stage: GovernanceStage,
    pub status: GovernanceCycleStatus,
    pub cycle_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GovernanceCycleError {
    #[error("governance cycle request is invalid: {0}")]
    InvalidRequest(String),
    #[error("governance cycle record is invalid: {0}")]
    InvalidOutput(String),
    #[error("governance cycle digest failed: {0}")]
    Digest(String),
}

fn identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn stage_rank(stage: GovernanceStage) -> usize {
    match stage {
        GovernanceStage::Proposal => 0,
        GovernanceStage::SiteReview => 1,
        GovernanceStage::PrivacyApproval => 2,
        GovernanceStage::Analysis => 3,
        GovernanceStage::DissentReconciliation => 4,
        GovernanceStage::Release => 5,
        GovernanceStage::Correction => 6,
        GovernanceStage::Closed => 7,
    }
}

fn digest_input(output: &GovernanceCycleRecord) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "proposal_id": output.proposal_id,
        "benchmark_id": output.benchmark_id,
        "policy_version": output.policy_version,
        "proposal_digest": output.proposal_digest,
        "transition_order": output.transition_order,
        "authorized_transition_order": output.authorized_transition_order,
        "approved_vote_order": output.approved_vote_order,
        "rejected_vote_order": output.rejected_vote_order,
        "abstained_vote_order": output.abstained_vote_order,
        "omitted_vote_order": output.omitted_vote_order,
        "dissent_order": output.dissent_order,
        "next_review_trigger_order": output.next_review_trigger_order,
        "current_stage": output.current_stage,
        "status": output.status,
    })
}

fn validate_request(request: &GovernanceCycleRequest) -> Result<(), GovernanceCycleError> {
    if !identifier(&request.proposal_id)
        || !identifier(&request.benchmark_id)
        || !identifier(&request.policy_version)
        || !digest(&request.proposal_digest)
        || request.current_epoch == 0
        || request.required_quorum == 0
        || request.transitions.is_empty()
        || request.transitions.len() > MAX_TRANSITIONS
        || request.votes.len() > MAX_VOTES
    {
        return Err(GovernanceCycleError::InvalidRequest(
            "proposal, benchmark, policy, digest, epoch, quorum, and bounded governance events are required".into(),
        ));
    }
    let mut transition_ids = BTreeSet::new();
    for transition in &request.transitions {
        if !identifier(&transition.transition_id)
            || !transition_ids.insert(transition.transition_id.clone())
            || !identifier(&transition.actor_id)
            || !identifier(&transition.actor_role)
            || transition
                .site_id
                .as_deref()
                .is_some_and(|site| !identifier(site))
            || !identifier(&transition.policy_version)
            || !digest(&transition.rationale_digest)
            || transition.epoch == 0
        {
            return Err(GovernanceCycleError::InvalidRequest(
                "transitions require unique bounded identity, actor scope, policy, rationale, and epoch".into(),
            ));
        }
    }
    let mut vote_ids = BTreeSet::new();
    for vote in &request.votes {
        if !identifier(&vote.vote_id)
            || !vote_ids.insert(vote.vote_id.clone())
            || !identifier(&vote.site_id)
            || !identifier(&vote.actor_id)
            || !identifier(&vote.policy_version)
            || !digest(&vote.rationale_digest)
            || vote.epoch == 0
        {
            return Err(GovernanceCycleError::InvalidRequest(
                "votes require unique bounded identity, local actor/site scope, policy, rationale, and epoch".into(),
            ));
        }
    }
    Ok(())
}

impl GovernanceCycleRecord {
    pub fn validate(&self) -> Result<(), GovernanceCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !identifier(&self.proposal_id)
            || !identifier(&self.benchmark_id)
            || !identifier(&self.policy_version)
            || !digest(&self.proposal_digest)
            || !canonical(&self.transition_order)
            || !canonical(&self.authorized_transition_order)
            || !canonical(&self.approved_vote_order)
            || !canonical(&self.rejected_vote_order)
            || !canonical(&self.abstained_vote_order)
            || !canonical(&self.omitted_vote_order)
            || !canonical(&self.dissent_order)
            || !canonical(&self.next_review_trigger_order)
            || !digest(&self.cycle_digest)
        {
            return Err(GovernanceCycleError::InvalidOutput(
                "cycle identity, canonical decision partitions, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GovernanceCycleError::Digest(error.to_string()))?;
        if expected != self.cycle_digest {
            return Err(GovernanceCycleError::InvalidOutput(
                "cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile a governance-cycle record while preserving local votes, dissent, abstentions, and
/// policy invalidation.  Missing votes never become approvals.
pub fn compile_glioma_benchmark_governance_cycle(
    request: &GovernanceCycleRequest,
) -> Result<GovernanceCycleRecord, GovernanceCycleError> {
    validate_request(request)?;
    let mut transitions = request.transitions.clone();
    transitions.sort_by(|left, right| {
        left.epoch
            .cmp(&right.epoch)
            .then_with(|| left.transition_id.cmp(&right.transition_id))
    });
    let mut transition_order = Vec::new();
    let mut authorized_transition_order = Vec::new();
    let mut dissent = BTreeSet::new();
    let mut triggers = BTreeSet::new();
    let mut current_stage = GovernanceStage::Proposal;
    let mut status = GovernanceCycleStatus::InProgress;
    for transition in &transitions {
        transition_order.push(transition.transition_id.clone());
        if transition.policy_version != request.policy_version {
            dissent.insert(format!(
                "{}:policy-version-invalidated",
                transition.transition_id
            ));
            triggers.insert("policy-version-change-requires-review".to_string());
            status = GovernanceCycleStatus::Corrective;
            continue;
        }
        if !transition.authorized || stage_rank(transition.to_stage) > stage_rank(current_stage) + 1
        {
            dissent.insert(format!(
                "{}:unauthorized-or-nonadjacent",
                transition.transition_id
            ));
            triggers.insert("authorized-transition-required".to_string());
            status = GovernanceCycleStatus::Blocked;
            continue;
        }
        authorized_transition_order.push(transition.transition_id.clone());
        if transition.from_stage != current_stage {
            dissent.insert(format!(
                "{}:stage-precondition-failed",
                transition.transition_id
            ));
            triggers.insert("stage-precondition-reconciliation".to_string());
            status = GovernanceCycleStatus::Blocked;
            continue;
        }
        match transition.decision {
            GovernanceTransitionDecision::Advance => current_stage = transition.to_stage,
            GovernanceTransitionDecision::Hold => {
                dissent.insert(format!("{}:hold", transition.transition_id));
                triggers.insert("next-review-after-hold".to_string());
                status = GovernanceCycleStatus::Held;
            }
            GovernanceTransitionDecision::Reject => {
                dissent.insert(format!("{}:reject", transition.transition_id));
                triggers.insert("proposal-rejection-requires-correction".to_string());
                status = GovernanceCycleStatus::Blocked;
            }
            GovernanceTransitionDecision::Correct => {
                current_stage = GovernanceStage::Correction;
                triggers.insert("correction-review-required".to_string());
                status = GovernanceCycleStatus::Corrective;
            }
        }
    }
    let mut approved = Vec::new();
    let mut rejected = Vec::new();
    let mut abstained = Vec::new();
    let mut omitted = Vec::new();
    for vote in &request.votes {
        if vote.policy_version != request.policy_version {
            omitted.push(vote.vote_id.clone());
            dissent.insert(format!("{}:policy-version-invalidated", vote.vote_id));
            triggers.insert("policy-version-change-requires-review".to_string());
        } else if !vote.authorized {
            omitted.push(vote.vote_id.clone());
            dissent.insert(format!("{}:unauthorized-vote", vote.vote_id));
        } else {
            match vote.decision {
                GovernanceVoteDecision::Approve => approved.push(vote.vote_id.clone()),
                GovernanceVoteDecision::Reject => {
                    rejected.push(vote.vote_id.clone());
                    dissent.insert(format!("{}:reject", vote.vote_id));
                }
                GovernanceVoteDecision::Abstain => {
                    abstained.push(vote.vote_id.clone());
                    dissent.insert(format!("{}:abstain", vote.vote_id));
                }
            }
        }
    }
    approved.sort();
    rejected.sort();
    abstained.sort();
    omitted.sort();
    if approved.len() < request.required_quorum as usize {
        triggers.insert("quorum-not-met-no-implicit-approval".to_string());
        if status == GovernanceCycleStatus::InProgress {
            status = GovernanceCycleStatus::Held;
        }
    }
    if !rejected.is_empty() {
        status = GovernanceCycleStatus::Blocked;
        triggers.insert("rejected-vote-requires-dissent-reconciliation".to_string());
    }
    if current_stage == GovernanceStage::Release && status == GovernanceCycleStatus::InProgress {
        status = GovernanceCycleStatus::Released;
    }
    if current_stage == GovernanceStage::Closed && status == GovernanceCycleStatus::InProgress {
        status = GovernanceCycleStatus::Closed;
    }
    let mut output = GovernanceCycleRecord {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        proposal_id: request.proposal_id.clone(),
        benchmark_id: request.benchmark_id.clone(),
        policy_version: request.policy_version.clone(),
        proposal_digest: request.proposal_digest.clone(),
        transition_order,
        authorized_transition_order,
        approved_vote_order: approved,
        rejected_vote_order: rejected,
        abstained_vote_order: abstained,
        omitted_vote_order: omitted,
        dissent_order: dissent.into_iter().collect(),
        next_review_trigger_order: triggers.into_iter().collect(),
        current_stage,
        status,
        cycle_digest: ContentHash::of_bytes(b"unsealed-glioma-benchmark-governance-cycle"),
    };
    output.cycle_digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GovernanceCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn transition(
        id: &str,
        from_stage: GovernanceStage,
        to_stage: GovernanceStage,
    ) -> GovernanceTransition {
        GovernanceTransition {
            transition_id: id.into(),
            from_stage,
            to_stage,
            actor_id: "board-a".into(),
            actor_role: "governance-chair".into(),
            site_id: None,
            policy_version: "policy-1".into(),
            authorized: true,
            decision: GovernanceTransitionDecision::Advance,
            rationale_digest: hash(id),
            epoch: 100,
        }
    }

    fn request() -> GovernanceCycleRequest {
        GovernanceCycleRequest {
            proposal_id: "proposal-1".into(),
            benchmark_id: "benchmark-1".into(),
            policy_version: "policy-1".into(),
            proposal_digest: hash("proposal"),
            current_epoch: 100,
            required_quorum: 1,
            transitions: vec![transition(
                "proposal-to-review",
                GovernanceStage::Proposal,
                GovernanceStage::SiteReview,
            )],
            votes: vec![GovernanceVote {
                vote_id: "vote-a".into(),
                site_id: "site-a".into(),
                actor_id: "pi-a".into(),
                policy_version: "policy-1".into(),
                authorized: true,
                decision: GovernanceVoteDecision::Approve,
                rationale_digest: hash("vote"),
                epoch: 100,
            }],
        }
    }

    #[test]
    fn authorized_transition_and_vote_remain_replayable() {
        let output = compile_glioma_benchmark_governance_cycle(&request()).unwrap();
        assert_eq!(output.status, GovernanceCycleStatus::InProgress);
        assert_eq!(output.current_stage, GovernanceStage::SiteReview);
        assert_eq!(output.approved_vote_order, vec!["vote-a"]);
        output.validate().unwrap();
    }

    #[test]
    fn absent_or_insufficient_votes_hold_without_implicit_approval() {
        let mut request = request();
        request.required_quorum = 2;
        let output = compile_glioma_benchmark_governance_cycle(&request).unwrap();
        assert_eq!(output.status, GovernanceCycleStatus::Held);
        assert!(output
            .next_review_trigger_order
            .iter()
            .any(|trigger| trigger.contains("no-implicit-approval")));
    }

    #[test]
    fn dissent_and_rejection_block_release() {
        let mut request = request();
        request.votes[0].decision = GovernanceVoteDecision::Reject;
        let output = compile_glioma_benchmark_governance_cycle(&request).unwrap();
        assert_eq!(output.status, GovernanceCycleStatus::Blocked);
        assert_eq!(output.rejected_vote_order, vec!["vote-a"]);
        assert!(output
            .dissent_order
            .iter()
            .any(|entry| entry == "vote-a:reject"));
    }

    #[test]
    fn policy_change_invalidates_prior_decision() {
        let mut request = request();
        request.transitions[0].policy_version = "policy-0".into();
        let output = compile_glioma_benchmark_governance_cycle(&request).unwrap();
        assert_eq!(output.status, GovernanceCycleStatus::Corrective);
        assert!(output
            .dissent_order
            .iter()
            .any(|entry| entry == "proposal-to-review:policy-version-invalidated"));
    }

    #[test]
    fn unauthorized_or_mutated_cycle_cannot_pass_validation() {
        let mut request = request();
        request.transitions[0].authorized = false;
        let output = compile_glioma_benchmark_governance_cycle(&request).unwrap();
        assert_eq!(output.status, GovernanceCycleStatus::Blocked);
        let mut mutated = output;
        mutated.policy_version = "tampered".into();
        assert!(matches!(
            mutated.validate(),
            Err(GovernanceCycleError::InvalidOutput(_))
        ));
    }
}
