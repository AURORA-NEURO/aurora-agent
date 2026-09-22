//! Multi-fidelity mechanism control for preclinical glioma research.
//!
//! This feature turns an assimilated mechanism ledger and a cross-model transport bridge into
//! a bounded escalation frontier. It chooses the least expensive declared model-system upgrade
//! that can reduce uncertainty or transport debt, while keeping contradictions, missing target
//! coverage, approval requirements, and risk visible. It plans research actions only: it never
//! creates an observation, executes an assay, moves raw data, or makes a clinical decision.

use super::evidence_assimilation::{AssimilatedMechanismStatus, MechanismEvidenceAssimilation};
use super::fidelity_bridge::MechanismFidelityBridge;
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F11";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismMultiFidelityControl1@1";
pub const MAX_CANDIDATES: usize = 1_024;
pub const MAX_SELECTED: usize = 256;

/// A declared model-system upgrade is ordered only for control-budget purposes. The ordering is
/// not a claim that one preclinical model is universally superior to another.
fn model_rank(model: GliomaModelSystem) -> u8 {
    match model {
        GliomaModelSystem::InSilico => 0,
        GliomaModelSystem::CellLine => 1,
        GliomaModelSystem::Organoid => 2,
        GliomaModelSystem::ZebrafishModel => 3,
        GliomaModelSystem::MouseModel => 4,
        GliomaModelSystem::PatientDerivedXenograft => 5,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiFidelityControlCandidate {
    pub action_id: String,
    pub mechanism_id: String,
    pub current_model_system: GliomaModelSystem,
    pub target_model_system: GliomaModelSystem,
    pub expected_information_gain_milli: u16,
    pub expected_transport_gain_milli: u16,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub reproducibility_milli: u16,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiFidelityControlRequest {
    pub objective: String,
    pub assimilation: MechanismEvidenceAssimilation,
    pub fidelity_bridge: MechanismFidelityBridge,
    pub candidates: Vec<MultiFidelityControlCandidate>,
    pub budget_units: u64,
    pub max_actions: usize,
    pub minimum_information_gain_milli: u16,
    pub minimum_transport_gain_milli: u16,
    pub maximum_risk_milli: u16,
    pub allow_approval_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiFidelityControlActionScore {
    pub action_id: String,
    pub mechanism_id: String,
    pub current_model_system: GliomaModelSystem,
    pub target_model_system: GliomaModelSystem,
    pub priority_milli: u16,
    pub information_gain_milli: u16,
    pub transport_gap_milli: u16,
    pub uncertainty_milli: u16,
    pub escalation_gap_milli: u16,
    pub eligible: bool,
    pub exclusion_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiFidelityControlDecision {
    pub mechanism_id: String,
    pub status: AssimilatedMechanismStatus,
    pub posterior_milli: u16,
    pub transport_milli: u16,
    pub target_observed: bool,
    pub next_action_id: Option<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiFidelityControlDisposition {
    Ready,
    Partial,
    Blocked,
    NoEligibleActions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiFidelityControlPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub assimilation_digest: ContentHash,
    pub fidelity_digest: ContentHash,
    pub mechanism_order: Vec<String>,
    pub decisions: Vec<MultiFidelityControlDecision>,
    pub candidate_order: Vec<String>,
    pub ranked_action_order: Vec<String>,
    pub selected_action_order: Vec<String>,
    pub deferred_action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub scores: Vec<MultiFidelityControlActionScore>,
    pub budget_units: u64,
    pub total_cost_units: u64,
    pub budget_remaining_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MultiFidelityControlDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultiFidelityControlError {
    #[error("multi-fidelity control request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-fidelity control output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-fidelity control digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(plan: &MultiFidelityControlPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "assimilation_digest": plan.assimilation_digest,
        "fidelity_digest": plan.fidelity_digest,
        "mechanism_order": plan.mechanism_order,
        "decisions": plan.decisions,
        "candidate_order": plan.candidate_order,
        "ranked_action_order": plan.ranked_action_order,
        "selected_action_order": plan.selected_action_order,
        "deferred_action_order": plan.deferred_action_order,
        "blocked_action_order": plan.blocked_action_order,
        "scores": plan.scores,
        "budget_units": plan.budget_units,
        "total_cost_units": plan.total_cost_units,
        "budget_remaining_units": plan.budget_remaining_units,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
    })
}

impl MultiFidelityControlPlan {
    pub fn validate(&self) -> Result<(), MultiFidelityControlError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.assimilation_digest.as_str().len() != 64
            || self.fidelity_digest.as_str().len() != 64
            || !canonical(&self.mechanism_order)
            || !unique_nonempty(&self.mechanism_order)
            || !canonical(&self.candidate_order)
            || !canonical(&self.deferred_action_order)
            || !canonical(&self.blocked_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || !unique_nonempty(&self.candidate_order)
            || !unique_nonempty(&self.ranked_action_order)
            || !unique_nonempty(&self.selected_action_order)
            || !unique_nonempty(&self.deferred_action_order)
            || !unique_nonempty(&self.blocked_action_order)
            || self.scores.iter().any(|score| {
                score.action_id.trim().is_empty()
                    || score.mechanism_id.trim().is_empty()
                    || score.priority_milli > 1_000
                    || score.information_gain_milli > 1_000
                    || score.transport_gap_milli > 1_000
                    || score.uncertainty_milli > 1_000
                    || score.escalation_gap_milli > 1_000
            })
        {
            return Err(MultiFidelityControlError::InvalidOutput(
                "identity, ordering, score bounds, or digest shape is invalid".into(),
            ));
        }
        let score_ids = self
            .scores
            .iter()
            .map(|score| score.action_id.clone())
            .collect::<BTreeSet<_>>();
        let ranked_ids = self
            .ranked_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let selected_ids = self
            .selected_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let deferred_ids = self
            .deferred_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let blocked_ids = self
            .blocked_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let partition_union = selected_ids
            .union(&deferred_ids)
            .cloned()
            .collect::<BTreeSet<_>>()
            .union(&blocked_ids)
            .cloned()
            .collect::<BTreeSet<_>>();
        let decision_order = self
            .decisions
            .iter()
            .map(|decision| decision.mechanism_id.clone())
            .collect::<Vec<_>>();
        if self.scores.len() != score_ids.len()
            || self.candidate_order.len() != score_ids.len()
            || score_ids
                != self
                    .candidate_order
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
            || ranked_ids != score_ids
            || self.ranked_action_order
                != self
                    .scores
                    .iter()
                    .map(|score| score.action_id.clone())
                    .collect::<Vec<_>>()
            || selected_ids.len() + deferred_ids.len() + blocked_ids.len() != partition_union.len()
            || partition_union != score_ids
            || decision_order != self.mechanism_order
            || self
                .total_cost_units
                .saturating_add(self.budget_remaining_units)
                != self.budget_units
        {
            return Err(MultiFidelityControlError::InvalidOutput(
                "candidate partitions, decision order, or budget accounting do not reconcile"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultiFidelityControlError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultiFidelityControlError::Digest(
                "multi-fidelity control digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &MultiFidelityControlRequest,
) -> Result<(), MultiFidelityControlError> {
    if request.objective.trim().is_empty()
        || request.assimilation.objective != request.objective
        || request.fidelity_bridge.objective != request.objective
        || request.candidates.len() > MAX_CANDIDATES
        || request.max_actions == 0
        || request.max_actions > MAX_SELECTED
        || request.budget_units == 0
        || request.minimum_information_gain_milli > 1_000
        || request.minimum_transport_gain_milli > 1_000
        || request.maximum_risk_milli > 1_000
    {
        return Err(MultiFidelityControlError::InvalidRequest(
            "objective/ledger binding, bounded candidates/actions, budget, and thresholds are required".into(),
        ));
    }
    request
        .assimilation
        .validate()
        .map_err(|error| MultiFidelityControlError::InvalidRequest(error.to_string()))?;
    request
        .fidelity_bridge
        .validate()
        .map_err(|error| MultiFidelityControlError::InvalidRequest(error.to_string()))?;
    let mechanism_ids = request
        .assimilation
        .mechanism_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let bridge_ids = request
        .fidelity_bridge
        .mechanism_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut action_ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.action_id.trim().is_empty()
            || !mechanism_ids.contains(&candidate.mechanism_id)
            || !bridge_ids.contains(&candidate.mechanism_id)
            || !action_ids.insert(candidate.action_id.clone())
            || candidate.expected_information_gain_milli > 1_000
            || candidate.expected_transport_gain_milli > 1_000
            || candidate.risk_milli > 1_000
            || candidate.reproducibility_milli > 1_000
            || model_rank(candidate.target_model_system)
                <= model_rank(candidate.current_model_system)
        {
            return Err(MultiFidelityControlError::InvalidRequest(
                "candidate identity, bridge/ledger coverage, score bounds, and strict model escalation are required".into(),
            ));
        }
    }
    Ok(())
}

fn priority_score(
    candidate: &MultiFidelityControlCandidate,
    posterior_milli: u16,
    transport_milli: u16,
) -> (u16, u16, u16, u16, u16) {
    let uncertainty = 1_000_u16.saturating_sub(posterior_milli);
    let transport_gap = 1_000_u16.saturating_sub(transport_milli);
    let escalation_gap = (u16::from(
        model_rank(candidate.target_model_system)
            .saturating_sub(model_rank(candidate.current_model_system)),
    ) * 250)
        .min(1_000);
    let raw = (u32::from(candidate.expected_information_gain_milli) * 30
        + u32::from(transport_gap) * 25
        + u32::from(uncertainty) * 20
        + u32::from(candidate.expected_transport_gain_milli) * 15
        + u32::from(candidate.reproducibility_milli) * 10)
        / 100;
    let penalty = (u32::from(candidate.risk_milli) * 20
        + u32::try_from(candidate.cost_units.min(1_000)).unwrap_or(1_000) * 5)
        / 100;
    let score = raw.saturating_sub(penalty).min(1_000) as u16;
    (
        score,
        candidate.expected_information_gain_milli,
        transport_gap,
        uncertainty,
        escalation_gap,
    )
}

/// Select bounded model-system upgrades that reduce mechanistic transport debt before a
/// downstream action planner or protocol executor is allowed to dispatch work.
pub fn plan_glioma_mechanism_multi_fidelity_control(
    request: &MultiFidelityControlRequest,
) -> Result<MultiFidelityControlPlan, MultiFidelityControlError> {
    validate_request(request)?;
    let assimilation_records = request
        .assimilation
        .records
        .iter()
        .map(|record| (record.mechanism_id.clone(), record))
        .collect::<BTreeMap<_, _>>();
    let fidelity_records = request
        .fidelity_bridge
        .records
        .iter()
        .map(|record| (record.mechanism_id.clone(), record))
        .collect::<BTreeMap<_, _>>();
    let mut scores = Vec::new();
    for candidate in &request.candidates {
        let assimilation = assimilation_records
            .get(&candidate.mechanism_id)
            .expect("validated assimilation mechanism");
        let fidelity = fidelity_records
            .get(&candidate.mechanism_id)
            .expect("validated fidelity mechanism");
        let (priority, _gain, transport_gap, uncertainty, escalation_gap) = priority_score(
            candidate,
            assimilation.posterior_milli,
            fidelity.transport_milli,
        );
        let exclusion_reason = if candidate.expected_information_gain_milli
            < request.minimum_information_gain_milli
        {
            Some("information-gain-below-threshold".into())
        } else if candidate.expected_transport_gain_milli < request.minimum_transport_gain_milli {
            Some("transport-gain-below-threshold".into())
        } else if candidate.risk_milli > request.maximum_risk_milli {
            Some("risk-above-threshold".into())
        } else if candidate.requires_approval && !request.allow_approval_required {
            Some("approval-required-by-policy".into())
        } else {
            None
        };
        scores.push(MultiFidelityControlActionScore {
            action_id: candidate.action_id.clone(),
            mechanism_id: candidate.mechanism_id.clone(),
            current_model_system: candidate.current_model_system,
            target_model_system: candidate.target_model_system,
            priority_milli: priority,
            information_gain_milli: candidate.expected_information_gain_milli,
            transport_gap_milli: transport_gap,
            uncertainty_milli: uncertainty,
            escalation_gap_milli: escalation_gap,
            eligible: exclusion_reason.is_none(),
            exclusion_reason,
        });
    }
    scores.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let ranked_action_order = scores
        .iter()
        .map(|score| score.action_id.clone())
        .collect::<Vec<_>>();
    let candidate_order = {
        let mut values = ranked_action_order.clone();
        values.sort();
        values
    };
    let candidate_map = request
        .candidates
        .iter()
        .map(|candidate| (candidate.action_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut selected = Vec::new();
    let mut deferred = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut spent = 0_u64;
    for score in &scores {
        let candidate = candidate_map
            .get(&score.action_id)
            .expect("validated candidate");
        if !score.eligible {
            blocked.insert(score.action_id.clone());
        } else if selected.len() < request.max_actions
            && spent.saturating_add(u64::from(candidate.cost_units)) <= request.budget_units
        {
            spent = spent.saturating_add(u64::from(candidate.cost_units));
            selected.push(score.action_id.clone());
        } else {
            deferred.insert(score.action_id.clone());
        }
    }
    let mut decisions = Vec::new();
    let mut uncertainty = request.assimilation.uncertainty.clone();
    let mut negative_evidence = request.assimilation.negative_evidence.clone();
    negative_evidence.extend(request.fidelity_bridge.negative_evidence.iter().cloned());
    for mechanism_id in &request.assimilation.mechanism_order {
        let assimilation = assimilation_records
            .get(mechanism_id)
            .expect("validated assimilation record");
        let fidelity = fidelity_records
            .get(mechanism_id)
            .expect("validated fidelity record");
        let next_action_id = selected
            .iter()
            .find(|action_id| candidate_map[*action_id].mechanism_id == *mechanism_id)
            .cloned();
        let rationale = match (
            assimilation.status,
            fidelity.target_observed,
            next_action_id.is_some(),
        ) {
            (AssimilatedMechanismStatus::Contradicted, _, true) => {
                "contradiction pressure selects a bounded higher-fidelity discriminator".into()
            }
            (AssimilatedMechanismStatus::Unresolved, false, true) => {
                "unresolved posterior and missing target coverage select a fidelity escalation"
                    .into()
            }
            (AssimilatedMechanismStatus::Supported, false, true) => {
                "supported posterior lacks target coverage; selected action validates transport"
                    .into()
            }
            (_, false, false) => {
                uncertainty.push(format!("{mechanism_id}:target-model-coverage-unresolved"));
                "target model remains unobserved; no eligible budgeted escalation selected".into()
            }
            (_, true, true) => {
                "target coverage exists; selected action validates transport stability".into()
            }
            (_, true, false) => "target coverage exists; no eligible escalation selected".into(),
        };
        decisions.push(MultiFidelityControlDecision {
            mechanism_id: mechanism_id.clone(),
            status: assimilation.status,
            posterior_milli: assimilation.posterior_milli,
            transport_milli: fidelity.transport_milli,
            target_observed: fidelity.target_observed,
            next_action_id,
            rationale,
        });
    }
    uncertainty.sort();
    uncertainty.dedup();
    negative_evidence.sort();
    negative_evidence.dedup();
    let disposition = if request.fidelity_bridge.disposition
        == super::fidelity_bridge::MechanismFidelityBridgeDisposition::Blocked
        || request.assimilation.disposition
            == super::evidence_assimilation::MechanismEvidenceAssimilationDisposition::Blocked
    {
        MultiFidelityControlDisposition::Blocked
    } else if selected.is_empty() && !request.candidates.is_empty() {
        MultiFidelityControlDisposition::NoEligibleActions
    } else if selected.len() < request.candidates.len() || !uncertainty.is_empty() {
        MultiFidelityControlDisposition::Partial
    } else {
        MultiFidelityControlDisposition::Ready
    };
    let mut output = MultiFidelityControlPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        assimilation_digest: request.assimilation.digest.clone(),
        fidelity_digest: request.fidelity_bridge.digest.clone(),
        mechanism_order: request.assimilation.mechanism_order.clone(),
        decisions,
        candidate_order,
        ranked_action_order,
        selected_action_order: selected,
        deferred_action_order: deferred.into_iter().collect(),
        blocked_action_order: blocked.into_iter().collect(),
        scores,
        budget_units: request.budget_units,
        total_cost_units: spent,
        budget_remaining_units: request.budget_units.saturating_sub(spent),
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultiFidelityControlError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::bayesian_update::{
        update_glioma_mechanism_posterior, BayesianMechanismHypothesis,
        BayesianMechanismUpdateRequest,
    };
    use super::super::discrimination::{MechanismFeatureObservation, MechanismPrediction};
    use super::super::evidence_assimilation::{
        assimilate_glioma_mechanism_evidence, MechanismEvidenceAssimilationRequest,
        MechanismEvidenceSnapshot,
    };
    use super::super::fidelity_bridge::{
        bridge_glioma_mechanism_fidelity, MechanismFidelityBridgeRequest,
        MechanismFidelityObservation,
    };
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn mechanism_update(
        value: i64,
        label: &str,
    ) -> super::super::bayesian_update::MechanismBayesianUpdate {
        let observation = MechanismFeatureObservation {
            feature_id: "growth".into(),
            observed_milli: value,
            uncertainty_milli: 10,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{label}"),
                content_hash: ContentHash::of_bytes(label.as_bytes()),
                content_type: "tabular-feature".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        };
        update_glioma_mechanism_posterior(
            &BayesianMechanismUpdateRequest {
                objective: "multi-fidelity mechanism control".into(),
                model_system: GliomaModelSystem::Organoid,
                min_shared_features: 1,
                max_hypotheses: 4,
                likelihood_scale_milli: 100,
                supported_posterior_floor_milli: 600,
                contradicted_posterior_ceiling_milli: 100,
            },
            &[
                BayesianMechanismHypothesis {
                    mechanism_id: "near".into(),
                    statement: "near mechanism".into(),
                    prior_milli: 500,
                    predictions: vec![MechanismPrediction {
                        feature_id: "growth".into(),
                        predicted_milli: 100,
                        uncertainty_milli: 10,
                    }],
                },
                BayesianMechanismHypothesis {
                    mechanism_id: "far".into(),
                    statement: "far mechanism".into(),
                    prior_milli: 500,
                    predictions: vec![MechanismPrediction {
                        feature_id: "growth".into(),
                        predicted_milli: 900,
                        uncertainty_milli: 10,
                    }],
                },
            ],
            &[observation],
        )
        .unwrap()
    }

    fn fidelity_observation(
        id: &str,
        mechanism_id: &str,
        model_system: GliomaModelSystem,
        observed_milli: i64,
    ) -> MechanismFidelityObservation {
        MechanismFidelityObservation {
            observation_id: id.into(),
            mechanism_id: mechanism_id.into(),
            model_system,
            predicted_milli: 100,
            observed_milli,
            uncertainty_milli: 100,
            artifact: LocalArtifactRef {
                artifact_id: format!("fidelity-{id}"),
                content_hash: ContentHash::of_bytes(id.as_bytes()),
                content_type: "application/vnd.aurora.glioma-feature+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }
    }

    fn request() -> MultiFidelityControlRequest {
        let assimilation =
            assimilate_glioma_mechanism_evidence(&MechanismEvidenceAssimilationRequest {
                objective: "multi-fidelity mechanism control".into(),
                snapshots: vec![MechanismEvidenceSnapshot {
                    epoch_id: "epoch-1".into(),
                    update: mechanism_update(110, "first"),
                }],
                recency_decay_milli: 800,
                max_snapshots: 8,
                supported_floor_milli: 600,
                contradicted_ceiling_milli: 100,
                preserve_negative_results: true,
            })
            .unwrap();
        let fidelity_bridge = bridge_glioma_mechanism_fidelity(&MechanismFidelityBridgeRequest {
            objective: "multi-fidelity mechanism control".into(),
            target_model_system: GliomaModelSystem::Organoid,
            observations: vec![
                fidelity_observation("near-cell", "near", GliomaModelSystem::CellLine, 110),
                fidelity_observation("near-organoid", "near", GliomaModelSystem::Organoid, 120),
                fidelity_observation("far-cell", "far", GliomaModelSystem::CellLine, 900),
                fidelity_observation("far-organoid", "far", GliomaModelSystem::Organoid, 850),
            ],
            min_model_systems: 2,
            min_transport_milli: 700,
            max_observations: 16,
        })
        .unwrap();
        MultiFidelityControlRequest {
            objective: "multi-fidelity mechanism control".into(),
            assimilation,
            fidelity_bridge,
            candidates: vec![
                MultiFidelityControlCandidate {
                    action_id: "near-escalation".into(),
                    mechanism_id: "near".into(),
                    current_model_system: GliomaModelSystem::CellLine,
                    target_model_system: GliomaModelSystem::Organoid,
                    expected_information_gain_milli: 850,
                    expected_transport_gain_milli: 800,
                    cost_units: 3,
                    risk_milli: 100,
                    reproducibility_milli: 900,
                    requires_approval: false,
                },
                MultiFidelityControlCandidate {
                    action_id: "far-escalation".into(),
                    mechanism_id: "far".into(),
                    current_model_system: GliomaModelSystem::CellLine,
                    target_model_system: GliomaModelSystem::MouseModel,
                    expected_information_gain_milli: 900,
                    expected_transport_gain_milli: 950,
                    cost_units: 3,
                    risk_milli: 100,
                    reproducibility_milli: 800,
                    requires_approval: true,
                },
            ],
            budget_units: 3,
            max_actions: 1,
            minimum_information_gain_milli: 500,
            minimum_transport_gain_milli: 500,
            maximum_risk_milli: 500,
            allow_approval_required: false,
        }
    }

    #[test]
    fn controller_selects_a_bounded_escalation_and_blocks_approval() {
        let plan = plan_glioma_mechanism_multi_fidelity_control(&request()).unwrap();
        assert_eq!(plan.selected_action_order, vec!["near-escalation"]);
        assert_eq!(plan.blocked_action_order, vec!["far-escalation"]);
        assert_eq!(plan.total_cost_units, 3);
        assert!(plan
            .decisions
            .iter()
            .any(|decision| decision.mechanism_id == "far" && decision.transport_milli < 700));
        plan.validate().unwrap();
    }

    #[test]
    fn controller_reports_no_eligible_escalation_when_transport_gate_is_strict() {
        let mut request = request();
        request.minimum_transport_gain_milli = 1_000;
        let plan = plan_glioma_mechanism_multi_fidelity_control(&request).unwrap();
        assert_eq!(
            plan.disposition,
            MultiFidelityControlDisposition::NoEligibleActions
        );
        assert!(plan.selected_action_order.is_empty());
        plan.validate().unwrap();
    }
}
