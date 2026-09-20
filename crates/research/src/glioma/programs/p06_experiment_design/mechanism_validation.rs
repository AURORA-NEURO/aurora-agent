//! Mechanism-to-validation compiler for preclinical glioma research.
//!
//! A robust counterfactual portfolio is still only a mechanistic prioritisation.  This feature
//! turns its selected perturbations into a power-aware validation queue and refuses to plan an
//! arm until the selected candidate, control, local artifact, and observation bindings are all
//! present.  It is the scientific bridge between P05 model uncertainty and P06 sequential assay
//! design: model-ensemble lower-tail gates choose what may be validated, while interim power and
//! futility boundaries decide whether the next local batch should continue, stop, or preserve a
//! negative result.  No assay, instrument, federation, or clinical action is dispatched here.

use super::power_reestimation::{
    plan_glioma_power_reestimation, PowerArmObservation, PowerDecisionKind, PowerReestimationError,
    PowerReestimationPlan, PowerReestimationRequest,
};
use crate::glioma::programs::p05_mechanism_exploration::robust_portfolio::{
    RobustInterventionPortfolio, RobustPortfolioDisposition,
};
use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F29";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismValidationPlan1@1";
pub const MAX_ARMS: usize = 256;
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_ACTIONS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationArmRole {
    Control,
    Intervention,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismValidationArm {
    pub arm_id: String,
    pub candidate_id: Option<String>,
    pub label: String,
    pub target_node_id: String,
    pub protocol_id: String,
    pub role: ValidationArmRole,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismValidationPlanRequest {
    pub objective: String,
    pub portfolio: RobustInterventionPortfolio,
    pub power: PowerReestimationRequest,
    pub arms: Vec<MechanismValidationArm>,
    pub observations: Vec<PowerArmObservation>,
    pub require_qualified_portfolio: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismValidationDisposition {
    Qualified,
    Partial,
    Negative,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationActionKind {
    RunValidationBatch,
    ContinueReplicates,
    StopForEfficacy,
    StopForFutility,
    ResolveMissingArm,
    ResolveMissingObservation,
    ResolvePortfolioHold,
    PreserveNegativeResult,
    ResolvePowerBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationAction {
    pub action_id: String,
    pub kind: ValidationActionKind,
    pub arm_id: Option<String>,
    pub candidate_id: Option<String>,
    pub priority_milli: u32,
    pub consumer: String,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismValidationPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub portfolio_digest: ContentHash,
    pub power_plan: Option<PowerReestimationPlan>,
    pub candidate_order: Vec<String>,
    pub selected_candidate_order: Vec<String>,
    pub mapped_arm_order: Vec<String>,
    pub missing_candidate_order: Vec<String>,
    pub missing_observation_order: Vec<String>,
    pub action_order: Vec<String>,
    pub actions: Vec<ValidationAction>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismValidationDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismValidationError {
    #[error("mechanism validation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism validation power planning failed: {0}")]
    Power(#[from] PowerReestimationError),
    #[error("mechanism validation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism validation digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MechanismValidationPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "portfolio_digest": output.portfolio_digest,
        "power_plan": output.power_plan,
        "candidate_order": output.candidate_order,
        "selected_candidate_order": output.selected_candidate_order,
        "mapped_arm_order": output.mapped_arm_order,
        "missing_candidate_order": output.missing_candidate_order,
        "missing_observation_order": output.missing_observation_order,
        "action_order": output.action_order,
        "actions": output.actions,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

fn validate_request(
    request: &MechanismValidationPlanRequest,
) -> Result<(), MechanismValidationError> {
    if request.objective.trim().is_empty()
        || request.portfolio.objective != request.objective
        || request.power.objective != request.objective
        || request.power.model_system != request.portfolio.model_system
        || request.arms.is_empty()
        || request.arms.len() > MAX_ARMS
        || request.observations.len() > MAX_OBSERVATIONS
    {
        return Err(MechanismValidationError::InvalidRequest(
            "objective, model binding, bounded arms, and observations are required".into(),
        ));
    }
    request
        .portfolio
        .validate()
        .map_err(|error| MechanismValidationError::InvalidRequest(error.to_string()))?;
    let mut arm_ids = BTreeSet::new();
    let mut candidate_ids = BTreeSet::new();
    let mut controls = 0_usize;
    for arm in &request.arms {
        arm.artifact
            .validate()
            .map_err(|error| MechanismValidationError::InvalidRequest(error.to_string()))?;
        if arm.arm_id.trim().is_empty()
            || !arm_ids.insert(arm.arm_id.clone())
            || arm.label.trim().is_empty()
            || arm.target_node_id.trim().is_empty()
            || arm.protocol_id.trim().is_empty()
        {
            return Err(MechanismValidationError::InvalidRequest(
                "arm identity, target, protocol, and artifact bindings must be unique and non-empty".into(),
            ));
        }
        match arm.role {
            ValidationArmRole::Control if arm.candidate_id.is_some() => {
                return Err(MechanismValidationError::InvalidRequest(
                    "control arms cannot bind a mechanism candidate".into(),
                ));
            }
            ValidationArmRole::Control => controls += 1,
            ValidationArmRole::Intervention => {
                let Some(candidate_id) = &arm.candidate_id else {
                    return Err(MechanismValidationError::InvalidRequest(
                        "intervention arms require a mechanism candidate".into(),
                    ));
                };
                if !candidate_ids.insert(candidate_id.clone()) {
                    return Err(MechanismValidationError::InvalidRequest(
                        "each mechanism candidate may map to at most one validation arm".into(),
                    ));
                }
            }
        }
    }
    if controls != 1 || request.power.control_arm_id.trim().is_empty() {
        return Err(MechanismValidationError::InvalidRequest(
            "exactly one control arm and a declared power control id are required".into(),
        ));
    }
    let control_id = request
        .arms
        .iter()
        .find(|arm| matches!(arm.role, ValidationArmRole::Control))
        .map(|arm| arm.arm_id.as_str())
        .expect("validated control arm exists");
    if request.power.control_arm_id != control_id {
        return Err(MechanismValidationError::InvalidRequest(
            "power control_arm_id must bind the declared control arm".into(),
        ));
    }
    let observation_ids = request
        .observations
        .iter()
        .map(|observation| observation.artifact.artifact_id.clone())
        .collect::<BTreeSet<_>>();
    if observation_ids.len() != request.observations.len() {
        return Err(MechanismValidationError::InvalidRequest(
            "observation artifact identities must be unique".into(),
        ));
    }
    Ok(())
}

fn action(
    action_id: impl Into<String>,
    kind: ValidationActionKind,
    arm_id: Option<String>,
    candidate_id: Option<String>,
    priority_milli: u32,
    consumer: &str,
    rationale: impl Into<String>,
) -> ValidationAction {
    ValidationAction {
        action_id: action_id.into(),
        kind,
        arm_id,
        candidate_id,
        priority_milli,
        consumer: consumer.into(),
        rationale: rationale.into(),
    }
}

impl MechanismValidationPlan {
    pub fn validate(&self) -> Result<(), MechanismValidationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.portfolio_digest.as_str().len() != 64
            || !canonical(&self.candidate_order)
            || !canonical(&self.selected_candidate_order)
            || !canonical(&self.mapped_arm_order)
            || !canonical(&self.missing_candidate_order)
            || !canonical(&self.missing_observation_order)
            || self.actions.len() > MAX_ACTIONS
            || self.actions.len() != self.action_order.len()
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_step.trim().is_empty()
        {
            return Err(MechanismValidationError::InvalidOutput(
                "identity, digest, ordering, action, evidence, or next-step invariants are invalid"
                    .into(),
            ));
        }
        let candidate_set = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let selected_set = self
            .selected_candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if selected_set.len() != self.selected_candidate_order.len()
            || !selected_set.is_subset(&candidate_set)
            || self
                .missing_candidate_order
                .iter()
                .any(|candidate| !selected_set.contains(candidate))
            || self
                .missing_candidate_order
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != self.missing_candidate_order.len()
            || self
                .mapped_arm_order
                .iter()
                .any(|arm| arm.trim().is_empty())
        {
            return Err(MechanismValidationError::InvalidOutput(
                "selected candidates, missing candidates, and mapped arms do not reconcile".into(),
            ));
        }
        let action_ids = self
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<BTreeSet<_>>();
        if action_ids.len() != self.actions.len()
            || action_ids != self.action_order.iter().cloned().collect::<BTreeSet<_>>()
            || self.actions.windows(2).any(|pair| {
                pair[0].priority_milli < pair[1].priority_milli
                    || (pair[0].priority_milli == pair[1].priority_milli
                        && pair[0].action_id > pair[1].action_id)
            })
            || self.actions.iter().any(|action| {
                action.consumer.trim().is_empty()
                    || action.rationale.trim().is_empty()
                    || action.priority_milli > 1_000_000
            })
        {
            return Err(MechanismValidationError::InvalidOutput(
                "validation actions are not unique, ordered, or fully specified".into(),
            ));
        }
        if let Some(power) = &self.power_plan {
            power
                .validate()
                .map_err(|error| MechanismValidationError::InvalidOutput(error.to_string()))?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismValidationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismValidationError::InvalidOutput(
                "mechanism validation digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile robust mechanism candidates into a power-aware, sequential validation queue.
pub fn plan_glioma_mechanism_validation(
    request: &MechanismValidationPlanRequest,
) -> Result<MechanismValidationPlan, MechanismValidationError> {
    validate_request(request)?;
    let candidate_order = request.portfolio.candidate_order.clone();
    let selected_candidate_order = request
        .portfolio
        .selected_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected = selected_candidate_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let arms_by_candidate = request
        .arms
        .iter()
        .filter_map(|arm| {
            arm.candidate_id
                .as_ref()
                .map(|candidate| (candidate.clone(), arm))
        })
        .collect::<BTreeMap<_, _>>();
    let control = request
        .arms
        .iter()
        .find(|arm| matches!(arm.role, ValidationArmRole::Control))
        .expect("validated control arm exists");
    let missing_candidate_order = selected
        .iter()
        .filter(|candidate| !arms_by_candidate.contains_key(*candidate))
        .cloned()
        .collect::<Vec<_>>();
    let mapped_arm_order = selected
        .iter()
        .filter_map(|candidate| {
            arms_by_candidate
                .get(candidate)
                .map(|arm| arm.arm_id.clone())
        })
        .chain(std::iter::once(control.arm_id.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mapped_set = mapped_arm_order.iter().cloned().collect::<BTreeSet<_>>();
    let observed_arms = request
        .observations
        .iter()
        .map(|observation| observation.arm_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_observation_order = mapped_set
        .iter()
        .filter(|arm_id| !observed_arms.contains(*arm_id))
        .cloned()
        .collect::<Vec<_>>();
    let portfolio_hold = request.require_qualified_portfolio
        && request.portfolio.disposition != RobustPortfolioDisposition::Qualified;
    let can_plan_power = missing_candidate_order.is_empty()
        && missing_observation_order.is_empty()
        && !selected.is_empty()
        && !portfolio_hold;
    let power_plan = if can_plan_power {
        let observations = request
            .observations
            .iter()
            .filter(|observation| mapped_set.contains(&observation.arm_id))
            .cloned()
            .collect::<Vec<_>>();
        Some(plan_glioma_power_reestimation(
            &request.power,
            &observations,
        )?)
    } else {
        None
    };

    let mut negative = request.portfolio.negative_evidence.clone();
    let mut uncertainty = request.portfolio.uncertainty.clone();
    if !missing_candidate_order.is_empty() {
        negative.extend(
            missing_candidate_order
                .iter()
                .map(|candidate| format!("missing-validation-arm:{candidate}")),
        );
    }
    if !missing_observation_order.is_empty() {
        uncertainty.extend(
            missing_observation_order
                .iter()
                .map(|arm| format!("missing-local-observation:{arm}")),
        );
    }
    if portfolio_hold {
        uncertainty.push("robust-portfolio-not-qualified".into());
    }
    let mut actions = Vec::new();
    for candidate in &missing_candidate_order {
        actions.push(action(
            format!("resolve:missing-arm:{candidate}"),
            ValidationActionKind::ResolveMissingArm,
            None,
            Some(candidate.clone()),
            950_000,
            "experimentalist",
            "bind every robustly selected mechanism candidate to one local validation protocol before power planning",
        ));
    }
    for arm_id in &missing_observation_order {
        let candidate_id = request
            .arms
            .iter()
            .find(|arm| arm.arm_id == *arm_id)
            .and_then(|arm| arm.candidate_id.clone());
        actions.push(action(
            format!("resolve:missing-observation:{arm_id}"),
            ValidationActionKind::ResolveMissingObservation,
            Some(arm_id.clone()),
            candidate_id,
            900_000,
            "data steward",
            "return a typed local aggregate observation; an unmeasured arm cannot be treated as a null or pass",
        ));
    }
    if portfolio_hold {
        actions.push(action(
            "resolve:portfolio-hold",
            ValidationActionKind::ResolvePortfolioHold,
            None,
            None,
            880_000,
            "mechanism scientist",
            "resolve model disagreement, lower-tail effect, risk, or feasibility gates before validation",
        ));
    }
    if let Some(power) = &power_plan {
        negative.extend(power.negative_evidence.clone());
        uncertainty.extend(power.uncertainty.clone());
        for decision in &power.decisions {
            let candidate_id = request
                .arms
                .iter()
                .find(|arm| arm.arm_id == decision.arm_id)
                .and_then(|arm| arm.candidate_id.clone());
            let (kind, priority, consumer, rationale) = match decision.decision {
                PowerDecisionKind::EfficacyStop => (
                    ValidationActionKind::StopForEfficacy,
                    820_000,
                    "methods reviewer",
                    "the interim effect and power proxy clear the declared efficacy boundary; preserve the stopping evidence and require independent review",
                ),
                PowerDecisionKind::FutilityStop => (
                    ValidationActionKind::StopForFutility,
                    810_000,
                    "methods reviewer",
                    "the observed arm is below the futility boundary; stop this validation path and preserve the negative result",
                ),
                PowerDecisionKind::Continue | PowerDecisionKind::HoldUnderpowered => (
                    ValidationActionKind::ContinueReplicates,
                    800_000,
                    "experimentalist",
                    "the next information-bearing replicate batch remains bounded by interim power, risk, and budget gates",
                ),
                PowerDecisionKind::RiskBlocked | PowerDecisionKind::BudgetBlocked => (
                    ValidationActionKind::ResolvePowerBoundary,
                    790_000,
                    "principal investigator",
                    "the power plan cannot admit this arm under the declared risk or budget boundary",
                ),
            };
            actions.push(action(
                format!("power:{:?}:{}", kind, decision.arm_id),
                kind,
                Some(decision.arm_id.clone()),
                candidate_id,
                priority,
                consumer,
                rationale,
            ));
        }
        if power
            .decisions
            .iter()
            .any(|decision| matches!(decision.decision, PowerDecisionKind::FutilityStop))
        {
            actions.push(action(
                "preserve:negative-validation-result",
                ValidationActionKind::PreserveNegativeResult,
                None,
                None,
                760_000,
                "reproducibility steward",
                "publish the futility or null result as first-class evidence and do not reinterpret it as missing success",
            ));
        }
    }
    actions.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    actions.truncate(MAX_ACTIONS);
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect();
    negative.sort();
    negative.dedup();
    uncertainty.sort();
    uncertainty.dedup();

    let disposition = if selected.is_empty() {
        MechanismValidationDisposition::Unresolved
    } else if portfolio_hold
        || !missing_candidate_order.is_empty()
        || !missing_observation_order.is_empty()
    {
        MechanismValidationDisposition::Blocked
    } else {
        match power_plan.as_ref().map(|plan| plan.disposition) {
            Some(super::power_reestimation::PowerReestimationDisposition::Efficacy)
                if request.portfolio.disposition == RobustPortfolioDisposition::Qualified =>
            {
                MechanismValidationDisposition::Qualified
            }
            Some(super::power_reestimation::PowerReestimationDisposition::Futility) => {
                MechanismValidationDisposition::Negative
            }
            Some(super::power_reestimation::PowerReestimationDisposition::Efficacy)
            | Some(super::power_reestimation::PowerReestimationDisposition::Continue) => {
                MechanismValidationDisposition::Partial
            }
            Some(
                super::power_reestimation::PowerReestimationDisposition::Hold
                | super::power_reestimation::PowerReestimationDisposition::RiskBlocked
                | super::power_reestimation::PowerReestimationDisposition::BudgetBlocked
                | super::power_reestimation::PowerReestimationDisposition::Unresolved,
            )
            | None => MechanismValidationDisposition::Blocked,
        }
    };
    let next_step = match disposition {
        MechanismValidationDisposition::Qualified => {
            "retain the qualified local stop evidence, request independent preclinical replication, and do not promote it beyond the declared research boundary"
        }
        MechanismValidationDisposition::Partial => {
            "execute only the bounded continuation actions, return local aggregates, and recompile the validation plan"
        }
        MechanismValidationDisposition::Negative => {
            "preserve the negative validation result, inspect mechanism and assay failure modes, and avoid silent hypothesis promotion"
        }
        MechanismValidationDisposition::Blocked => {
            "resolve missing bindings, portfolio disagreement, risk, power, or budget gates before any validation dispatch"
        }
        MechanismValidationDisposition::Unresolved => {
            "supply a robust selected mechanism and typed local control/intervention observations before validation can be planned"
        }
    }
    .into();
    let mut output = MechanismValidationPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.portfolio.model_system,
        portfolio_digest: request.portfolio.digest.clone(),
        power_plan,
        candidate_order,
        selected_candidate_order,
        mapped_arm_order,
        missing_candidate_order,
        missing_observation_order,
        action_order,
        actions,
        negative_evidence: negative,
        uncertainty,
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-validation"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismValidationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::counterfactual::CounterfactualIntervention;
    use crate::glioma::programs::p05_mechanism_exploration::ensemble_counterfactual::CounterfactualModel;
    use crate::glioma::programs::p05_mechanism_exploration::graph_propagation::{
        MechanismGraphEdge, MechanismGraphNode, MechanismGraphRelation,
    };
    use crate::glioma::programs::p05_mechanism_exploration::robust_portfolio::{
        plan_glioma_robust_intervention_portfolio, PortfolioDirection, RobustInterventionCandidate,
        RobustInterventionRequest,
    };

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn model(id: &str, support: u16) -> CounterfactualModel {
        CounterfactualModel {
            model_id: id.into(),
            prior_milli: 500,
            nodes: vec![
                MechanismGraphNode {
                    node_id: "egfr".into(),
                    label: "egfr".into(),
                    modality: crate::glioma_engine::GliomaModality::Genomics,
                    prior_milli: 0,
                    support_milli: support,
                    contradiction_milli: 0,
                },
                MechanismGraphNode {
                    node_id: "invasion".into(),
                    label: "invasion".into(),
                    modality: crate::glioma_engine::GliomaModality::Imaging,
                    prior_milli: 0,
                    support_milli: 0,
                    contradiction_milli: 0,
                },
            ],
            edges: vec![MechanismGraphEdge {
                edge_id: format!("{id}-edge"),
                source_node_id: "egfr".into(),
                target_node_id: "invasion".into(),
                relation: MechanismGraphRelation::Activates,
                confidence_milli: 900,
                evidence_order: vec![format!("evidence-{id}")],
            }],
        }
    }

    fn portfolio() -> RobustInterventionPortfolio {
        plan_glioma_robust_intervention_portfolio(
            &RobustInterventionRequest {
                objective: "validate robust invasion suppression".into(),
                model_system: GliomaModelSystem::Organoid,
                max_iterations: 100,
                convergence_tolerance_milli: 1,
                damping_milli: 600,
                min_edge_confidence_milli: 500,
                direction: PortfolioDirection::Decrease,
                budget_units: 3,
                max_selected: 1,
                min_robust_effect_milli: 10,
                min_agreement_milli: 750,
                risk_ceiling_milli: 800,
                effect_weight_milli: 500,
                tail_weight_milli: 300,
                worst_case_weight_milli: 200,
                feasibility_weight_milli: 1_000,
                risk_penalty_milli: 1,
            },
            &[model("conservative", 700), model("optimistic", 900)],
            &[RobustInterventionCandidate {
                candidate_id: "egfr-invasion".into(),
                label: "EGFR invasion perturbation".into(),
                intervention: CounterfactualIntervention {
                    intervention_id: "egfr-invasion-intervention".into(),
                    node_id: "egfr".into(),
                    delta_milli: -600,
                    rationale: "test EGFR to invasion mechanism".into(),
                    evidence_order: vec!["evidence-egfr".into()],
                },
                target_node_id: "invasion".into(),
                redundancy_group: "egfr".into(),
                feasibility_milli: 1_000,
                cost_units: 1,
                risk_milli: 100,
            }],
        )
        .unwrap()
    }

    fn request(with_observations: bool) -> MechanismValidationPlanRequest {
        let portfolio = portfolio();
        let artifact = |id: &str| LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: hash(id),
            content_type: "application/vnd.aurora.glioma-validation+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        };
        let arms = vec![
            MechanismValidationArm {
                arm_id: "control".into(),
                candidate_id: None,
                label: "vehicle control".into(),
                target_node_id: "invasion".into(),
                protocol_id: "invasion-validation-v1".into(),
                role: ValidationArmRole::Control,
                artifact: artifact("control-arm"),
            },
            MechanismValidationArm {
                arm_id: "egfr-arm".into(),
                candidate_id: Some("egfr-invasion".into()),
                label: "EGFR perturbation".into(),
                target_node_id: "invasion".into(),
                protocol_id: "invasion-validation-v1".into(),
                role: ValidationArmRole::Intervention,
                artifact: artifact("egfr-arm"),
            },
        ];
        let observations = if with_observations {
            vec![
                PowerArmObservation {
                    arm_id: "control".into(),
                    label: "vehicle control".into(),
                    artifact: artifact("control-observation"),
                    model_system: GliomaModelSystem::Organoid,
                    mean_response_milli: 100,
                    variance_milli2: 100,
                    observations: 2,
                    risk_milli: 100,
                    cost_units: 1,
                },
                PowerArmObservation {
                    arm_id: "egfr-arm".into(),
                    label: "EGFR perturbation".into(),
                    artifact: artifact("egfr-observation"),
                    model_system: GliomaModelSystem::Organoid,
                    mean_response_milli: 160,
                    variance_milli2: 100,
                    observations: 2,
                    risk_milli: 100,
                    cost_units: 1,
                },
            ]
        } else {
            Vec::new()
        };
        MechanismValidationPlanRequest {
            objective: "validate robust invasion suppression".into(),
            portfolio,
            power: PowerReestimationRequest {
                objective: "validate robust invasion suppression".into(),
                model_system: GliomaModelSystem::Organoid,
                endpoint: "invasion-index".into(),
                control_arm_id: "control".into(),
                target_effect_milli: 10,
                alpha_total_milli: 100,
                power_target_milli: 500,
                current_look: 1,
                max_looks: 2,
                min_replicates_per_arm: 1,
                max_replicates_per_arm: 16,
                max_new_replicates_per_arm: 4,
                budget_units: 16,
                risk_ceiling_milli: 800,
            },
            arms,
            observations,
            require_qualified_portfolio: true,
        }
    }

    #[test]
    fn robust_portfolio_drives_power_gated_validation_and_replays() {
        let first = plan_glioma_mechanism_validation(&request(true)).unwrap();
        let second = plan_glioma_mechanism_validation(&request(true)).unwrap();
        assert_eq!(first, second);
        assert!(first.power_plan.is_some());
        assert_eq!(first.disposition, MechanismValidationDisposition::Qualified);
        assert!(first
            .actions
            .iter()
            .any(|action| action.kind == ValidationActionKind::StopForEfficacy));
        first.validate().unwrap();
    }

    #[test]
    fn missing_observations_block_power_without_fabricating_a_null_result() {
        let output = plan_glioma_mechanism_validation(&request(false)).unwrap();
        assert!(output.power_plan.is_none());
        assert_eq!(output.disposition, MechanismValidationDisposition::Blocked);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item == "missing-local-observation:control"));
        assert!(output
            .actions
            .iter()
            .any(|action| action.kind == ValidationActionKind::ResolveMissingObservation));
        output.validate().unwrap();
    }
}
