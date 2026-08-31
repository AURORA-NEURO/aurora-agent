//! Power-aware preclinical experiment design.
//!
//! The design program emits an executable allocation plan, not a suggestion to run an experiment.
//! It uses fixed-point arithmetic so a design has stable bytes across supported runtimes.  The
//! physical assay remains behind the engine's caller-owned executor and requires its own local
//! approvals.

use super::super::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F01";
pub const OUTPUT_SCHEMA: &str = "GliomaExperimentDesign1@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeKind {
    Continuous,
    Binary,
    Count,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentArm {
    pub arm_id: String,
    pub model_system: GliomaModelSystem,
    pub condition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub outcome: OutcomeKind,
    pub alpha_milli: u16,
    pub target_power_milli: u16,
    pub standardized_effect_milli: u16,
    pub variance_milli: u32,
    pub dropout_milli: u16,
    pub max_replicates_per_arm: u16,
    pub blocking_factors: Vec<String>,
    pub randomization_seed: ContentHash,
    pub release_null_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArmAllocation {
    pub arm_id: String,
    pub planned_replicates: u16,
    pub allocation_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentDisposition {
    Ready,
    Underpowered,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub outcome: OutcomeKind,
    pub required_replicates_per_arm: u16,
    pub total_replicates: u32,
    pub achieved_power_milli: u16,
    pub allocations: Vec<ArmAllocation>,
    pub blocking_factor_order: Vec<String>,
    pub assumptions: Vec<String>,
    pub acceptance_gate: String,
    pub negative_result_plan: String,
    pub blocked_order: Vec<String>,
    pub disposition: ExperimentDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExperimentError {
    #[error("experiment request is invalid: {0}")]
    InvalidRequest(String),
    #[error("experiment arm is invalid: {0}")]
    InvalidArm(String),
    #[error("experiment design is invalid: {0}")]
    InvalidOutput(String),
    #[error("experiment digest failed: {0}")]
    Digest(String),
}

fn z_alpha_milli(alpha_milli: u16) -> Option<u32> {
    match alpha_milli {
        1..=10 => Some(2_576),
        11..=50 => Some(1_960),
        51..=100 => Some(1_645),
        _ => None,
    }
}

fn z_power_milli(power_milli: u16) -> Option<u32> {
    match power_milli {
        800..=849 => Some(842),
        850..=899 => Some(1_036),
        900..=949 => Some(1_282),
        950..=999 => Some(1_645),
        1_000 => Some(3_000),
        _ => None,
    }
}

fn required_replicates(request: &ExperimentRequest) -> u16 {
    let z_sum = z_alpha_milli(request.alpha_milli).unwrap()
        + z_power_milli(request.target_power_milli).unwrap();
    let numerator = 2_u128 * (z_sum as u128) * (z_sum as u128) * request.variance_milli as u128;
    let denominator = (request.standardized_effect_milli as u128)
        * (request.standardized_effect_milli as u128)
        * 1_000;
    let base = numerator.div_ceil(denominator).max(2) as u32;
    let adjusted = (base as u64 * (1_000 + request.dropout_milli as u64)).div_ceil(1_000) as u32;
    adjusted.min(u16::MAX as u32) as u16
}

fn digest_input(design: &ExperimentDesign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": design.feature_id,
        "output_schema": design.output_schema,
        "objective": design.objective,
        "model_system": design.model_system,
        "outcome": design.outcome,
        "required_replicates_per_arm": design.required_replicates_per_arm,
        "total_replicates": design.total_replicates,
        "achieved_power_milli": design.achieved_power_milli,
        "allocations": design.allocations,
        "blocking_factor_order": design.blocking_factor_order,
        "assumptions": design.assumptions,
        "acceptance_gate": design.acceptance_gate,
        "negative_result_plan": design.negative_result_plan,
        "blocked_order": design.blocked_order,
        "disposition": design.disposition,
    })
}

impl ExperimentDesign {
    pub fn validate(&self) -> Result<(), ExperimentError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.required_replicates_per_arm == 0
            || self.total_replicates == 0
            || self.achieved_power_milli > 1_000
            || self.allocations.is_empty()
            || self.allocations.iter().any(|allocation| {
                allocation.arm_id.trim().is_empty()
                    || allocation.planned_replicates == 0
                    || allocation.allocation_order.len() != allocation.planned_replicates as usize
            })
            || self
                .blocking_factor_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self.blocked_order.windows(2).any(|pair| pair[0] > pair[1])
            || self.assumptions.iter().any(|item| item.trim().is_empty())
        {
            return Err(ExperimentError::InvalidOutput(
                "identity, allocation, power, or ordering is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|e| ExperimentError::Digest(e.to_string()))?;
        if expected != self.digest {
            return Err(ExperimentError::InvalidOutput(
                "digest is not bound to the design".into(),
            ));
        }
        Ok(())
    }
}

pub fn design_preclinical_experiment(
    request: &ExperimentRequest,
    arms: &[ExperimentArm],
) -> Result<ExperimentDesign, ExperimentError> {
    if request.objective.trim().is_empty()
        || arms.len() < 2
        || request.alpha_milli == 0
        || request.target_power_milli < 800
        || request.target_power_milli > 1_000
        || request.standardized_effect_milli == 0
        || request.variance_milli == 0
        || request.dropout_milli > 500
        || request.max_replicates_per_arm == 0
        || request.randomization_seed.as_str().len() != 64
        || z_alpha_milli(request.alpha_milli).is_none()
        || z_power_milli(request.target_power_milli).is_none()
    {
        return Err(ExperimentError::InvalidRequest(
            "objective, arm count, fixed-point power inputs, seed, or bounds are invalid".into(),
        ));
    }
    let mut arm_ids = BTreeSet::new();
    for arm in arms {
        if arm.arm_id.trim().is_empty()
            || arm.condition.trim().is_empty()
            || arm.model_system != request.model_system
            || !arm_ids.insert(arm.arm_id.clone())
        {
            return Err(ExperimentError::InvalidArm(
                "arm identity, condition, model binding, or uniqueness is invalid".into(),
            ));
        }
    }
    let required = required_replicates(request);
    let planned = required.min(request.max_replicates_per_arm);
    let mut allocations = Vec::with_capacity(arms.len());
    for arm in arms {
        let mut keyed = (0..planned)
            .map(|index| {
                let key = ContentHash::of_value(&serde_json::json!({"seed": request.randomization_seed, "arm": arm.arm_id, "index": index}))
                    .map_err(|e| ExperimentError::Digest(e.to_string()))?;
                Ok((key, format!("{}:{index:04}", arm.arm_id)))
            })
            .collect::<Result<Vec<_>, ExperimentError>>()?;
        keyed.sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));
        allocations.push(ArmAllocation {
            arm_id: arm.arm_id.clone(),
            planned_replicates: planned,
            allocation_order: keyed.into_iter().map(|(_, token)| token).collect(),
        });
    }
    allocations.sort_by(|left, right| left.arm_id.cmp(&right.arm_id));
    let blocked_order = if required > request.max_replicates_per_arm {
        vec!["required-replicates-exceed-configured-cap".into()]
    } else {
        Vec::new()
    };
    let disposition = if blocked_order.is_empty() {
        ExperimentDisposition::Ready
    } else {
        ExperimentDisposition::Underpowered
    };
    let achieved_power_milli = if disposition == ExperimentDisposition::Ready {
        request.target_power_milli
    } else {
        ((request.target_power_milli as u32 * planned as u32) / required.max(1) as u32) as u16
    };
    let mut assumptions = vec![
        "fixed-point normal approximation; provider must validate distributional assumptions"
            .into(),
        "replicates are independent within the declared preclinical model system".into(),
        "no individual clinical inference is permitted".into(),
    ];
    assumptions.extend(
        request
            .blocking_factors
            .iter()
            .map(|factor| format!("blocking-factor:{factor}")),
    );
    assumptions.sort();
    let mut design = ExperimentDesign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        outcome: request.outcome,
        required_replicates_per_arm: required,
        total_replicates: planned as u32 * arms.len() as u32,
        achieved_power_milli,
        allocations,
        blocking_factor_order: {
            let mut factors = request.blocking_factors.clone();
            factors.sort();
            factors
        },
        assumptions,
        acceptance_gate: "all planned replicates execute under the declared model, QC passes, and the null-result branch is released if the estimand is not met".into(),
        negative_result_plan: if request.release_null_result { "publish null and underpowered outcomes with the design digest and exclusions" } else { "release is blocked until the investigator enables explicit null-result publication" }.into(),
        blocked_order,
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({})).map_err(|e| ExperimentError::Digest(e.to_string()))?,
    };
    design.digest = ContentHash::of_value(&digest_input(&design))
        .map_err(|e| ExperimentError::Digest(e.to_string()))?;
    design.validate()?;
    Ok(design)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ExperimentRequest {
        ExperimentRequest {
            objective: "test invasion perturbation".into(),
            model_system: GliomaModelSystem::Organoid,
            outcome: OutcomeKind::Continuous,
            alpha_milli: 50,
            target_power_milli: 800,
            standardized_effect_milli: 500,
            variance_milli: 1_000,
            dropout_milli: 100,
            max_replicates_per_arm: 200,
            blocking_factors: vec!["batch".into()],
            randomization_seed: ContentHash::of_value(&serde_json::json!({"seed": "experiment"}))
                .unwrap(),
            release_null_result: true,
        }
    }

    fn arms() -> Vec<ExperimentArm> {
        vec![
            ExperimentArm {
                arm_id: "control".into(),
                model_system: GliomaModelSystem::Organoid,
                condition: "vehicle".into(),
            },
            ExperimentArm {
                arm_id: "perturbed".into(),
                model_system: GliomaModelSystem::Organoid,
                condition: "gene-knockdown".into(),
            },
        ]
    }

    #[test]
    fn design_is_power_aware_balanced_and_deterministic() {
        let first = design_preclinical_experiment(&request(), &arms()).unwrap();
        let second = design_preclinical_experiment(&request(), &arms()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, ExperimentDisposition::Ready);
        assert_eq!(
            first.allocations[0].planned_replicates,
            first.required_replicates_per_arm
        );
        assert_eq!(
            first.total_replicates,
            first.required_replicates_per_arm as u32 * 2
        );
        first.validate().unwrap();
    }

    #[test]
    fn cap_yields_explicit_underpowered_design_instead_of_false_readiness() {
        let mut request = request();
        request.max_replicates_per_arm = 2;
        let design = design_preclinical_experiment(&request, &arms()).unwrap();
        assert_eq!(design.disposition, ExperimentDisposition::Underpowered);
        assert_eq!(
            design.blocked_order,
            vec!["required-replicates-exceed-configured-cap"]
        );
        assert!(design.achieved_power_milli < request.target_power_milli);
    }
}
