//! Autonomous planning over federated continual glioma knowledge.
//!
//! This feature is the agent layer above the aggregate continual knowledge primitive. It ranks
//! bounded research actions using uncertainty, missing coverage, cross-site influence, and
//! contradiction pressure, then applies budget, dependency, autonomy, and human-authorization
//! gates. It returns a typed plan only; it never executes an instrument, moves raw data, or makes
//! a clinical decision.

use super::federated_continual::{FederatedContinualClaimDisposition, FederatedContinualKnowledge};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F12";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedContinualAgent1@1";
pub const MAX_CANDIDATES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedAgentActionKind {
    AcquireCoverage,
    ReplicateHighInfluence,
    AdjudicateContradiction,
    HarmonizeModality,
    ContinueMonitor,
    QuarantineNegative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAgentCandidate {
    pub action_id: String,
    pub claim_key: String,
    pub kind: FederatedAgentActionKind,
    pub cost_milli: u32,
    pub expected_information_milli: u16,
    pub required_autonomy_tier: u8,
    pub physical_effect: bool,
    pub human_authorized: bool,
    pub depends_on: Vec<String>,
    pub target_site_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualAgentRequest {
    pub objective: String,
    pub knowledge: FederatedContinualKnowledge,
    pub candidates: Vec<FederatedAgentCandidate>,
    pub budget_milli: u32,
    pub max_actions: usize,
    pub allowed_autonomy_tier: u8,
    pub min_expected_information_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedAgentDecision {
    Selected,
    ApprovalRequired,
    Blocked,
    Deferred,
    Quarantined,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAgentPlanItem {
    pub action_id: String,
    pub claim_key: String,
    pub kind: FederatedAgentActionKind,
    pub priority_score_milli: u32,
    pub cost_milli: u32,
    pub expected_information_milli: u16,
    pub decision: FederatedAgentDecision,
    pub depends_on: Vec<String>,
    pub target_site_order: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedAgentDisposition {
    Selected,
    ApprovalRequired,
    PartiallyBlocked,
    Quarantined,
    NoRunnableAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualAgentPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub source_knowledge_digest: ContentHash,
    pub plan_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub approval_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub quarantined_order: Vec<String>,
    pub items: Vec<FederatedAgentPlanItem>,
    pub budget_milli: u32,
    pub spent_milli: u32,
    pub remaining_budget_milli: u32,
    pub omitted_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedAgentDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedContinualAgentError {
    #[error("federated continual agent request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated continual agent output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated continual agent digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &FederatedContinualAgentPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "source_knowledge_digest": output.source_knowledge_digest,
        "plan_order": output.plan_order,
        "selected_order": output.selected_order,
        "approval_order": output.approval_order,
        "blocked_order": output.blocked_order,
        "deferred_order": output.deferred_order,
        "quarantined_order": output.quarantined_order,
        "items": output.items,
        "budget_milli": output.budget_milli,
        "spent_milli": output.spent_milli,
        "remaining_budget_milli": output.remaining_budget_milli,
        "omitted_order": output.omitted_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl FederatedContinualAgentPlan {
    pub fn validate(&self) -> Result<(), FederatedContinualAgentError> {
        let item_ids = self
            .items
            .iter()
            .map(|item| item.action_id.clone())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.plan_order)
            || item_ids != self.plan_order
            || !canonical(&self.selected_order)
            || !canonical(&self.approval_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.quarantined_order)
            || !canonical(&self.omitted_order)
            || !canonical(&self.uncertainty)
            || self.spent_milli > self.budget_milli
            || self.remaining_budget_milli != self.budget_milli - self.spent_milli
            || self.items.iter().any(|item| {
                item.action_id.trim().is_empty()
                    || item.claim_key.trim().is_empty()
                    || item.priority_score_milli > 4_000_000
                    || item.expected_information_milli > 1_000
                    || !canonical(&item.depends_on)
                    || !canonical(&item.target_site_order)
            })
        {
            return Err(FederatedContinualAgentError::InvalidOutput(
                "identity, ordering, budget arithmetic, priority, or canonical fields are invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedContinualAgentError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedContinualAgentError::InvalidOutput(
                "federated continual agent digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &FederatedContinualAgentRequest,
) -> Result<(), FederatedContinualAgentError> {
    if request.objective.trim().is_empty()
        || request.max_actions == 0
        || request.candidates.len() > MAX_CANDIDATES
        || request.allowed_autonomy_tier > 4
        || request.min_expected_information_milli > 1_000
    {
        return Err(FederatedContinualAgentError::InvalidRequest(
            "objective, candidate/action capacity, autonomy tier, or information floor is invalid"
                .into(),
        ));
    }
    request
        .knowledge
        .validate()
        .map_err(|error| FederatedContinualAgentError::InvalidRequest(error.to_string()))?;
    if request.knowledge.objective != request.objective {
        return Err(FederatedContinualAgentError::InvalidRequest(
            "agent objective must bind to the source knowledge objective".into(),
        ));
    }
    let claim_keys = request
        .knowledge
        .claims
        .iter()
        .map(|claim| claim.claim_key.clone())
        .collect::<BTreeSet<_>>();
    let mut action_ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.action_id.trim().is_empty()
            || candidate.claim_key.trim().is_empty()
            || !claim_keys.contains(&candidate.claim_key)
            || !action_ids.insert(candidate.action_id.clone())
            || candidate.cost_milli == 0
            || candidate.required_autonomy_tier > 4
            || candidate.expected_information_milli > 1_000
            || !canonical(&candidate.depends_on)
            || !canonical(&candidate.target_site_order)
        {
            return Err(FederatedContinualAgentError::InvalidRequest(
                "candidate identity, claim binding, uniqueness, cost, autonomy, information, or ordering is invalid".into(),
            ));
        }
        if candidate.physical_effect && !candidate.human_authorized {
            return Err(FederatedContinualAgentError::InvalidRequest(
                "physical-effect candidates require explicit human authorization".into(),
            ));
        }
        if candidate
            .depends_on
            .iter()
            .any(|dependency| dependency == &candidate.action_id)
        {
            return Err(FederatedContinualAgentError::InvalidRequest(
                "candidate cannot depend on itself".into(),
            ));
        }
    }
    for candidate in &request.candidates {
        if candidate
            .depends_on
            .iter()
            .any(|dependency| !action_ids.contains(dependency))
        {
            return Err(FederatedContinualAgentError::InvalidRequest(
                "candidate dependency does not name a supplied action".into(),
            ));
        }
    }
    fn visit(
        action_id: &str,
        candidates: &BTreeMap<String, &FederatedAgentCandidate>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if visited.contains(action_id) {
            return true;
        }
        if !visiting.insert(action_id.to_string()) {
            return false;
        }
        let acyclic = candidates
            .get(action_id)
            .map(|candidate| {
                candidate
                    .depends_on
                    .iter()
                    .all(|dependency| visit(dependency, candidates, visiting, visited))
            })
            .unwrap_or(false);
        visiting.remove(action_id);
        if acyclic {
            visited.insert(action_id.to_string());
        }
        acyclic
    }
    let candidate_map = request
        .candidates
        .iter()
        .map(|candidate| (candidate.action_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    if candidate_map
        .keys()
        .any(|action_id| !visit(action_id, &candidate_map, &mut visiting, &mut visited))
    {
        return Err(FederatedContinualAgentError::InvalidRequest(
            "candidate dependency graph contains a cycle".into(),
        ));
    }
    Ok(())
}

fn dependency_depth(
    action_id: &str,
    candidates: &BTreeMap<String, &FederatedAgentCandidate>,
    memo: &mut BTreeMap<String, usize>,
) -> usize {
    if let Some(depth) = memo.get(action_id) {
        return *depth;
    }
    let depth = candidates
        .get(action_id)
        .map(|candidate| {
            candidate
                .depends_on
                .iter()
                .map(|dependency| dependency_depth(dependency, candidates, memo) + 1)
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    memo.insert(action_id.to_string(), depth);
    depth
}

fn priority_score(
    claim: &super::federated_continual::FederatedContinualClaim,
    candidate: &FederatedAgentCandidate,
) -> u32 {
    let latest_epoch = claim.epochs.last();
    let coverage_debt =
        (claim.missing_modality_order.len() + claim.missing_model_system_order.len()) as u32;
    let contradiction = claim.contradiction_observation_count.min(1_000) as u32;
    let influence = u32::from(claim.leave_one_site_out_influence_milli);
    let trend_pressure = match claim.trend {
        super::federated_continual::FederatedContinualTrend::Weakening => 900,
        super::federated_continual::FederatedContinualTrend::Contested => 1_000,
        super::federated_continual::FederatedContinualTrend::Negative => 800,
        super::federated_continual::FederatedContinualTrend::Insufficient => 700,
        super::federated_continual::FederatedContinualTrend::Strengthening => 250,
        super::federated_continual::FederatedContinualTrend::Stable => 100,
    };
    let confidence_debt = latest_epoch
        .map(|epoch| 1_000_u32.saturating_sub(u32::from(epoch.robust_confidence_milli)))
        .unwrap_or(1_000);
    let action_leverage = u32::from(candidate.expected_information_milli);
    (35 * action_leverage
        + 20 * coverage_debt.min(1_000)
        + 15 * influence
        + 15 * contradiction
        + 10 * trend_pressure
        + 5 * confidence_debt)
        .min(4_000_000)
}

fn rationale(
    claim: &super::federated_continual::FederatedContinualClaim,
    candidate: &FederatedAgentCandidate,
) -> String {
    format!(
        "{} targets {} with {} support, {} influence, {} contradiction observations, and {} missing coverage dimensions",
        candidate.action_id,
        claim.claim_key,
        claim.latest_support_milli,
        claim.leave_one_site_out_influence_milli,
        claim.contradiction_observation_count,
        claim.missing_modality_order.len() + claim.missing_model_system_order.len()
    )
}

pub fn plan_federated_continual_agent(
    request: &FederatedContinualAgentRequest,
) -> Result<FederatedContinualAgentPlan, FederatedContinualAgentError> {
    validate_request(request)?;
    let claim_map = request
        .knowledge
        .claims
        .iter()
        .map(|claim| (claim.claim_key.clone(), claim))
        .collect::<BTreeMap<_, _>>();
    let candidate_map = request
        .candidates
        .iter()
        .map(|candidate| (candidate.action_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut ranked = request
        .candidates
        .iter()
        .map(|candidate| {
            let claim = claim_map
                .get(&candidate.claim_key)
                .expect("validated candidate claim exists");
            let score = priority_score(claim, candidate);
            let density =
                (score as u64 * 1_000_000 / u64::from(candidate.cost_milli.max(1))) as u32;
            (candidate, score, density)
        })
        .collect::<Vec<_>>();
    let mut dependency_depths = BTreeMap::new();
    for candidate in &request.candidates {
        dependency_depth(&candidate.action_id, &candidate_map, &mut dependency_depths);
    }
    ranked.sort_by(|left, right| {
        dependency_depths[&left.0.action_id]
            .cmp(&dependency_depths[&right.0.action_id])
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.0.action_id.cmp(&right.0.action_id))
    });
    let mut selected = BTreeSet::new();
    let mut selected_order = Vec::new();
    let mut approval_order = Vec::new();
    let mut blocked_order = Vec::new();
    let mut deferred_order = Vec::new();
    let mut quarantined_order = Vec::new();
    let mut omitted_order = Vec::new();
    let mut uncertainty = Vec::new();
    let mut items = Vec::new();
    let mut spent_milli = 0_u32;
    for (candidate, score, _) in ranked {
        let claim = claim_map
            .get(&candidate.claim_key)
            .expect("validated candidate claim exists");
        let mut decision = FederatedAgentDecision::Selected;
        let mut reason = rationale(claim, candidate);
        if matches!(
            claim.disposition,
            FederatedContinualClaimDisposition::Quarantined
        ) || matches!(candidate.kind, FederatedAgentActionKind::QuarantineNegative)
        {
            decision = FederatedAgentDecision::Quarantined;
            quarantined_order.push(candidate.action_id.clone());
            reason.push_str("; quarantine is mandatory before any promotion");
        } else if candidate.required_autonomy_tier > request.allowed_autonomy_tier {
            decision = FederatedAgentDecision::Blocked;
            blocked_order.push(candidate.action_id.clone());
            reason.push_str("; autonomy tier exceeds the request grant");
        } else if candidate.physical_effect {
            decision = FederatedAgentDecision::ApprovalRequired;
            approval_order.push(candidate.action_id.clone());
            reason.push_str("; physical effect requires a separate signed human approval");
        } else if candidate.expected_information_milli < request.min_expected_information_milli {
            decision = FederatedAgentDecision::Deferred;
            deferred_order.push(candidate.action_id.clone());
            reason.push_str("; expected information is below the request floor");
        } else if candidate
            .depends_on
            .iter()
            .any(|dependency| !selected.contains(dependency))
        {
            decision = FederatedAgentDecision::Blocked;
            blocked_order.push(candidate.action_id.clone());
            reason.push_str("; dependency closure is not selected");
        } else if selected_order.len() >= request.max_actions {
            decision = FederatedAgentDecision::Deferred;
            deferred_order.push(candidate.action_id.clone());
            reason.push_str("; action capacity is exhausted");
        } else if spent_milli.saturating_add(candidate.cost_milli) > request.budget_milli {
            decision = FederatedAgentDecision::Deferred;
            deferred_order.push(candidate.action_id.clone());
            reason.push_str("; budget is insufficient");
        } else {
            selected.insert(candidate.action_id.clone());
            selected_order.push(candidate.action_id.clone());
            spent_milli = spent_milli.saturating_add(candidate.cost_milli);
        }
        if !matches!(decision, FederatedAgentDecision::Selected) {
            omitted_order.push(format!("{}: {:?}", candidate.action_id, decision));
        }
        items.push(FederatedAgentPlanItem {
            action_id: candidate.action_id.clone(),
            claim_key: candidate.claim_key.clone(),
            kind: candidate.kind,
            priority_score_milli: score,
            cost_milli: candidate.cost_milli,
            expected_information_milli: candidate.expected_information_milli,
            decision,
            depends_on: candidate.depends_on.clone(),
            target_site_order: candidate.target_site_order.clone(),
            rationale: reason,
        });
    }
    items.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    selected_order.sort();
    approval_order.sort();
    blocked_order.sort();
    deferred_order.sort();
    quarantined_order.sort();
    omitted_order.sort();
    for item in &items {
        if matches!(item.decision, FederatedAgentDecision::ApprovalRequired) {
            uncertainty.push(format!(
                "{}: physical execution remains outside this plan",
                item.action_id
            ));
        }
    }
    uncertainty.sort();
    uncertainty.dedup();
    let plan_order = items
        .iter()
        .map(|item| item.action_id.clone())
        .collect::<Vec<_>>();
    let disposition = if !quarantined_order.is_empty() {
        FederatedAgentDisposition::Quarantined
    } else if selected_order.is_empty() && approval_order.is_empty() {
        FederatedAgentDisposition::NoRunnableAction
    } else if !blocked_order.is_empty() {
        FederatedAgentDisposition::PartiallyBlocked
    } else if !approval_order.is_empty() {
        FederatedAgentDisposition::ApprovalRequired
    } else {
        FederatedAgentDisposition::Selected
    };
    let next_step = match disposition {
        FederatedAgentDisposition::Selected => {
            "hand selected non-physical actions to the bounded local action dispatcher"
        }
        FederatedAgentDisposition::ApprovalRequired => {
            "obtain signed human authorization before any physical-effect candidate is admitted"
        }
        FederatedAgentDisposition::PartiallyBlocked => {
            "resolve autonomy or dependency blocks before dispatching the selected frontier"
        }
        FederatedAgentDisposition::Quarantined => {
            "retain quarantined negative claims and route them to local adjudication or replication"
        }
        FederatedAgentDisposition::NoRunnableAction => {
            "increase evidence coverage, budget, or autonomy grant before another planning cycle"
        }
    };
    let mut output = FederatedContinualAgentPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        source_knowledge_digest: request.knowledge.digest.clone(),
        plan_order,
        selected_order,
        approval_order,
        blocked_order,
        deferred_order,
        quarantined_order,
        items,
        budget_milli: request.budget_milli,
        spent_milli,
        remaining_budget_milli: request.budget_milli - spent_milli,
        omitted_order,
        uncertainty,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| FederatedContinualAgentError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedContinualAgentError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p02_evidence_knowledge::federated_continual::{
        analyze_federated_continual_knowledge, FederatedContinualKnowledgeRequest,
        FederatedContinualObservation,
    };
    use crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::KnowledgeClaimDisposition;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
    use std::collections::BTreeSet;

    fn digest(key: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"claim": key})).unwrap()
    }

    fn observation(
        key: &str,
        site: &str,
        epoch: u32,
        disposition: KnowledgeClaimDisposition,
    ) -> FederatedContinualObservation {
        FederatedContinualObservation {
            claim_key: key.into(),
            claim_digest: digest(key),
            site_id: site.into(),
            epoch,
            support_milli: if matches!(disposition, KnowledgeClaimDisposition::Negative) {
                100
            } else {
                820
            },
            confidence_milli: 900,
            contradiction_milli: if matches!(disposition, KnowledgeClaimDisposition::Contested) {
                900
            } else {
                0
            },
            source_count: 3,
            independent_artifact_count: 2,
            modality_order: vec![GliomaModality::Transcriptomics],
            model_system_order: vec![GliomaModelSystem::Organoid],
            disposition,
            exportable: true,
            preclinical_only: true,
        }
    }

    fn knowledge(disposition: KnowledgeClaimDisposition) -> FederatedContinualKnowledge {
        let mut observations = Vec::new();
        for epoch in 1..=2 {
            observations.extend([
                observation("egfr-invasion", "site-a", epoch, disposition),
                observation("egfr-invasion", "site-b", epoch, disposition),
                observation("egfr-invasion", "site-c", epoch, disposition),
            ]);
        }
        analyze_federated_continual_knowledge(&FederatedContinualKnowledgeRequest {
            objective: "federated agent".into(),
            observations,
            min_sites: 3,
            min_epochs: 2,
            min_consensus_support_milli: 700,
            drift_threshold_milli: 150,
            outlier_threshold_milli: 250,
            required_modalities: BTreeSet::from([GliomaModality::Transcriptomics]),
            required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            max_claims: 8,
        })
        .unwrap()
    }

    fn candidate(kind: FederatedAgentActionKind) -> FederatedAgentCandidate {
        FederatedAgentCandidate {
            action_id: format!("action-{kind:?}"),
            claim_key: "egfr-invasion".into(),
            kind,
            cost_milli: 100,
            expected_information_milli: 900,
            required_autonomy_tier: 1,
            physical_effect: false,
            human_authorized: false,
            depends_on: Vec::new(),
            target_site_order: vec!["site-a".into()],
        }
    }

    #[test]
    fn selects_high_information_action_within_budget() {
        let output = plan_federated_continual_agent(&FederatedContinualAgentRequest {
            objective: "federated agent".into(),
            knowledge: knowledge(KnowledgeClaimDisposition::Supported),
            candidates: vec![candidate(FederatedAgentActionKind::ReplicateHighInfluence)],
            budget_milli: 100,
            max_actions: 1,
            allowed_autonomy_tier: 1,
            min_expected_information_milli: 500,
        })
        .unwrap();
        assert_eq!(output.disposition, FederatedAgentDisposition::Selected);
        assert_eq!(output.selected_order, vec!["action-ReplicateHighInfluence"]);
        output.validate().expect("digest validates");
    }

    #[test]
    fn physical_effect_is_approval_required_not_selected() {
        let mut action = candidate(FederatedAgentActionKind::AcquireCoverage);
        action.action_id = "physical".into();
        action.physical_effect = true;
        action.human_authorized = true;
        let output = plan_federated_continual_agent(&FederatedContinualAgentRequest {
            objective: "federated agent".into(),
            knowledge: knowledge(KnowledgeClaimDisposition::Supported),
            candidates: vec![action],
            budget_milli: 100,
            max_actions: 1,
            allowed_autonomy_tier: 1,
            min_expected_information_milli: 500,
        })
        .unwrap();
        assert_eq!(output.approval_order, vec!["physical"]);
        assert!(output.selected_order.is_empty());
    }

    #[test]
    fn negative_knowledge_is_quarantined() {
        let output = plan_federated_continual_agent(&FederatedContinualAgentRequest {
            objective: "federated agent".into(),
            knowledge: knowledge(KnowledgeClaimDisposition::Negative),
            candidates: vec![candidate(FederatedAgentActionKind::ContinueMonitor)],
            budget_milli: 100,
            max_actions: 1,
            allowed_autonomy_tier: 1,
            min_expected_information_milli: 500,
        })
        .unwrap();
        assert_eq!(output.disposition, FederatedAgentDisposition::Quarantined);
        assert_eq!(output.quarantined_order, vec!["action-ContinueMonitor"]);
    }

    #[test]
    fn dependency_closure_is_selected_before_dependent_action() {
        let mut prerequisite = candidate(FederatedAgentActionKind::AcquireCoverage);
        prerequisite.action_id = "base".into();
        let mut dependent = candidate(FederatedAgentActionKind::ReplicateHighInfluence);
        dependent.action_id = "dependent".into();
        dependent.depends_on = vec!["base".into()];
        let output = plan_federated_continual_agent(&FederatedContinualAgentRequest {
            objective: "federated agent".into(),
            knowledge: knowledge(KnowledgeClaimDisposition::Supported),
            candidates: vec![dependent, prerequisite],
            budget_milli: 200,
            max_actions: 2,
            allowed_autonomy_tier: 1,
            min_expected_information_milli: 500,
        })
        .unwrap();
        assert_eq!(output.selected_order, vec!["base", "dependent"]);
        assert!(output.blocked_order.is_empty());
    }
}
