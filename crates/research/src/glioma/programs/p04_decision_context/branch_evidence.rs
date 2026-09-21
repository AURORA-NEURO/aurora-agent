//! Branch-evidence assimilation for autonomous preclinical glioma decision contexts.
//!
//! A branch plan is a forecast over possible action portfolios.  This feature closes the loop by
//! attaching explicit local outcomes to each branch, scoring forecast/observation agreement, and
//! preserving contradicted, blocked, and unobserved branches before the next plan is admitted.

use super::branch_planner::DecisionBranchPlan;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F03";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionBranchEvidence1@1";
pub const MAX_OUTCOMES: usize = 2_048;
pub const MAX_ABS_VALUE_MILLI: i64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionBranchEvidenceOutcomeStatus {
    Confirmed,
    Contradicted,
    Inconclusive,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBranchEvidenceOutcome {
    pub branch_id: String,
    pub status: DecisionBranchEvidenceOutcomeStatus,
    pub observed_value_milli: i64,
    pub uncertainty_milli: u32,
    pub result_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBranchEvidenceRequest {
    pub objective: String,
    pub plan: DecisionBranchPlan,
    pub outcomes: Vec<DecisionBranchEvidenceOutcome>,
    pub minimum_confidence_milli: u16,
    pub max_outcomes: usize,
    pub preserve_negative_results: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionBranchEvidenceStatus {
    Confirmed,
    Contradicted,
    Inconclusive,
    Blocked,
    Unobserved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBranchEvidenceRecord {
    pub branch_id: String,
    pub prior_expected_value_milli: i64,
    pub prior_robustness_milli: i64,
    pub observed_value_milli: Option<i64>,
    pub result_digest: Option<ContentHash>,
    pub evidence_score_milli: u16,
    pub outcome_status: Option<DecisionBranchEvidenceOutcomeStatus>,
    pub status: DecisionBranchEvidenceStatus,
    pub negative_evidence_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionBranchEvidenceDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBranchEvidence {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub branch_order: Vec<String>,
    pub records: Vec<DecisionBranchEvidenceRecord>,
    pub frontier_order: Vec<String>,
    pub confirmed_branch_id: Option<String>,
    pub contradicted_branch_order: Vec<String>,
    pub unresolved_branch_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: DecisionBranchEvidenceDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionBranchEvidenceError {
    #[error("decision branch evidence request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision branch evidence outcome is invalid: {0}")]
    InvalidOutcome(String),
    #[error("decision branch evidence output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision branch evidence digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(output: &DecisionBranchEvidence) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "branch_order": output.branch_order,
        "records": output.records,
        "frontier_order": output.frontier_order,
        "confirmed_branch_id": output.confirmed_branch_id,
        "contradicted_branch_order": output.contradicted_branch_order,
        "unresolved_branch_order": output.unresolved_branch_order,
        "omission_order": output.omission_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn agreement_score(expected: i64, observed: i64, uncertainty: u32) -> u16 {
    let scale = u64::from(uncertainty).saturating_add(500);
    let penalty = expected.abs_diff(observed).saturating_mul(1_000) / scale;
    (1_000_u64.saturating_sub(penalty.min(1_000))) as u16
}

impl DecisionBranchEvidence {
    pub fn validate(&self) -> Result<(), DecisionBranchEvidenceError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.branch_order)
            || self.records.len() != self.branch_order.len()
            || self
                .records
                .windows(2)
                .any(|pair| pair[0].branch_id >= pair[1].branch_id)
            || !unique_nonempty(&self.frontier_order)
            || !canonical(&self.contradicted_branch_order)
            || !canonical(&self.unresolved_branch_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.records.iter().any(|record| {
                record.branch_id.trim().is_empty()
                    || record.evidence_score_milli > 1_000
                    || record
                        .result_digest
                        .as_ref()
                        .is_some_and(|digest| digest.as_str().len() != 64)
                    || !canonical(&record.negative_evidence_order)
            })
            || self.digest.as_str().len() != 64
        {
            return Err(DecisionBranchEvidenceError::InvalidOutput(
                "identity, branch partitions, score bounds, canonical ordering, or digest shape is invalid".into(),
            ));
        }
        let ids = self
            .records
            .iter()
            .map(|record| record.branch_id.clone())
            .collect::<BTreeSet<_>>();
        if ids != self.branch_order.iter().cloned().collect::<BTreeSet<_>>()
            || self.frontier_order.iter().any(|id| !ids.contains(id))
            || self
                .confirmed_branch_id
                .as_ref()
                .is_some_and(|id| !ids.contains(id))
            || self
                .contradicted_branch_order
                .iter()
                .any(|id| !ids.contains(id))
            || self
                .unresolved_branch_order
                .iter()
                .any(|id| !ids.contains(id))
        {
            return Err(DecisionBranchEvidenceError::InvalidOutput(
                "branch partitions do not reconcile with records".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionBranchEvidenceError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionBranchEvidenceError::Digest(
                "decision branch evidence digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &DecisionBranchEvidenceRequest,
) -> Result<(), DecisionBranchEvidenceError> {
    if request.objective.trim().is_empty()
        || request.plan.objective != request.objective
        || request.max_outcomes == 0
        || request.max_outcomes > MAX_OUTCOMES
        || request.outcomes.len() > request.max_outcomes
        || request.minimum_confidence_milli > 1_000
    {
        return Err(DecisionBranchEvidenceError::InvalidRequest(
            "objective/plan binding, bounded outcomes, and confidence threshold are required"
                .into(),
        ));
    }
    request
        .plan
        .validate()
        .map_err(|error| DecisionBranchEvidenceError::InvalidRequest(error.to_string()))?;
    let branch_ids = request
        .plan
        .branch_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut outcome_ids = BTreeSet::new();
    for outcome in &request.outcomes {
        if outcome.branch_id.trim().is_empty()
            || !branch_ids.contains(&outcome.branch_id)
            || !outcome_ids.insert(outcome.branch_id.clone())
            || outcome.uncertainty_milli == 0
            || outcome.observed_value_milli.unsigned_abs() > MAX_ABS_VALUE_MILLI as u64
            || outcome.result_digest.as_str().len() != 64
        {
            return Err(DecisionBranchEvidenceError::InvalidOutcome(
                "outcomes require unique known branches, bounded observed values, uncertainty, and digests".into(),
            ));
        }
    }
    Ok(())
}

/// Assimilate observed local outcomes into a planned decision-branch frontier.
pub fn assimilate_glioma_decision_branch_evidence(
    request: &DecisionBranchEvidenceRequest,
) -> Result<DecisionBranchEvidence, DecisionBranchEvidenceError> {
    validate_request(request)?;
    let outcomes = request
        .outcomes
        .iter()
        .map(|outcome| (outcome.branch_id.clone(), outcome))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut records = Vec::new();
    let mut contradicted = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for portfolio in &request.plan.portfolios {
        let outcome = outcomes.get(&portfolio.branch_id).copied();
        let mut status = DecisionBranchEvidenceStatus::Unobserved;
        let mut score = 0_u16;
        let mut observed = None;
        let mut negative_order = Vec::new();
        if let Some(outcome) = outcome {
            observed = Some(outcome.observed_value_milli);
            score = agreement_score(
                portfolio.expected_value_milli,
                outcome.observed_value_milli,
                outcome.uncertainty_milli,
            );
            status = match outcome.status {
                DecisionBranchEvidenceOutcomeStatus::Confirmed
                    if score >= request.minimum_confidence_milli =>
                {
                    DecisionBranchEvidenceStatus::Confirmed
                }
                DecisionBranchEvidenceOutcomeStatus::Confirmed => {
                    uncertainty.insert(format!("{}:low-confirmation-score", portfolio.branch_id));
                    DecisionBranchEvidenceStatus::Inconclusive
                }
                DecisionBranchEvidenceOutcomeStatus::Contradicted
                | DecisionBranchEvidenceOutcomeStatus::Failed => {
                    let evidence = format!("{}:negative-result", portfolio.branch_id);
                    negative.insert(evidence.clone());
                    negative_order.push(evidence);
                    contradicted.insert(portfolio.branch_id.clone());
                    DecisionBranchEvidenceStatus::Contradicted
                }
                DecisionBranchEvidenceOutcomeStatus::Inconclusive => {
                    uncertainty.insert(format!("{}:inconclusive", portfolio.branch_id));
                    DecisionBranchEvidenceStatus::Inconclusive
                }
                DecisionBranchEvidenceOutcomeStatus::Blocked => {
                    uncertainty.insert(format!("{}:blocked", portfolio.branch_id));
                    DecisionBranchEvidenceStatus::Blocked
                }
            };
            if matches!(status, DecisionBranchEvidenceStatus::Inconclusive) {
                unresolved.insert(portfolio.branch_id.clone());
            }
            if matches!(status, DecisionBranchEvidenceStatus::Blocked) {
                unresolved.insert(portfolio.branch_id.clone());
            }
        } else {
            omission.insert(portfolio.branch_id.clone());
            uncertainty.insert(format!("{}:unobserved", portfolio.branch_id));
        }
        records.push(DecisionBranchEvidenceRecord {
            branch_id: portfolio.branch_id.clone(),
            prior_expected_value_milli: portfolio.expected_value_milli,
            prior_robustness_milli: portfolio.robustness_milli,
            observed_value_milli: observed,
            result_digest: outcome.map(|outcome| outcome.result_digest.clone()),
            evidence_score_milli: score,
            outcome_status: outcome.map(|outcome| outcome.status),
            status,
            negative_evidence_order: negative_order,
        });
    }
    records.sort_by(|left, right| left.branch_id.cmp(&right.branch_id));
    let mut frontier = records.clone();
    frontier.sort_by(|left, right| {
        let left_rank = match left.status {
            DecisionBranchEvidenceStatus::Blocked => 0,
            DecisionBranchEvidenceStatus::Contradicted => 1,
            DecisionBranchEvidenceStatus::Inconclusive => 2,
            DecisionBranchEvidenceStatus::Unobserved => 3,
            DecisionBranchEvidenceStatus::Confirmed => 4,
        };
        let right_rank = match right.status {
            DecisionBranchEvidenceStatus::Blocked => 0,
            DecisionBranchEvidenceStatus::Contradicted => 1,
            DecisionBranchEvidenceStatus::Inconclusive => 2,
            DecisionBranchEvidenceStatus::Unobserved => 3,
            DecisionBranchEvidenceStatus::Confirmed => 4,
        };
        left_rank
            .cmp(&right_rank)
            .then_with(|| left.evidence_score_milli.cmp(&right.evidence_score_milli))
            .then_with(|| left.branch_id.cmp(&right.branch_id))
    });
    let frontier_order = frontier
        .into_iter()
        .map(|record| record.branch_id)
        .collect::<Vec<_>>();
    let confirmed_branch_id = request
        .plan
        .selected_branch_id
        .as_ref()
        .and_then(|selected| {
            records
                .iter()
                .find(|record| &record.branch_id == selected)
                .filter(|record| record.status == DecisionBranchEvidenceStatus::Confirmed)
                .map(|record| record.branch_id.clone())
        });
    if confirmed_branch_id.is_none() && request.plan.selected_branch_id.is_some() {
        uncertainty.insert("selected-branch-not-confirmed".into());
    }
    let has_blocked = records
        .iter()
        .any(|record| record.status == DecisionBranchEvidenceStatus::Blocked);
    let has_unresolved = records.iter().any(|record| {
        matches!(
            record.status,
            DecisionBranchEvidenceStatus::Inconclusive | DecisionBranchEvidenceStatus::Unobserved
        )
    });
    let disposition = if has_blocked {
        DecisionBranchEvidenceDisposition::Blocked
    } else if has_unresolved || !contradicted.is_empty() || confirmed_branch_id.is_none() {
        DecisionBranchEvidenceDisposition::Partial
    } else {
        DecisionBranchEvidenceDisposition::Ready
    };
    let mut output = DecisionBranchEvidence {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        branch_order: request.plan.branch_order.clone(),
        records,
        frontier_order,
        confirmed_branch_id,
        contradicted_branch_order: contradicted.into_iter().collect(),
        unresolved_branch_order: unresolved.into_iter().collect(),
        omission_order: omission.into_iter().collect(),
        negative_evidence: if request.preserve_negative_results {
            negative.into_iter().collect()
        } else {
            Vec::new()
        },
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DecisionBranchEvidenceError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::branch_planner::{
        plan_glioma_decision_branches, DecisionBranchPlannerRequest, DecisionScenario,
        DecisionScenarioOutcome,
    };
    use super::super::context_compiler::{compile_decision_context, DecisionContextRequest};
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma_engine::{
        GliomaActionCandidate, GliomaModality, GliomaModelSystem, GliomaStageKind, LocalArtifactRef,
    };
    use bioprism_foundation::{AutonomyTier, Effect};
    use std::collections::BTreeSet;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn plan() -> DecisionBranchPlan {
        let record = EvidenceRecord {
            evidence_id: "e1".into(),
            source_artifact: LocalArtifactRef {
                artifact_id: "a1".into(),
                content_hash: hash("a1"),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: "EGFR signaling increases invasion".into(),
            scope: "preclinical glioma".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        };
        let knowledge = compile_typed_knowledge(
            &KnowledgeRequest {
                objective: "branch evidence".into(),
                required_modalities: BTreeSet::from([GliomaModality::Genomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &[record],
        )
        .unwrap();
        let context = compile_decision_context(
            &DecisionContextRequest {
                objective: "branch evidence".into(),
                max_actions: 8,
                default_cost_units: 5,
            },
            &knowledge,
        )
        .unwrap();
        let candidate = GliomaActionCandidate {
            action_id: "branch-action".into(),
            stage_kind: GliomaStageKind::MechanismExploration,
            modality: GliomaModality::Genomics,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            cost_units: 5,
            information_gain_milli: 900,
            frontier_novelty_milli: 800,
            workflow_leverage_milli: 800,
            cross_stage_unlock_milli: 800,
            reproducibility_safety_milli: 900,
            federation_value_milli: 500,
            feasibility_milli: 800,
            autonomy_tier: AutonomyTier::A1,
            effects: BTreeSet::from([Effect::ReadLocalData, Effect::ExecuteLocalComputation]),
        };
        plan_glioma_decision_branches(
            &DecisionBranchPlannerRequest {
                objective: "branch evidence".into(),
                candidates: vec![candidate],
                completed_action_order: Vec::new(),
                scenarios: vec![
                    DecisionScenario {
                        scenario_id: "high".into(),
                        probability_milli: 600,
                        outcomes: vec![DecisionScenarioOutcome {
                            action_id: "branch-action".into(),
                            value_milli: 800,
                            uncertainty_milli: 100,
                            failure_probability_milli: 50,
                        }],
                    },
                    DecisionScenario {
                        scenario_id: "low".into(),
                        probability_milli: 400,
                        outcomes: vec![DecisionScenarioOutcome {
                            action_id: "branch-action".into(),
                            value_milli: 200,
                            uncertainty_milli: 300,
                            failure_probability_milli: 100,
                        }],
                    },
                ],
                budget_units: 10,
                max_actions_per_branch: 2,
                max_branches: 4,
                beam_width: 8,
                minimum_robustness_milli: -1_000_000,
                uncertainty_penalty_milli: 100,
                failure_penalty_milli: 100,
                selection_weights: Default::default(),
            },
            &context,
        )
        .unwrap()
    }

    #[test]
    fn branch_evidence_preserves_contradiction_and_blocks_selection() {
        let plan = plan();
        let branch_id = plan.branch_order[0].clone();
        let output = assimilate_glioma_decision_branch_evidence(&DecisionBranchEvidenceRequest {
            objective: "branch evidence".into(),
            plan,
            outcomes: vec![DecisionBranchEvidenceOutcome {
                branch_id,
                status: DecisionBranchEvidenceOutcomeStatus::Contradicted,
                observed_value_milli: -200,
                uncertainty_milli: 100,
                result_digest: hash("negative"),
            }],
            minimum_confidence_milli: 700,
            max_outcomes: 8,
            preserve_negative_results: true,
        })
        .unwrap();
        assert_eq!(
            output.disposition,
            DecisionBranchEvidenceDisposition::Partial
        );
        assert!(!output.contradicted_branch_order.is_empty());
        assert!(!output.omission_order.is_empty() || output.records.len() == 1);
        output.validate().unwrap();
    }

    #[test]
    fn branch_evidence_rejects_unknown_branch() {
        let error = assimilate_glioma_decision_branch_evidence(&DecisionBranchEvidenceRequest {
            objective: "branch evidence".into(),
            plan: plan(),
            outcomes: vec![DecisionBranchEvidenceOutcome {
                branch_id: "unknown".into(),
                status: DecisionBranchEvidenceOutcomeStatus::Confirmed,
                observed_value_milli: 0,
                uncertainty_milli: 100,
                result_digest: hash("unknown"),
            }],
            minimum_confidence_milli: 700,
            max_outcomes: 8,
            preserve_negative_results: true,
        })
        .unwrap_err();
        assert!(error.to_string().contains("known branches"));
    }
}
