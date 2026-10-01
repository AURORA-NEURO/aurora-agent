//! Cross-model replication frontier for autonomous preclinical glioma research.
//!
//! The claim-envelope analyzer deliberately stops at an interpretation boundary. This module
//! turns that boundary into a bounded, dependency-aware portfolio of follow-up studies without
//! pretending that a planned study is already evidence. It rewards independent model-system
//! coverage and expected heterogeneity reduction, preserves negative/model-dependent holds, and
//! can be consumed by the P07 action selector or a researcher workbench.

use super::cross_model_claim_envelope::{
    CrossModelClaimEnvelope, CrossModelClaimEnvelopeDisposition,
};
use crate::glioma_engine::{
    GliomaActionCandidate, GliomaModality, GliomaModelSystem, GliomaStageKind,
};
use bioprism_foundation::{AutonomyTier, Effect};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const COMPOSITION_ID: &str = "glioma-cross-model-replication-frontier";
pub const OUTPUT_SCHEMA: &str = "GliomaCrossModelReplicationFrontier1@1";
pub const MAX_CANDIDATES: usize = 512;
pub const MAX_SELECTED: usize = 64;
const BEAM_WIDTH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossModelFollowUpKind {
    IndependentReplication,
    AcquireMissingModelSystem,
    ResolveHeterogeneity,
    StressInfluentialStudy,
    ConfirmNegative,
    MethodsAudit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossModelReplicationCandidate {
    pub action_id: String,
    pub kind: CrossModelFollowUpKind,
    pub model_system: GliomaModelSystem,
    pub independent_group: String,
    pub rationale: String,
    pub expected_information_milli: u32,
    pub expected_range_reduction_milli: u64,
    pub reproducibility_milli: u16,
    pub feasibility_milli: u16,
    pub risk_milli: u16,
    pub cost_units: u32,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossModelReplicationFrontierRequest {
    pub source: CrossModelClaimEnvelope,
    pub candidates: Vec<CrossModelReplicationCandidate>,
    pub budget_units: u32,
    pub max_actions: u16,
    pub max_risk_milli: u16,
    pub min_information_milli: u32,
    pub target_between_system_range_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossModelReplicationFrontierDisposition {
    Ready,
    NegativeHold,
    Partial,
    Blocked,
    NoRunnableActions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossModelReplicationScore {
    pub action_id: String,
    pub kind: CrossModelFollowUpKind,
    pub model_system: GliomaModelSystem,
    pub utility_milli: u32,
    pub information_milli: u32,
    pub range_reduction_milli: u64,
    pub cost_units: u32,
    pub decision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossModelReplicationFrontier {
    pub composition_id: String,
    pub output_schema: String,
    pub objective: String,
    pub claim_id: String,
    pub source_digest: ContentHash,
    pub source_disposition: CrossModelClaimEnvelopeDisposition,
    pub candidate_order: Vec<String>,
    pub scores: Vec<CrossModelReplicationScore>,
    pub selected_order: Vec<String>,
    pub selected_model_system_order: Vec<GliomaModelSystem>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub projected_information_milli: u32,
    pub projected_between_system_range_milli: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: CrossModelReplicationFrontierDisposition,
    pub next_operator_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CrossModelReplicationFrontierError {
    #[error("cross-model replication frontier request is invalid: {0}")]
    InvalidRequest(String),
    #[error("cross-model replication frontier output is invalid: {0}")]
    InvalidOutput(String),
    #[error("cross-model replication frontier digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn need_bonus(
    source: CrossModelClaimEnvelopeDisposition,
    kind: CrossModelFollowUpKind,
    model_system: GliomaModelSystem,
    represented: &BTreeSet<GliomaModelSystem>,
) -> u32 {
    let disposition_bonus: u32 = match source {
        CrossModelClaimEnvelopeDisposition::ModelDependent => match kind {
            CrossModelFollowUpKind::ResolveHeterogeneity
            | CrossModelFollowUpKind::AcquireMissingModelSystem => 420,
            CrossModelFollowUpKind::IndependentReplication => 260,
            CrossModelFollowUpKind::StressInfluentialStudy => 180,
            _ => 80,
        },
        CrossModelClaimEnvelopeDisposition::Partial
        | CrossModelClaimEnvelopeDisposition::Unresolved => match kind {
            CrossModelFollowUpKind::AcquireMissingModelSystem
            | CrossModelFollowUpKind::IndependentReplication => 420,
            CrossModelFollowUpKind::ResolveHeterogeneity => 260,
            _ => 100,
        },
        CrossModelClaimEnvelopeDisposition::Negative => match kind {
            CrossModelFollowUpKind::ConfirmNegative => 460,
            CrossModelFollowUpKind::StressInfluentialStudy
            | CrossModelFollowUpKind::MethodsAudit => 220,
            _ => 60,
        },
        CrossModelClaimEnvelopeDisposition::Qualified => match kind {
            CrossModelFollowUpKind::IndependentReplication
            | CrossModelFollowUpKind::StressInfluentialStudy
            | CrossModelFollowUpKind::MethodsAudit => 280,
            _ => 100,
        },
    };
    let diversity_bonus = if represented.contains(&model_system) {
        0
    } else {
        180
    };
    disposition_bonus.saturating_add(diversity_bonus)
}

fn validate_request(
    request: &CrossModelReplicationFrontierRequest,
) -> Result<(), CrossModelReplicationFrontierError> {
    if request.budget_units == 0
        || request.max_actions == 0
        || usize::from(request.max_actions) > MAX_SELECTED
        || request.max_risk_milli > 1_000
        || request.min_information_milli == 0
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.target_between_system_range_milli == 0
    {
        return Err(CrossModelReplicationFrontierError::InvalidRequest(
            "bounded budget, action/risk limits, information target, range target, and candidates are required".into(),
        ));
    }
    request
        .source
        .validate()
        .map_err(|error| CrossModelReplicationFrontierError::InvalidRequest(error.to_string()))?;
    let mut ids = BTreeSet::new();
    let candidate_ids = request
        .candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<BTreeSet<_>>();
    for candidate in &request.candidates {
        if candidate.action_id.trim().is_empty()
            || candidate.independent_group.trim().is_empty()
            || candidate.rationale.trim().is_empty()
            || candidate.expected_information_milli == 0
            || candidate.reproducibility_milli > 1_000
            || candidate.feasibility_milli > 1_000
            || candidate.risk_milli > 1_000
            || candidate.cost_units == 0
            || !ids.insert(candidate.action_id.clone())
            || candidate.depends_on.iter().any(|dependency| {
                dependency == &candidate.action_id || !candidate_ids.contains(dependency)
            })
            || !canonical(&candidate.depends_on)
        {
            return Err(CrossModelReplicationFrontierError::InvalidRequest(
                "candidates require unique ids, rationale, positive information/cost, bounded scores, and canonical existing dependencies".into(),
            ));
        }
    }
    Ok(())
}

fn digest_input(frontier: &CrossModelReplicationFrontier) -> serde_json::Value {
    serde_json::json!({
        "composition_id": frontier.composition_id,
        "output_schema": frontier.output_schema,
        "objective": frontier.objective,
        "claim_id": frontier.claim_id,
        "source_digest": frontier.source_digest,
        "source_disposition": frontier.source_disposition,
        "candidate_order": frontier.candidate_order,
        "scores": frontier.scores,
        "selected_order": frontier.selected_order,
        "selected_model_system_order": frontier.selected_model_system_order,
        "deferred_order": frontier.deferred_order,
        "blocked_order": frontier.blocked_order,
        "projected_information_milli": frontier.projected_information_milli,
        "projected_between_system_range_milli": frontier.projected_between_system_range_milli,
        "negative_evidence": frontier.negative_evidence,
        "uncertainty": frontier.uncertainty,
        "disposition": frontier.disposition,
        "next_operator_action": frontier.next_operator_action,
    })
}

impl CrossModelReplicationFrontier {
    pub fn validate(&self) -> Result<(), CrossModelReplicationFrontierError> {
        if self.composition_id != COMPOSITION_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.claim_id.trim().is_empty()
            || !canonical(&self.candidate_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.selected_model_system_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.selected_order.iter().any(|id| {
                !self.candidate_order.binary_search(id).is_ok()
                    || self.deferred_order.binary_search(id).is_ok()
                    || self.blocked_order.binary_search(id).is_ok()
            })
            || self.deferred_order.iter().any(|id| {
                self.blocked_order.binary_search(id).is_ok()
                    || self.selected_order.binary_search(id).is_ok()
            })
        {
            return Err(CrossModelReplicationFrontierError::InvalidOutput(
                "identity, ordering, candidate partition, or evidence invariants are invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| CrossModelReplicationFrontierError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(CrossModelReplicationFrontierError::InvalidOutput(
                "frontier digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PortfolioState {
    selected: Vec<usize>,
    spent: u32,
    risk: u32,
    utility: u64,
    information: u64,
    range_reduction: u64,
    systems: BTreeSet<GliomaModelSystem>,
    actions: BTreeSet<String>,
}

fn select_portfolio(
    candidates: &[CrossModelReplicationCandidate],
    utilities: &[u32],
    source_range: u64,
    budget: u32,
    max_actions: usize,
    max_risk: u32,
) -> BTreeSet<String> {
    let mut states = vec![PortfolioState {
        selected: Vec::new(),
        spent: 0,
        risk: 0,
        utility: 0,
        information: 0,
        range_reduction: 0,
        systems: BTreeSet::new(),
        actions: BTreeSet::new(),
    }];
    for (index, candidate) in candidates.iter().enumerate() {
        let mut next = states.clone();
        for state in &states {
            if state.selected.len() >= max_actions
                || state.spent.saturating_add(candidate.cost_units) > budget
                || state.risk.saturating_add(u32::from(candidate.risk_milli)) > max_risk
                || candidate
                    .depends_on
                    .iter()
                    .any(|dependency| !state.actions.contains(dependency))
            {
                continue;
            }
            let mut selected = state.selected.clone();
            selected.push(index);
            let mut systems = state.systems.clone();
            let new_system = systems.insert(candidate.model_system);
            let mut actions = state.actions.clone();
            actions.insert(candidate.action_id.clone());
            let information = state
                .information
                .saturating_add(u64::from(candidate.expected_information_milli));
            let range_reduction = state
                .range_reduction
                .saturating_add(candidate.expected_range_reduction_milli)
                .min(source_range);
            let diversity_bonus = if new_system { 25_000 } else { 0 };
            next.push(PortfolioState {
                selected,
                spent: state.spent.saturating_add(candidate.cost_units),
                risk: state.risk.saturating_add(u32::from(candidate.risk_milli)),
                utility: state
                    .utility
                    .saturating_add(u64::from(utilities[index]))
                    .saturating_add(diversity_bonus),
                information,
                range_reduction,
                systems,
                actions,
            });
        }
        next.sort_by(|left, right| {
            right
                .utility
                .cmp(&left.utility)
                .then_with(|| right.information.cmp(&left.information))
                .then_with(|| left.spent.cmp(&right.spent))
                .then_with(|| left.risk.cmp(&right.risk))
                .then_with(|| {
                    let left_ids = left
                        .selected
                        .iter()
                        .map(|index| candidates[*index].action_id.as_str())
                        .collect::<Vec<_>>();
                    let right_ids = right
                        .selected
                        .iter()
                        .map(|index| candidates[*index].action_id.as_str())
                        .collect::<Vec<_>>();
                    left_ids.cmp(&right_ids)
                })
        });
        next.truncate(BEAM_WIDTH);
        states = next;
    }
    states
        .first()
        .map(|state| {
            state
                .selected
                .iter()
                .map(|index| candidates[*index].action_id.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Compile the next independent, model-diverse follow-up portfolio for a cross-model claim.
pub fn plan_glioma_cross_model_replication_frontier(
    request: &CrossModelReplicationFrontierRequest,
) -> Result<CrossModelReplicationFrontier, CrossModelReplicationFrontierError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let represented = request
        .source
        .model_system_order
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut utilities = Vec::with_capacity(candidates.len());
    let mut scores = Vec::with_capacity(candidates.len());
    for candidate in &candidates {
        let need = need_bonus(
            request.source.disposition,
            candidate.kind,
            candidate.model_system,
            &represented,
        );
        let reliability = u32::from(candidate.reproducibility_milli.min(1_000));
        let feasibility = u32::from(candidate.feasibility_milli.min(1_000));
        let risk_discount = 1_000_u32.saturating_sub(u32::from(candidate.risk_milli));
        let raw = u64::from(
            candidate
                .expected_information_milli
                .min(1_000_000)
                .saturating_add(candidate.expected_range_reduction_milli.min(1_000_000) as u32),
        )
        .saturating_mul(u64::from(need))
        .saturating_mul(u64::from(reliability.saturating_add(feasibility) / 2))
        .saturating_mul(u64::from(risk_discount))
            / 1_000_000;
        let utility = raw
            .saturating_sub(u64::from(candidate.cost_units.saturating_mul(35)))
            .min(u64::from(u32::MAX)) as u32;
        utilities.push(utility);
        scores.push(CrossModelReplicationScore {
            action_id: candidate.action_id.clone(),
            kind: candidate.kind,
            model_system: candidate.model_system,
            utility_milli: utility,
            information_milli: candidate.expected_information_milli,
            range_reduction_milli: candidate.expected_range_reduction_milli,
            cost_units: candidate.cost_units,
            decision: "eligible-until-portfolio-gate".into(),
        });
    }
    let selected = select_portfolio(
        &candidates,
        &utilities,
        request.source.between_system_range_milli,
        request.budget_units,
        usize::from(request.max_actions),
        u32::from(request.max_risk_milli),
    );
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<Vec<_>>();
    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let selected_candidates = candidates
        .iter()
        .filter(|candidate| selected.contains(&candidate.action_id))
        .collect::<Vec<_>>();
    let selected_systems = selected_candidates
        .iter()
        .map(|candidate| candidate.model_system)
        .collect::<BTreeSet<_>>();
    let projected_information = selected_candidates
        .iter()
        .map(|candidate| candidate.expected_information_milli as u64)
        .sum::<u64>()
        .min(u64::from(u32::MAX)) as u32;
    let projected_reduction = selected_candidates
        .iter()
        .map(|candidate| candidate.expected_range_reduction_milli)
        .sum::<u64>()
        .min(request.source.between_system_range_milli);
    let projected_range = request
        .source
        .between_system_range_milli
        .saturating_sub(projected_reduction);
    let blocked = candidates
        .iter()
        .filter(|candidate| {
            candidate
                .depends_on
                .iter()
                .any(|dependency| !selected.contains(dependency))
                && !selected.contains(&candidate.action_id)
        })
        .map(|candidate| candidate.action_id.clone())
        .collect::<Vec<_>>();
    let deferred = candidates
        .iter()
        .filter(|candidate| {
            !selected.contains(&candidate.action_id) && !blocked.contains(&candidate.action_id)
        })
        .map(|candidate| candidate.action_id.clone())
        .collect::<Vec<_>>();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if matches!(
        request.source.disposition,
        CrossModelClaimEnvelopeDisposition::ModelDependent
    ) {
        negative_evidence.push("source-claim-is-model-dependent".into());
    }
    if matches!(
        request.source.disposition,
        CrossModelClaimEnvelopeDisposition::Negative
    ) {
        negative_evidence.push("source-envelope-is-practically-negative".into());
    }
    if selected.is_empty() {
        uncertainty.push("no-budget-compatible-follow-up-was-selected".into());
    }
    if projected_information < request.min_information_milli {
        uncertainty.push(format!(
            "projected-information {} below required {}",
            projected_information, request.min_information_milli
        ));
    }
    if projected_range > request.target_between_system_range_milli {
        uncertainty.push(format!(
            "projected-between-system-range {} above target {}",
            projected_range, request.target_between_system_range_milli
        ));
    }
    if selected_systems.len() < request.source.model_system_order.len()
        && matches!(
            request.source.disposition,
            CrossModelClaimEnvelopeDisposition::ModelDependent
                | CrossModelClaimEnvelopeDisposition::Partial
                | CrossModelClaimEnvelopeDisposition::Unresolved
        )
    {
        uncertainty.push("selected-portfolio-does-not-touch-every-represented-system".into());
    }
    let disposition = if selected.is_empty() && !blocked.is_empty() {
        CrossModelReplicationFrontierDisposition::Blocked
    } else if selected.is_empty() {
        CrossModelReplicationFrontierDisposition::NoRunnableActions
    } else if projected_information < request.min_information_milli {
        CrossModelReplicationFrontierDisposition::Partial
    } else if matches!(
        request.source.disposition,
        CrossModelClaimEnvelopeDisposition::Negative
    ) {
        CrossModelReplicationFrontierDisposition::NegativeHold
    } else if projected_range > request.target_between_system_range_milli {
        CrossModelReplicationFrontierDisposition::Partial
    } else {
        CrossModelReplicationFrontierDisposition::Ready
    };
    let next_operator_action = match disposition {
        CrossModelReplicationFrontierDisposition::Ready => {
            "admit-selected-cross-model-follow-up-to-p07-after-local-authority-check".into()
        }
        CrossModelReplicationFrontierDisposition::NegativeHold => {
            "preserve-negative-envelope-and-review-confirmatory-work".into()
        }
        CrossModelReplicationFrontierDisposition::Partial => {
            "defer-claim-and-acquire-the-highest-ranked-independent-coverage".into()
        }
        CrossModelReplicationFrontierDisposition::Blocked
        | CrossModelReplicationFrontierDisposition::NoRunnableActions => {
            "hold-autonomous-dispatch-and-request-researcher-review".into()
        }
    };
    let mut output = CrossModelReplicationFrontier {
        composition_id: COMPOSITION_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.source.objective.clone(),
        claim_id: request.source.claim_id.clone(),
        source_digest: request.source.digest.clone(),
        source_disposition: request.source.disposition,
        candidate_order,
        scores,
        selected_order,
        selected_model_system_order: selected_systems.into_iter().collect(),
        deferred_order: sorted_unique(deferred),
        blocked_order: sorted_unique(blocked),
        projected_information_milli: projected_information,
        projected_between_system_range_milli: projected_range,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        disposition,
        next_operator_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-cross-model-replication-frontier"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| CrossModelReplicationFrontierError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

/// Materialize the selected frontier entries into the P07 action contract. This does not execute
/// them or grant authority; it only makes the scientific frontier consumable by the existing
/// dependency/risk/budget scheduler.
pub fn materialize_glioma_cross_model_replication_actions(
    request: &CrossModelReplicationFrontierRequest,
    frontier: &CrossModelReplicationFrontier,
) -> Result<Vec<GliomaActionCandidate>, CrossModelReplicationFrontierError> {
    frontier.validate()?;
    if frontier.source_digest != request.source.digest {
        return Err(CrossModelReplicationFrontierError::InvalidRequest(
            "frontier source digest does not match request source".into(),
        ));
    }
    let selected = frontier.selected_order.iter().collect::<BTreeSet<_>>();
    let candidates = request
        .candidates
        .iter()
        .filter(|candidate| selected.contains(&candidate.action_id))
        .map(|candidate| {
            let modality = match candidate.kind {
                CrossModelFollowUpKind::MethodsAudit
                | CrossModelFollowUpKind::StressInfluentialStudy => GliomaModality::Computational,
                CrossModelFollowUpKind::AcquireMissingModelSystem => GliomaModality::AnimalModel,
                CrossModelFollowUpKind::IndependentReplication
                | CrossModelFollowUpKind::ResolveHeterogeneity
                | CrossModelFollowUpKind::ConfirmNegative => GliomaModality::Replication,
            };
            let information = candidate.expected_information_milli.min(1_000) as u16;
            let range_unlock = candidate.expected_range_reduction_milli.min(1_000) as u16;
            GliomaActionCandidate {
                action_id: candidate.action_id.clone(),
                stage_kind: GliomaStageKind::ReplicationRobustness,
                modality,
                model_system: candidate.model_system,
                depends_on: candidate.depends_on.clone(),
                cost_units: candidate.cost_units,
                information_gain_milli: information,
                frontier_novelty_milli: 700,
                workflow_leverage_milli: range_unlock,
                cross_stage_unlock_milli: range_unlock,
                reproducibility_safety_milli: candidate.reproducibility_milli,
                federation_value_milli: 0,
                feasibility_milli: candidate.feasibility_milli,
                autonomy_tier: AutonomyTier::A1,
                effects: [
                    Effect::ReadLocalData,
                    Effect::ExecuteLocalComputation,
                    Effect::WriteLocalArtifact,
                ]
                .into_iter()
                .collect(),
            }
        })
        .collect::<Vec<_>>();
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p10_interpretation_replication::cross_model_claim_envelope::{
        analyze_glioma_cross_model_claim_envelope, CrossModelClaimEnvelopeRequest,
        CrossModelStudyEstimate,
    };
    use crate::glioma_engine::LocalArtifactRef;

    fn source() -> CrossModelClaimEnvelope {
        let artifact = |id: &str| LocalArtifactRef {
            artifact_id: format!("artifact-{id}"),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        };
        let estimate = |id: &str, study: &str, system, low, high| CrossModelStudyEstimate {
            estimate_id: id.into(),
            study_id: study.into(),
            model_system: system,
            independent_group: format!("group-{study}"),
            interval_low_milli: low,
            interval_high_milli: high,
            quality_milli: 900,
            uncertainty_milli: 100,
            artifact: artifact(id),
        };
        analyze_glioma_cross_model_claim_envelope(&CrossModelClaimEnvelopeRequest {
            objective: "test follow-up frontier".into(),
            claim_id: "claim-frontier".into(),
            min_model_systems: 2,
            min_studies_per_system: 2,
            practical_effect_milli: 50,
            hidden_bias_budget_milli: 10,
            max_between_system_range_milli: 20,
            estimates: vec![
                estimate("o1", "organoid-1", GliomaModelSystem::Organoid, 200, 400),
                estimate("o2", "organoid-2", GliomaModelSystem::Organoid, 210, 410),
                estimate("m1", "mouse-1", GliomaModelSystem::MouseModel, 80, 400),
                estimate("m2", "mouse-2", GliomaModelSystem::MouseModel, 90, 410),
            ],
        })
        .unwrap()
    }

    fn candidate(
        id: &str,
        kind: CrossModelFollowUpKind,
        system: GliomaModelSystem,
        group: &str,
        reduction: u64,
    ) -> CrossModelReplicationCandidate {
        CrossModelReplicationCandidate {
            action_id: id.into(),
            kind,
            model_system: system,
            independent_group: group.into(),
            rationale: "resolve cross-model uncertainty".into(),
            expected_information_milli: 700,
            expected_range_reduction_milli: reduction,
            reproducibility_milli: 900,
            feasibility_milli: 800,
            risk_milli: 100,
            cost_units: 2,
            depends_on: Vec::new(),
        }
    }

    fn request() -> CrossModelReplicationFrontierRequest {
        CrossModelReplicationFrontierRequest {
            source: source(),
            candidates: vec![
                candidate(
                    "new-organoid-site",
                    CrossModelFollowUpKind::IndependentReplication,
                    GliomaModelSystem::Organoid,
                    "new-organoid-group",
                    20,
                ),
                candidate(
                    "new-xenograft-site",
                    CrossModelFollowUpKind::AcquireMissingModelSystem,
                    GliomaModelSystem::PatientDerivedXenograft,
                    "new-xenograft-group",
                    120,
                ),
                candidate(
                    "mouse-heterogeneity",
                    CrossModelFollowUpKind::ResolveHeterogeneity,
                    GliomaModelSystem::MouseModel,
                    "new-mouse-group",
                    60,
                ),
            ],
            budget_units: 4,
            max_actions: 2,
            max_risk_milli: 500,
            min_information_milli: 600,
            target_between_system_range_milli: 40,
        }
    }

    #[test]
    fn frontier_prefers_new_model_system_when_claim_is_model_dependent() {
        let output = plan_glioma_cross_model_replication_frontier(&request()).unwrap();
        assert_eq!(
            output.disposition,
            CrossModelReplicationFrontierDisposition::Ready
        );
        assert!(output
            .selected_model_system_order
            .contains(&GliomaModelSystem::PatientDerivedXenograft));
        assert!(output.projected_between_system_range_milli <= 40);
        let actions =
            materialize_glioma_cross_model_replication_actions(&request(), &output).unwrap();
        assert_eq!(actions.len(), output.selected_order.len());
        assert!(actions.iter().all(|action| {
            action.stage_kind == GliomaStageKind::ReplicationRobustness
                && action.autonomy_tier == AutonomyTier::A1
                && action.effects.contains(&Effect::ExecuteLocalComputation)
        }));
        output.validate().unwrap();
    }

    #[test]
    fn frontier_is_permutation_stable_and_preserves_budget_hold() {
        let mut request = request();
        request.candidates.reverse();
        let first = plan_glioma_cross_model_replication_frontier(&request).unwrap();
        request.budget_units = 2;
        request.min_information_milli = 1_200;
        let second = plan_glioma_cross_model_replication_frontier(&request).unwrap();
        assert_eq!(
            first.candidate_order,
            vec![
                "mouse-heterogeneity",
                "new-organoid-site",
                "new-xenograft-site"
            ]
        );
        assert_eq!(
            second.disposition,
            CrossModelReplicationFrontierDisposition::Partial
        );
        assert!(!second.uncertainty.is_empty());
    }

    #[test]
    fn frontier_marks_dependency_ineligible_work_as_blocked() {
        let mut request = request();
        request.budget_units = 1;
        request.candidates[1].cost_units = 4;
        let mut dependent = candidate(
            "dependent-follow-up",
            CrossModelFollowUpKind::IndependentReplication,
            GliomaModelSystem::PatientDerivedXenograft,
            "dependent-group",
            40,
        );
        dependent.depends_on = vec!["new-xenograft-site".into()];
        request.candidates.push(dependent);
        let output = plan_glioma_cross_model_replication_frontier(&request).unwrap();
        assert_eq!(
            output.disposition,
            CrossModelReplicationFrontierDisposition::Blocked
        );
        assert_eq!(output.blocked_order, vec!["dependent-follow-up"]);
    }
}
