//! Sequential Bayesian assay allocation for preclinical glioma experiments.
//!
//! The allocator is deliberately conservative and deterministic.  It uses a Beta posterior for
//! binary assay outcomes and a one-sided Cantelli lower bound for the probability that an arm's
//! effect over the local control exceeds the researcher's declared target.  Posterior uncertainty
//! drives exploration, while a risk ceiling, replicate floor, and explicit budget keep the result
//! executable as a bounded next batch rather than an unreviewed recommendation.  The function
//! never dispatches a protocol, chooses a clinical dose, or moves raw observations.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F10";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveAllocation1@1";
pub const MAX_ARMS: usize = 256;
pub const MAX_OBSERVATIONS_PER_ARM: u32 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveAllocationRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub control_arm_id: String,
    pub target_effect_milli: u16,
    pub min_replicates_per_arm: u32,
    pub min_probability_milli: u16,
    pub max_posterior_uncertainty_milli: u32,
    pub exploration_weight_milli: u16,
    pub max_selected_arms: usize,
    pub max_new_replicates: u32,
    pub budget_units: u64,
    pub risk_ceiling_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveArmObservation {
    pub arm_id: String,
    pub label: String,
    pub artifact: LocalArtifactRef,
    pub model_system: GliomaModelSystem,
    pub successes: u32,
    pub failures: u32,
    pub prior_alpha: u32,
    pub prior_beta: u32,
    pub risk_milli: u16,
    pub cost_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveAllocationActionKind {
    Allocate,
    Hold,
    Deprioritize,
    BudgetBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveArmPosterior {
    pub arm_id: String,
    pub label: String,
    pub is_control: bool,
    pub alpha: u64,
    pub beta: u64,
    pub observations: u32,
    pub mean_response_milli: u32,
    pub variance_milli2: u64,
    pub uncertainty_milli: u32,
    pub effect_vs_control_milli: i32,
    pub probability_exceeds_target_milli: u16,
    pub exploration_bonus_milli: u16,
    pub utility_milli: u16,
    pub recommended_replicates: u32,
    pub allocated_replicates: u32,
    pub action: AdaptiveAllocationActionKind,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveAllocationDisposition {
    Qualified,
    Underpowered,
    Negative,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveAllocation {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub control_arm_id: String,
    pub control_mean_response_milli: u32,
    pub posterior_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub underpowered_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub risk_blocked_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub budget_remaining_units: u64,
    pub posteriors: Vec<AdaptiveArmPosterior>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: AdaptiveAllocationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptiveAllocationError {
    #[error("adaptive allocation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("adaptive allocation arm is invalid: {0}")]
    InvalidArm(String),
    #[error("adaptive allocation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive allocation digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &AdaptiveAllocation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "control_arm_id": output.control_arm_id,
        "control_mean_response_milli": output.control_mean_response_milli,
        "posterior_order": output.posterior_order,
        "selected_order": output.selected_order,
        "underpowered_order": output.underpowered_order,
        "negative_order": output.negative_order,
        "risk_blocked_order": output.risk_blocked_order,
        "uncertainty_order": output.uncertainty_order,
        "budget_remaining_units": output.budget_remaining_units,
        "posteriors": output.posteriors,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn integer_sqrt(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut low = 1_u128;
    let mut high = value.min(u128::from(u64::MAX));
    while low <= high {
        let mid = low + (high - low) / 2;
        if mid <= value / mid {
            low = mid + 1;
        } else {
            high = mid - 1;
        }
    }
    high
}

fn posterior(
    arm: &AdaptiveArmObservation,
) -> Result<(u64, u64, u32, u32, u64, u32), AdaptiveAllocationError> {
    let alpha = u64::from(arm.prior_alpha) + u64::from(arm.successes);
    let beta = u64::from(arm.prior_beta) + u64::from(arm.failures);
    let observations = arm.successes.saturating_add(arm.failures);
    let total = alpha.saturating_add(beta);
    if alpha == 0 || beta == 0 || total == 0 {
        return Err(AdaptiveAllocationError::InvalidArm(
            "posterior alpha/beta and total trials must be positive".into(),
        ));
    }
    let mean = ((u128::from(alpha) * 1_000) / u128::from(total)) as u32;
    let denominator = u128::from(total)
        .saturating_mul(u128::from(total))
        .saturating_mul(u128::from(total.saturating_add(1)));
    let variance = (u128::from(alpha)
        .saturating_mul(u128::from(beta))
        .saturating_mul(1_000_000))
        / denominator.max(1);
    let uncertainty = integer_sqrt(variance).min(u128::from(u32::MAX)) as u32;
    Ok((
        alpha,
        beta,
        observations,
        mean,
        variance.min(u128::from(u64::MAX)) as u64,
        uncertainty,
    ))
}

fn probability_exceeds_target(effect_milli: i32, target_milli: u16, variance_milli2: u128) -> u16 {
    let excess = i128::from(effect_milli) - i128::from(target_milli);
    if excess <= 0 {
        return 0;
    }
    let squared = (excess as u128).saturating_mul(excess as u128);
    let probability = squared.saturating_mul(1_000) / squared.saturating_add(variance_milli2);
    probability.min(1_000) as u16
}

fn recommended_replicates(
    observations: u32,
    minimum: u32,
    uncertainty_milli: u32,
    max_uncertainty_milli: u32,
    probability_milli: u16,
    minimum_probability_milli: u16,
    max_new: u32,
) -> u32 {
    let floor = minimum.saturating_sub(observations);
    let precision = if uncertainty_milli > max_uncertainty_milli {
        let excess = uncertainty_milli - max_uncertainty_milli;
        excess.saturating_add(max_uncertainty_milli.max(1) - 1) / max_uncertainty_milli.max(1)
    } else {
        0
    };
    let confidence_follow_up = if probability_milli >= minimum_probability_milli
        && uncertainty_milli > max_uncertainty_milli / 2
    {
        1
    } else {
        0
    };
    floor.max(precision).max(confidence_follow_up).min(max_new)
}

impl AdaptiveAllocation {
    pub fn validate(&self) -> Result<(), AdaptiveAllocationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.control_arm_id.trim().is_empty()
            || self.control_mean_response_milli > 1_000
            || self
                .posterior_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .underpowered_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .negative_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .risk_blocked_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .uncertainty_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.posteriors.windows(2).any(|pair| {
                pair[0].utility_milli < pair[1].utility_milli
                    || (pair[0].utility_milli == pair[1].utility_milli
                        && pair[0].arm_id > pair[1].arm_id)
            })
            || self.posteriors.iter().any(|posterior| {
                posterior.arm_id.trim().is_empty()
                    || posterior.label.trim().is_empty()
                    || posterior.alpha == 0
                    || posterior.beta == 0
                    || posterior.mean_response_milli > 1_000
                    || posterior.uncertainty_milli > 1_000
                    || posterior.probability_exceeds_target_milli > 1_000
                    || posterior.exploration_bonus_milli > 1_000
                    || posterior.utility_milli > 1_000
                    || posterior.allocated_replicates > posterior.recommended_replicates
                    || posterior.rationale.trim().is_empty()
            })
        {
            return Err(AdaptiveAllocationError::InvalidOutput(
                "identity, ordering, posterior bounds, or rationale is invalid".into(),
            ));
        }
        let ids = self
            .posteriors
            .iter()
            .map(|posterior| posterior.arm_id.clone())
            .collect::<BTreeSet<_>>();
        if ids.len() != self.posteriors.len()
            || self.posterior_order != ids.iter().cloned().collect::<Vec<_>>()
            || self.selected_order.iter().any(|id| !ids.contains(id))
            || self.underpowered_order.iter().any(|id| !ids.contains(id))
            || self.negative_order.iter().any(|id| !ids.contains(id))
            || self.risk_blocked_order.iter().any(|id| !ids.contains(id))
            || self.uncertainty_order.iter().any(|id| !ids.contains(id))
            || self.selected_order.iter().any(|id| {
                self.posteriors
                    .iter()
                    .find(|posterior| &posterior.arm_id == id)
                    .is_none_or(|posterior| posterior.allocated_replicates == 0)
            })
            || self.selected_order
                != self
                    .posteriors
                    .iter()
                    .filter(|posterior| posterior.allocated_replicates > 0)
                    .map(|posterior| posterior.arm_id.clone())
                    .collect::<Vec<_>>()
        {
            return Err(AdaptiveAllocationError::InvalidOutput(
                "posterior identity or allocation partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AdaptiveAllocationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AdaptiveAllocationError::InvalidOutput(
                "digest is not bound to adaptive allocation".into(),
            ));
        }
        Ok(())
    }
}

pub fn allocate_glioma_assays(
    request: &AdaptiveAllocationRequest,
    arms: &[AdaptiveArmObservation],
) -> Result<AdaptiveAllocation, AdaptiveAllocationError> {
    if request.objective.trim().is_empty()
        || request.control_arm_id.trim().is_empty()
        || request.target_effect_milli > 1_000
        || request.min_replicates_per_arm == 0
        || request.min_replicates_per_arm > MAX_OBSERVATIONS_PER_ARM
        || request.min_probability_milli > 1_000
        || request.max_posterior_uncertainty_milli == 0
        || request.max_posterior_uncertainty_milli > 1_000
        || request.exploration_weight_milli > 1_000
        || request.max_selected_arms == 0
        || request.max_new_replicates == 0
        || request.risk_ceiling_milli > 1_000
        || arms.is_empty()
        || arms.len() > MAX_ARMS
    {
        return Err(AdaptiveAllocationError::InvalidRequest(
            "objective, control, posterior thresholds, arm bound, and positive budgets are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut control_index = None;
    for (index, arm) in arms.iter().enumerate() {
        if arm.arm_id.trim().is_empty()
            || arm.label.trim().is_empty()
            || arm.model_system != request.model_system
            || arm.artifact.validate().is_err()
            || arm.artifact.contains_human_data
            || arm.artifact.contains_direct_identifiers
            || !arm.artifact.local_only
            || arm.prior_alpha == 0
            || arm.prior_beta == 0
            || arm.successes.saturating_add(arm.failures) > MAX_OBSERVATIONS_PER_ARM
            || arm.cost_units == 0
            || arm.risk_milli > 1_000
            || !ids.insert(arm.arm_id.clone())
        {
            return Err(AdaptiveAllocationError::InvalidArm(
                "arm identity, local preclinical artifact, model binding, prior, trial count, risk, cost, or uniqueness is invalid".into(),
            ));
        }
        if arm.arm_id == request.control_arm_id {
            control_index = Some(index);
        }
    }
    let control_index = control_index.ok_or_else(|| {
        AdaptiveAllocationError::InvalidRequest("declared control arm is absent".into())
    })?;
    let control = &arms[control_index];
    let (_, _, _, control_mean, control_variance, _) = posterior(control)?;
    let mut posteriors = Vec::with_capacity(arms.len());
    let mut underpowered = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut risk_blocked = BTreeSet::new();
    let mut uncertainty_order = BTreeSet::new();
    for arm in arms {
        let (alpha, beta, observations, mean, variance, uncertainty) = posterior(arm)?;
        let is_control = arm.arm_id == request.control_arm_id;
        let effect = if is_control {
            0
        } else {
            mean as i32 - control_mean as i32
        };
        let difference_variance = if is_control {
            0_u128
        } else {
            u128::from(variance).saturating_add(u128::from(control_variance))
        };
        let probability = if is_control {
            0
        } else {
            probability_exceeds_target(effect, request.target_effect_milli, difference_variance)
        };
        let exploration_bonus = if is_control {
            0
        } else {
            (u64::from(uncertainty).saturating_mul(u64::from(request.exploration_weight_milli))
                / 1_000)
                .min(1_000) as u16
        };
        let floor_bonus = if !is_control && observations < request.min_replicates_per_arm {
            400_u64
        } else {
            0
        };
        let risk_penalty = if is_control {
            0_u64
        } else {
            u64::from(arm.risk_milli) / 4
        };
        let utility = if is_control {
            0
        } else {
            (u64::from(probability) * 6 / 10)
                .saturating_add(u64::from(exploration_bonus))
                .saturating_add(floor_bonus)
                .saturating_sub(risk_penalty)
                .min(1_000) as u16
        };
        let recommended = if is_control || arm.risk_milli > request.risk_ceiling_milli {
            0
        } else {
            recommended_replicates(
                observations,
                request.min_replicates_per_arm,
                uncertainty,
                request.max_posterior_uncertainty_milli,
                probability,
                request.min_probability_milli,
                request.max_new_replicates,
            )
        };
        let action = if is_control || arm.risk_milli > request.risk_ceiling_milli {
            AdaptiveAllocationActionKind::Deprioritize
        } else if recommended == 0 {
            AdaptiveAllocationActionKind::Hold
        } else {
            AdaptiveAllocationActionKind::Allocate
        };
        if !is_control && observations < request.min_replicates_per_arm {
            underpowered.insert(arm.arm_id.clone());
        }
        if !is_control
            && observations >= request.min_replicates_per_arm
            && probability < request.min_probability_milli
        {
            negative.insert(arm.arm_id.clone());
        }
        if !is_control && arm.risk_milli > request.risk_ceiling_milli {
            risk_blocked.insert(arm.arm_id.clone());
        }
        if !is_control && uncertainty > request.max_posterior_uncertainty_milli {
            uncertainty_order.insert(arm.arm_id.clone());
        }
        let rationale = if is_control {
            "local control posterior anchors every treatment contrast and is never adaptively allocated".into()
        } else if arm.risk_milli > request.risk_ceiling_milli {
            "arm exceeds the declared preclinical risk ceiling".into()
        } else if probability >= request.min_probability_milli {
            "posterior lower bound supports the target effect; allocate only the bounded precision batch".into()
        } else if observations < request.min_replicates_per_arm {
            "arm is below the replicate floor; exploration is required before a negative claim"
                .into()
        } else {
            "posterior lower bound does not support the target effect; hold and preserve the negative result".into()
        };
        posteriors.push(AdaptiveArmPosterior {
            arm_id: arm.arm_id.clone(),
            label: arm.label.clone(),
            is_control,
            alpha,
            beta,
            observations,
            mean_response_milli: mean,
            variance_milli2: variance,
            uncertainty_milli: uncertainty,
            effect_vs_control_milli: effect,
            probability_exceeds_target_milli: probability,
            exploration_bonus_milli: exploration_bonus,
            utility_milli: utility,
            recommended_replicates: recommended,
            allocated_replicates: 0,
            action,
            rationale,
        });
    }
    posteriors.sort_by(|left, right| {
        right
            .utility_milli
            .cmp(&left.utility_milli)
            .then_with(|| left.arm_id.cmp(&right.arm_id))
    });
    let mut remaining_budget = request.budget_units;
    let mut selected_order = Vec::new();
    for posterior in posteriors
        .iter_mut()
        .filter(|posterior| !posterior.is_control && posterior.recommended_replicates > 0)
        .take(request.max_selected_arms)
    {
        let arm = arms
            .iter()
            .find(|arm| arm.arm_id == posterior.arm_id)
            .expect("validated arm exists");
        let affordable = if arm.cost_units == 0 {
            0
        } else {
            (remaining_budget / u64::from(arm.cost_units))
                .min(u64::from(posterior.recommended_replicates)) as u32
        };
        if affordable > 0 {
            posterior.allocated_replicates = affordable;
            remaining_budget = remaining_budget
                .saturating_sub(u64::from(affordable).saturating_mul(u64::from(arm.cost_units)));
            selected_order.push(posterior.arm_id.clone());
            if affordable < posterior.recommended_replicates {
                posterior.rationale =
                    "hard budget capped the otherwise recommended next batch".into();
            }
        } else {
            posterior.action = AdaptiveAllocationActionKind::BudgetBlocked;
            posterior.rationale = "hard budget cannot fund even one recommended replicate".into();
        }
    }
    let posterior_order = ids.iter().cloned().collect::<Vec<_>>();
    let mut negative_evidence = BTreeSet::new();
    if !negative.is_empty() {
        negative_evidence.insert("posterior-probability-below-target-after-replicate-floor".into());
    }
    if !risk_blocked.is_empty() {
        negative_evidence.insert("risk-ceiling-blocked-unsafe-arm".into());
    }
    let mut uncertainty = BTreeSet::new();
    if !underpowered.is_empty() {
        uncertainty.insert("replicate-floor-not-met".into());
    }
    if !uncertainty_order.is_empty() {
        uncertainty.insert("posterior-uncertainty-exceeds-declared-bound".into());
    }
    if selected_order.len()
        < posteriors
            .iter()
            .filter(|posterior| !posterior.is_control && posterior.recommended_replicates > 0)
            .count()
            .min(request.max_selected_arms)
    {
        uncertainty.insert("budget-capped-selected-arms".into());
    }
    if posteriors.iter().any(|posterior| {
        posterior.recommended_replicates > posterior.allocated_replicates
            && posterior.recommended_replicates > 0
    }) {
        uncertainty.insert("budget-capped-replicate-allocation".into());
    }
    let disposition = if selected_order.is_empty() && posteriors.len() <= 1 {
        AdaptiveAllocationDisposition::Unresolved
    } else if !underpowered.is_empty() && selected_order.is_empty() {
        AdaptiveAllocationDisposition::Underpowered
    } else if !negative.is_empty()
        && negative.len()
            == posteriors
                .iter()
                .filter(|posterior| !posterior.is_control)
                .count()
        && selected_order.is_empty()
    {
        AdaptiveAllocationDisposition::Negative
    } else if selected_order.is_empty() && !uncertainty.is_empty() {
        AdaptiveAllocationDisposition::BudgetBlocked
    } else {
        AdaptiveAllocationDisposition::Qualified
    };
    let mut output = AdaptiveAllocation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        control_arm_id: request.control_arm_id.clone(),
        control_mean_response_milli: control_mean,
        posterior_order,
        selected_order,
        underpowered_order: underpowered.into_iter().collect(),
        negative_order: negative.into_iter().collect(),
        risk_blocked_order: risk_blocked.into_iter().collect(),
        uncertainty_order: uncertainty_order.into_iter().collect(),
        budget_remaining_units: remaining_budget,
        posteriors,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| AdaptiveAllocationError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AdaptiveAllocationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("artifact-{id}"),
            content_hash: ContentHash::of_value(&serde_json::json!({"id": id})).unwrap(),
            content_type: "application/vnd.aurora.glioma-adaptive-arm+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn arm(id: &str, successes: u32, failures: u32, risk_milli: u16) -> AdaptiveArmObservation {
        AdaptiveArmObservation {
            arm_id: id.into(),
            label: id.into(),
            artifact: artifact(id),
            model_system: GliomaModelSystem::Organoid,
            successes,
            failures,
            prior_alpha: 1,
            prior_beta: 1,
            risk_milli,
            cost_units: 2,
        }
    }

    fn request() -> AdaptiveAllocationRequest {
        AdaptiveAllocationRequest {
            objective: "allocate organoid invasion replicates".into(),
            model_system: GliomaModelSystem::Organoid,
            control_arm_id: "control".into(),
            target_effect_milli: 100,
            min_replicates_per_arm: 30,
            min_probability_milli: 700,
            max_posterior_uncertainty_milli: 60,
            exploration_weight_milli: 300,
            max_selected_arms: 2,
            max_new_replicates: 20,
            budget_units: 80,
            risk_ceiling_milli: 700,
        }
    }

    #[test]
    fn high_posterior_arm_receives_bounded_next_batch() {
        let mut request = request();
        request.max_selected_arms = 1;
        let output = allocate_glioma_assays(
            &request,
            &[
                arm("control", 50, 50, 100),
                arm("egfr", 28, 2, 100),
                arm("matrix", 12, 18, 100),
            ],
        )
        .unwrap();
        assert_eq!(output.disposition, AdaptiveAllocationDisposition::Qualified);
        assert_eq!(output.selected_order, vec!["egfr"]);
        let egfr = output
            .posteriors
            .iter()
            .find(|posterior| posterior.arm_id == "egfr")
            .unwrap();
        assert!(egfr.probability_exceeds_target_milli >= 700);
        assert!(egfr.allocated_replicates > 0);
        output.validate().unwrap();
    }

    #[test]
    fn weak_arm_is_negative_only_after_replicate_floor() {
        let mut request = request();
        request.min_replicates_per_arm = 10;
        request.max_posterior_uncertainty_milli = 200;
        let output = allocate_glioma_assays(
            &request,
            &[arm("control", 80, 20, 100), arm("weak", 10, 90, 100)],
        )
        .unwrap();
        assert_eq!(output.disposition, AdaptiveAllocationDisposition::Negative);
        assert_eq!(output.negative_order, vec!["weak"]);
        assert!(output.selected_order.is_empty());
    }

    #[test]
    fn budget_cap_is_deterministic_and_replay_stable() {
        let mut request = request();
        request.budget_units = 2;
        let arms = vec![arm("control", 50, 50, 100), arm("egfr", 28, 2, 100)];
        let first = allocate_glioma_assays(&request, &arms).unwrap();
        let second = allocate_glioma_assays(&request, &arms).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.budget_remaining_units, 0);
        assert_eq!(first.posteriors[0].allocated_replicates, 1);
    }
}
