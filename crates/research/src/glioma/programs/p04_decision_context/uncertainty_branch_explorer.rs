//! Uncertainty-aware comparison of competing preclinical glioma research branches.
//!
//! The branch planner produces executable portfolios, but a planner score alone is not a
//! researcher-facing decision surface. This feature joins the immutable decision context to the
//! forecast portfolio and any observed branch evidence, then exposes coverage, disagreement,
//! information gain, cost, and blocked alternatives in one deterministic comparison. Forecasts
//! remain visibly separate from observations: an unobserved branch can be ranked, but it cannot
//! be promoted to a confirmed scientific direction.

use super::branch_evidence::{DecisionBranchEvidence, DecisionBranchEvidenceStatus};
use super::branch_planner::{DecisionBranchPlan, DecisionBranchPortfolio};
use super::context_compiler::DecisionContext;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F17";
pub const OUTPUT_SCHEMA: &str = "GliomaUncertaintyBranchExplorer1@1";
pub const MAX_BRANCHES: usize = 64;
pub const MAX_ANNOTATIONS: usize = 128;
pub const MAX_NOTE_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBranchAnnotation {
    pub branch_id: String,
    pub author_role: String,
    pub note: String,
    pub context_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UncertaintyBranchExplorerRequest {
    pub objective: String,
    pub context: DecisionContext,
    pub plan: DecisionBranchPlan,
    pub evidence: Option<DecisionBranchEvidence>,
    pub annotations: Vec<DecisionBranchAnnotation>,
    pub max_branches: usize,
    pub expected_value_weight_milli: u16,
    pub information_gain_weight_milli: u16,
    pub coverage_weight_milli: u16,
    pub cost_penalty_weight_milli: u16,
    pub disagreement_penalty_weight_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionBranchExplorerStatus {
    AdmissibleForecast,
    EvidenceLimited,
    Confirmed,
    Contradicted,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBranchComparison {
    pub branch_id: String,
    pub selected_action_order: Vec<String>,
    pub covered_claim_order: Vec<String>,
    pub uncovered_claim_order: Vec<String>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub modality_order: Vec<GliomaModality>,
    pub evidence_coverage_milli: u16,
    pub model_disagreement_milli: u16,
    pub expected_information_gain_milli: u16,
    pub expected_value_milli: i64,
    pub worst_case_value_milli: i64,
    pub failure_risk_milli: u32,
    pub cost_units: u32,
    pub ranking_score_milli: i64,
    pub blocked_reason_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub status: DecisionBranchExplorerStatus,
    pub annotation: Option<DecisionBranchAnnotation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UncertaintyBranchExplorer {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub context_digest: ContentHash,
    pub branch_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub comparisons: Vec<DecisionBranchComparison>,
    pub omitted_branch_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: DecisionBranchExplorerDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionBranchExplorerDisposition {
    Ready,
    EvidenceLimited,
    Blocked,
    Unresolved,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionBranchExplorerError {
    #[error("uncertainty branch explorer request is invalid: {0}")]
    InvalidRequest(String),
    #[error("uncertainty branch explorer output is invalid: {0}")]
    InvalidOutput(String),
    #[error("uncertainty branch explorer digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_NOTE_BYTES
}

fn digest_input(output: &UncertaintyBranchExplorer) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "context_digest": output.context_digest,
        "branch_order": output.branch_order,
        "ranked_order": output.ranked_order,
        "comparisons": output.comparisons,
        "omitted_branch_order": output.omitted_branch_order,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
    })
}

impl UncertaintyBranchExplorer {
    pub fn validate(&self) -> Result<(), DecisionBranchExplorerError> {
        let ids = self
            .comparisons
            .iter()
            .map(|comparison| comparison.branch_id.clone())
            .collect::<BTreeSet<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || self.context_digest.as_str().len() != 64
            || !canonical(&self.branch_order)
            || !canonical(&self.omitted_branch_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.comparisons.len() != self.branch_order.len()
            || self
                .comparisons
                .windows(2)
                .any(|pair| pair[0].branch_id >= pair[1].branch_id)
            || self.comparisons.iter().any(|comparison| {
                comparison.branch_id.trim().is_empty()
                    || !canonical(&comparison.selected_action_order)
                    || !canonical(&comparison.covered_claim_order)
                    || !canonical(&comparison.uncovered_claim_order)
                    || !canonical(&comparison.model_system_order)
                    || !canonical(&comparison.modality_order)
                    || !canonical(&comparison.blocked_reason_order)
                    || !canonical(&comparison.negative_evidence_order)
                    || comparison.evidence_coverage_milli > 1_000
                    || comparison.model_disagreement_milli > 1_000
                    || comparison.expected_information_gain_milli > 1_000
                    || comparison.failure_risk_milli > 1_000
                    || comparison.annotation.as_ref().is_some_and(|annotation| {
                        annotation.branch_id != comparison.branch_id
                            || !safe_text(&annotation.author_role)
                            || !safe_text(&annotation.note)
                            || annotation.context_digest.as_str().len() != 64
                    })
            })
        {
            return Err(DecisionBranchExplorerError::InvalidOutput(
                "identity, branch ordering, score bounds, or comparison invariants are invalid"
                    .into(),
            ));
        }
        if ids != self.branch_order.iter().cloned().collect::<BTreeSet<_>>()
            || self.ranked_order.iter().any(|branch| !ids.contains(branch))
            || self.ranked_order.windows(2).any(|pair| pair[0] == pair[1])
            || self
                .omitted_branch_order
                .iter()
                .any(|branch| ids.contains(branch))
        {
            return Err(DecisionBranchExplorerError::InvalidOutput(
                "branch partitions do not reconcile with comparisons".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionBranchExplorerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionBranchExplorerError::InvalidOutput(
                "explorer digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &UncertaintyBranchExplorerRequest,
) -> Result<(), DecisionBranchExplorerError> {
    if !safe_text(&request.objective)
        || request.objective.trim() != request.context.objective.trim()
        || request.objective.trim() != request.plan.objective.trim()
        || request.context.digest != request.plan.context_digest
        || request.max_branches == 0
        || request.max_branches > MAX_BRANCHES
        || request
            .expected_value_weight_milli
            .saturating_add(request.information_gain_weight_milli)
            .saturating_add(request.coverage_weight_milli)
            .saturating_add(request.cost_penalty_weight_milli)
            .saturating_add(request.disagreement_penalty_weight_milli)
            != 1_000
    {
        return Err(DecisionBranchExplorerError::InvalidRequest(
            "objective/context/plan binding, bounded branch count, and weights summing to 1000 are required".into(),
        ));
    }
    request
        .context
        .validate()
        .map_err(|error| DecisionBranchExplorerError::InvalidRequest(error.to_string()))?;
    request
        .plan
        .validate()
        .map_err(|error| DecisionBranchExplorerError::InvalidRequest(error.to_string()))?;
    if request.plan.portfolios.len() > MAX_BRANCHES {
        return Err(DecisionBranchExplorerError::InvalidRequest(
            "branch plan exceeds explorer bound".into(),
        ));
    }
    if let Some(evidence) = &request.evidence {
        evidence
            .validate()
            .map_err(|error| DecisionBranchExplorerError::InvalidRequest(error.to_string()))?;
        if evidence.objective.trim() != request.objective.trim()
            || evidence.branch_order != request.plan.branch_order
        {
            return Err(DecisionBranchExplorerError::InvalidRequest(
                "branch evidence must bind to the same objective and plan branch order".into(),
            ));
        }
    }
    let mut annotation_ids = BTreeSet::new();
    if request.annotations.len() > MAX_ANNOTATIONS
        || request.annotations.iter().any(|annotation| {
            annotation.branch_id.trim().is_empty()
                || !safe_text(&annotation.author_role)
                || !safe_text(&annotation.note)
                || annotation.context_digest != request.context.digest
                || !request.plan.branch_order.contains(&annotation.branch_id)
                || !annotation_ids.insert(annotation.branch_id.clone())
        })
    {
        return Err(DecisionBranchExplorerError::InvalidRequest(
            "annotations must be bounded, unique per known branch, and context-bound".into(),
        ));
    }
    Ok(())
}

fn branch_disagreement(portfolio: &DecisionBranchPortfolio) -> u16 {
    let (minimum, maximum) = portfolio
        .scenario_scores
        .iter()
        .map(|score| score.value_milli)
        .fold((i64::MAX, i64::MIN), |(minimum, maximum), value| {
            (minimum.min(value), maximum.max(value))
        });
    maximum.saturating_sub(minimum).unsigned_abs().min(1_000) as u16
}

fn branch_information_gain(
    selected_action_order: &[String],
    action_map: &BTreeMap<String, &super::context_compiler::DecisionAction>,
) -> u16 {
    selected_action_order
        .iter()
        .filter_map(|action_id| action_map.get(action_id))
        .map(|action| u32::from(action.candidate.information_gain_milli))
        .sum::<u32>()
        .min(1_000) as u16
}

fn branch_coverage(covered: usize, total: usize) -> u16 {
    if total == 0 {
        return 0;
    }
    ((covered as u64).saturating_mul(1_000) / total as u64).min(1_000) as u16
}

fn ranking_score(
    portfolio: &DecisionBranchPortfolio,
    coverage: u16,
    information_gain: u16,
    disagreement: u16,
    request: &UncertaintyBranchExplorerRequest,
) -> i64 {
    let positive = i128::from(portfolio.expected_value_milli)
        .saturating_mul(i128::from(request.expected_value_weight_milli))
        .saturating_add(
            i128::from(information_gain)
                .saturating_mul(i128::from(request.information_gain_weight_milli)),
        )
        .saturating_add(
            i128::from(coverage).saturating_mul(i128::from(request.coverage_weight_milli)),
        );
    let penalties = i128::from(portfolio.cost_units)
        .saturating_mul(i128::from(request.cost_penalty_weight_milli))
        .saturating_add(
            i128::from(disagreement)
                .saturating_mul(i128::from(request.disagreement_penalty_weight_milli)),
        );
    positive
        .saturating_sub(penalties)
        .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

/// Build a researcher-facing comparison of competing decision branches.
pub fn explore_glioma_uncertain_branches(
    request: &UncertaintyBranchExplorerRequest,
) -> Result<UncertaintyBranchExplorer, DecisionBranchExplorerError> {
    validate_request(request)?;
    let action_map = request
        .context
        .actions
        .iter()
        .map(|action| (action.action_id.clone(), action))
        .collect::<BTreeMap<_, _>>();
    let evidence_map = request.evidence.as_ref().map(|evidence| {
        evidence
            .records
            .iter()
            .map(|record| (record.branch_id.clone(), record))
            .collect::<BTreeMap<_, _>>()
    });
    let annotation_map = request
        .annotations
        .iter()
        .map(|annotation| (annotation.branch_id.clone(), annotation.clone()))
        .collect::<BTreeMap<_, _>>();
    let context_claims = request
        .context
        .claim_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut comparisons = Vec::new();
    let mut omitted = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for portfolio in &request.plan.portfolios {
        let selected = portfolio.selected_order.clone();
        let mut covered_claims = BTreeSet::new();
        let mut models = BTreeSet::new();
        let mut modalities = BTreeSet::new();
        for action_id in &selected {
            if let Some(action) = action_map.get(action_id) {
                covered_claims.insert(action.claim_id.clone());
                models.insert(action.target_model_system);
                modalities.insert(action.target_modality);
            } else {
                uncertainty.insert(format!("{}:action-not-in-context", portfolio.branch_id));
            }
        }
        let covered_claim_order = covered_claims.iter().cloned().collect::<Vec<_>>();
        let uncovered_claim_order = context_claims
            .difference(&covered_claims)
            .cloned()
            .collect::<Vec<_>>();
        let coverage = branch_coverage(covered_claims.len(), context_claims.len());
        let disagreement = branch_disagreement(portfolio);
        let information_gain = branch_information_gain(&selected, &action_map);
        let evidence_record = evidence_map
            .as_ref()
            .and_then(|records| records.get(&portfolio.branch_id));
        let mut status = match evidence_record.map(|record| record.status) {
            Some(DecisionBranchEvidenceStatus::Confirmed) => {
                DecisionBranchExplorerStatus::Confirmed
            }
            Some(DecisionBranchEvidenceStatus::Contradicted) => {
                DecisionBranchExplorerStatus::Contradicted
            }
            Some(DecisionBranchEvidenceStatus::Blocked) => DecisionBranchExplorerStatus::Blocked,
            Some(
                DecisionBranchEvidenceStatus::Inconclusive
                | DecisionBranchEvidenceStatus::Unobserved,
            ) => DecisionBranchExplorerStatus::EvidenceLimited,
            None => DecisionBranchExplorerStatus::AdmissibleForecast,
        };
        let mut blocked_reason_order = Vec::new();
        let mut negative_evidence_order = Vec::new();
        if request
            .plan
            .budget_block_order
            .iter()
            .any(|action| selected.binary_search(action).is_ok())
        {
            blocked_reason_order.push("budget-blocked-action".into());
            status = DecisionBranchExplorerStatus::Blocked;
        }
        if request
            .plan
            .unresolved_scenario_order
            .iter()
            .any(|scenario| {
                portfolio
                    .scenario_scores
                    .iter()
                    .any(|score| &score.scenario_id == scenario)
            })
        {
            blocked_reason_order.push("unresolved-scenario".into());
            if matches!(status, DecisionBranchExplorerStatus::AdmissibleForecast) {
                status = DecisionBranchExplorerStatus::Unresolved;
            }
        }
        if let Some(record) = evidence_record {
            if matches!(record.status, DecisionBranchEvidenceStatus::Contradicted) {
                let reason = format!("{}:contradicted", portfolio.branch_id);
                negative.insert(reason.clone());
                negative_evidence_order.push(reason);
            }
            if matches!(record.status, DecisionBranchEvidenceStatus::Unobserved) {
                uncertainty.insert(format!("{}:unobserved", portfolio.branch_id));
            }
        } else {
            uncertainty.insert(format!("{}:forecast-only", portfolio.branch_id));
        }
        if !uncovered_claim_order.is_empty() {
            blocked_reason_order.push("uncovered-claims".into());
        }
        blocked_reason_order.sort();
        blocked_reason_order.dedup();
        let ranking = ranking_score(portfolio, coverage, information_gain, disagreement, request);
        comparisons.push(DecisionBranchComparison {
            branch_id: portfolio.branch_id.clone(),
            selected_action_order: selected,
            covered_claim_order,
            uncovered_claim_order,
            model_system_order: models.into_iter().collect(),
            modality_order: modalities.into_iter().collect(),
            evidence_coverage_milli: coverage,
            model_disagreement_milli: disagreement,
            expected_information_gain_milli: information_gain,
            expected_value_milli: portfolio.expected_value_milli,
            worst_case_value_milli: portfolio.worst_case_value_milli,
            failure_risk_milli: portfolio.failure_risk_milli,
            cost_units: portfolio.cost_units,
            ranking_score_milli: ranking,
            blocked_reason_order,
            negative_evidence_order,
            status,
            annotation: annotation_map.get(&portfolio.branch_id).cloned(),
        });
    }
    comparisons.sort_by(|left, right| left.branch_id.cmp(&right.branch_id));
    let all_branch_ids = comparisons
        .iter()
        .map(|comparison| comparison.branch_id.clone())
        .collect::<BTreeSet<_>>();
    let mut ranked = comparisons.clone();
    ranked.sort_by(|left, right| {
        right
            .ranking_score_milli
            .cmp(&left.ranking_score_milli)
            .then_with(|| left.branch_id.cmp(&right.branch_id))
    });
    let ranked_order = ranked
        .iter()
        .take(request.max_branches)
        .map(|comparison| comparison.branch_id.clone())
        .collect::<Vec<_>>();
    for branch in all_branch_ids.iter() {
        if !ranked_order.iter().any(|selected| selected == branch) {
            omitted.insert(branch.clone());
        }
    }
    if comparisons.iter().any(|comparison| {
        matches!(
            comparison.status,
            DecisionBranchExplorerStatus::EvidenceLimited
                | DecisionBranchExplorerStatus::Unresolved
        )
    }) {
        uncertainty.insert("one-or-more-branches-remain-evidence-limited".into());
    }
    let all_blocked_or_empty = comparisons.is_empty()
        || comparisons.iter().all(|comparison| {
            matches!(
                comparison.status,
                DecisionBranchExplorerStatus::Blocked | DecisionBranchExplorerStatus::Contradicted
            )
        });
    let disposition = if all_blocked_or_empty {
        DecisionBranchExplorerDisposition::Blocked
    } else if evidence_map.is_none()
        || comparisons.iter().any(|comparison| {
            matches!(
                comparison.status,
                DecisionBranchExplorerStatus::EvidenceLimited
                    | DecisionBranchExplorerStatus::Unresolved
            )
        })
    {
        DecisionBranchExplorerDisposition::EvidenceLimited
    } else {
        DecisionBranchExplorerDisposition::Ready
    };
    let mut output = UncertaintyBranchExplorer {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        context_digest: request.context.digest.clone(),
        branch_order: comparisons
            .iter()
            .map(|comparison| comparison.branch_id.clone())
            .collect(),
        ranked_order,
        comparisons,
        omitted_branch_order: omitted.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-uncertainty-branch-explorer"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DecisionBranchExplorerError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p02_evidence_knowledge::{
        KnowledgeClaim, KnowledgeClaimDisposition, TypedKnowledge,
    };
    use crate::glioma::programs::p04_decision_context::branch_planner::{
        plan_glioma_decision_branches, DecisionBranchPlannerRequest, DecisionScenario,
        DecisionScenarioOutcome,
    };
    use crate::glioma::programs::p04_decision_context::context_compiler::{
        compile_decision_context, DecisionContextRequest,
    };
    use crate::glioma_engine::{GliomaActionCandidate, GliomaSelectionWeights, GliomaStageKind};
    use bioprism_foundation::{AutonomyTier, Effect};

    fn knowledge() -> TypedKnowledge {
        let claim = KnowledgeClaim {
            claim_id: "claim-a".into(),
            statement: "state transition remains unresolved".into(),
            scope: "preclinical glioma organoid".into(),
            disposition: KnowledgeClaimDisposition::Unresolved,
            support_milli: 0,
            contradiction_milli: 0,
            confidence_milli: 500,
            supporting_evidence_order: Vec::new(),
            negative_evidence_order: Vec::new(),
            contradictory_evidence_order: Vec::new(),
            unresolved_evidence_order: vec!["gap-a".into()],
            modality_order: vec![crate::glioma_engine::GliomaModality::Imaging],
            model_system_order: vec![GliomaModelSystem::Organoid],
            missing_modality_order: vec![crate::glioma_engine::GliomaModality::Spatial],
            missing_model_system_order: Vec::new(),
        };
        let mut output = TypedKnowledge {
            feature_id: "GAF-GLIOMA-P02-F01".into(),
            output_schema: "GliomaTypedKnowledge1@1".into(),
            objective: "compare invasion branches".into(),
            claim_order: vec![claim.claim_id.clone()],
            claims: vec![claim],
            top_claim_order: Vec::new(),
            omission_order: Vec::new(),
            negative_evidence_order: Vec::new(),
            uncertainty_order: vec!["claim-a:unresolved".into()],
            disposition:
                crate::glioma::programs::p02_evidence_knowledge::KnowledgeDisposition::Unresolved,
            digest: ContentHash::of_bytes(b"placeholder"),
        };
        output.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": output.feature_id,
            "output_schema": output.output_schema,
            "objective": output.objective,
            "claim_order": output.claim_order,
            "claims": output.claims,
            "top_claim_order": output.top_claim_order,
            "omission_order": output.omission_order,
            "negative_evidence_order": output.negative_evidence_order,
            "uncertainty_order": output.uncertainty_order,
            "disposition": output.disposition,
        }))
        .expect("knowledge digest");
        output
    }

    fn context_and_plan() -> (DecisionContext, DecisionBranchPlan) {
        let knowledge = knowledge();
        let context = compile_decision_context(
            &DecisionContextRequest {
                objective: knowledge.objective.clone(),
                max_actions: 8,
                default_cost_units: 10,
            },
            &knowledge,
        )
        .expect("context");
        let action = context.actions[0].candidate.clone();
        let second = GliomaActionCandidate {
            action_id: "branch-second".into(),
            stage_kind: GliomaStageKind::ExperimentDesign,
            modality: crate::glioma_engine::GliomaModality::Spatial,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            cost_units: 10,
            information_gain_milli: 800,
            frontier_novelty_milli: 700,
            workflow_leverage_milli: 800,
            cross_stage_unlock_milli: 700,
            reproducibility_safety_milli: 900,
            federation_value_milli: 400,
            feasibility_milli: 800,
            autonomy_tier: AutonomyTier::A1,
            effects: BTreeSet::from([
                Effect::ReadLocalData,
                Effect::ExecuteLocalComputation,
                Effect::WriteLocalArtifact,
            ]),
        };
        let mut candidates = vec![action, second];
        candidates.sort_by(|left, right| left.action_id.cmp(&right.action_id));
        let scenarios = vec![
            DecisionScenario {
                scenario_id: "baseline".into(),
                probability_milli: 500,
                outcomes: candidates
                    .iter()
                    .map(|candidate| DecisionScenarioOutcome {
                        action_id: candidate.action_id.clone(),
                        value_milli: if candidate.action_id == "branch-second" {
                            700
                        } else {
                            400
                        },
                        uncertainty_milli: 100,
                        failure_probability_milli: 50,
                    })
                    .collect(),
            },
            DecisionScenario {
                scenario_id: "stress".into(),
                probability_milli: 500,
                outcomes: candidates
                    .iter()
                    .map(|candidate| DecisionScenarioOutcome {
                        action_id: candidate.action_id.clone(),
                        value_milli: if candidate.action_id == "branch-second" {
                            100
                        } else {
                            350
                        },
                        uncertainty_milli: 200,
                        failure_probability_milli: 150,
                    })
                    .collect(),
            },
        ];
        let plan = plan_glioma_decision_branches(
            &DecisionBranchPlannerRequest {
                objective: context.objective.clone(),
                candidates,
                completed_action_order: Vec::new(),
                scenarios,
                budget_units: 100,
                max_actions_per_branch: 1,
                max_branches: 4,
                beam_width: 8,
                minimum_robustness_milli: -10_000,
                uncertainty_penalty_milli: 100,
                failure_penalty_milli: 100,
                selection_weights: GliomaSelectionWeights {
                    information_gain: 20,
                    frontier_novelty: 15,
                    workflow_leverage: 15,
                    cross_stage_unlock: 15,
                    reproducibility_safety: 15,
                    federation_value: 10,
                    feasibility: 10,
                },
            },
            &context,
        )
        .expect("plan");
        (context, plan)
    }

    fn request() -> UncertaintyBranchExplorerRequest {
        let (context, plan) = context_and_plan();
        UncertaintyBranchExplorerRequest {
            objective: context.objective.clone(),
            context,
            plan,
            evidence: None,
            annotations: Vec::new(),
            max_branches: 4,
            expected_value_weight_milli: 400,
            information_gain_weight_milli: 250,
            coverage_weight_milli: 250,
            cost_penalty_weight_milli: 50,
            disagreement_penalty_weight_milli: 50,
        }
    }

    #[test]
    fn ranks_forecasts_but_keeps_unobserved_state_explicit() {
        let output = explore_glioma_uncertain_branches(&request()).expect("explorer");
        assert!(!output.ranked_order.is_empty());
        assert_eq!(
            output.disposition,
            DecisionBranchExplorerDisposition::EvidenceLimited
        );
        assert!(output
            .uncertainty_order
            .iter()
            .any(|value| value.ends_with(":forecast-only")));
        output.validate().expect("valid output");
    }

    #[test]
    fn contradictory_evidence_surfaces_negative_branch() {
        let mut request = request();
        let branch_id = request.plan.branch_order[0].clone();
        let mut evidence = DecisionBranchEvidence {
            feature_id: "GAF-GLIOMA-P04-F03".into(),
            output_schema: "GliomaDecisionBranchEvidence1@1".into(),
            objective: request.objective.clone(),
            branch_order: request.plan.branch_order.clone(),
            records: request
                .plan
                .portfolios
                .iter()
                .map(|portfolio| crate::glioma::programs::p04_decision_context::branch_evidence::DecisionBranchEvidenceRecord {
                    branch_id: portfolio.branch_id.clone(),
                    prior_expected_value_milli: portfolio.expected_value_milli,
                    prior_robustness_milli: portfolio.robustness_milli,
                    observed_value_milli: if portfolio.branch_id == branch_id { Some(-800) } else { None },
                    result_digest: if portfolio.branch_id == branch_id { Some(ContentHash::of_bytes(b"negative")) } else { None },
                    evidence_score_milli: if portfolio.branch_id == branch_id { 900 } else { 0 },
                    outcome_status: None,
                    status: if portfolio.branch_id == branch_id { DecisionBranchEvidenceStatus::Contradicted } else { DecisionBranchEvidenceStatus::Unobserved },
                    negative_evidence_order: if portfolio.branch_id == branch_id { vec![format!("{}:negative-result", branch_id)] } else { Vec::new() },
                })
                .collect(),
            frontier_order: request.plan.frontier_order.clone(),
            confirmed_branch_id: None,
            contradicted_branch_order: vec![branch_id.clone()],
            unresolved_branch_order: request
                .plan
                .branch_order
                .iter()
                .filter(|id| **id != branch_id)
                .cloned()
                .collect(),
            omission_order: Vec::new(),
            negative_evidence: vec![format!("{}:negative-result", branch_id)],
            uncertainty: vec!["unobserved".into()],
            disposition: crate::glioma::programs::p04_decision_context::branch_evidence::DecisionBranchEvidenceDisposition::Partial,
            digest: ContentHash::of_bytes(b"placeholder"),
        };
        evidence.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": evidence.feature_id,
            "output_schema": evidence.output_schema,
            "objective": evidence.objective,
            "branch_order": evidence.branch_order,
            "records": evidence.records,
            "frontier_order": evidence.frontier_order,
            "confirmed_branch_id": evidence.confirmed_branch_id,
            "contradicted_branch_order": evidence.contradicted_branch_order,
            "unresolved_branch_order": evidence.unresolved_branch_order,
            "omission_order": evidence.omission_order,
            "negative_evidence": evidence.negative_evidence,
            "uncertainty": evidence.uncertainty,
            "disposition": evidence.disposition,
        }))
        .expect("evidence digest");
        request.evidence = Some(evidence);
        let output = explore_glioma_uncertain_branches(&request).expect("explorer");
        let comparison = output
            .comparisons
            .iter()
            .find(|comparison| comparison.branch_id == branch_id)
            .expect("branch comparison");
        assert_eq!(
            comparison.status,
            DecisionBranchExplorerStatus::Contradicted
        );
        assert!(!comparison.negative_evidence_order.is_empty());
    }

    #[test]
    fn annotations_are_context_bound_and_replay_stable() {
        let mut first = request();
        let branch_id = first.plan.branch_order[0].clone();
        first.annotations.push(DecisionBranchAnnotation {
            branch_id: branch_id.clone(),
            author_role: "methods-reviewer".into(),
            note: "requires spatial validation before promotion".into(),
            context_digest: first.context.digest.clone(),
        });
        let second = first.clone();
        let left = explore_glioma_uncertain_branches(&first).expect("first");
        let right = explore_glioma_uncertain_branches(&second).expect("second");
        assert_eq!(left.digest, right.digest);
        assert_eq!(
            left.comparisons
                .iter()
                .find(|comparison| comparison.branch_id == branch_id)
                .and_then(|comparison| comparison.annotation.as_ref())
                .map(|annotation| annotation.note.as_str()),
            Some("requires spatial validation before promotion")
        );
    }

    #[test]
    fn mismatched_context_or_weight_budget_is_rejected() {
        let mut request = request();
        request.coverage_weight_milli = 249;
        assert!(matches!(
            explore_glioma_uncertain_branches(&request),
            Err(DecisionBranchExplorerError::InvalidRequest(_))
        ));
    }
}
