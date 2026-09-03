//! Power-aware, boundary-safe preclinical experiment design.
//!
//! Atlas feature: `AFA-lab-P09-F01`.
//!
//! This module compiles a declared preclinical research design into a
//! deterministic sample allocation, power curve, and acceptance gates. It does
//! not claim that a design will produce a scientific truth; it makes the
//! assumptions, null-result handling, and resource limits executable and
//! replayable before a laboratory or computational workflow is authorized.

use bioprism_foundation::{
    LossSeverity, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::{to_canonical_bytes, ContentHash};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-lab-P09-F01";
pub const FEATURE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudyPopulation {
    CellSystem,
    Organoid,
    AnimalModel,
    InSilico,
    ExVivo,
    MultimodalPreclinical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmKind {
    Control,
    Treatment,
    Comparator,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignArm {
    pub arm_id: String,
    pub kind: ArmKind,
    pub allocation_weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestTail {
    OneSided,
    TwoSided,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RandomizationScheme {
    Simple,
    Block,
    Stratified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlindingScheme {
    None,
    Assessor,
    Double,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NullResultPolicy {
    PublishWithUncertainty,
    EscalateForReplication,
    BlockInterpretation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentDesignRequest {
    pub study_id: String,
    pub population: StudyPopulation,
    pub outcome: String,
    pub arms: Vec<DesignArm>,
    /// Absolute expected difference on the outcome scale.
    pub expected_effect: f64,
    /// Outcome variance declared from a prior or pilot source.
    pub outcome_variance: f64,
    pub alpha: f64,
    pub target_power: f64,
    pub attrition_fraction: f64,
    pub tail: TestTail,
    pub randomization: RandomizationScheme,
    pub blinding: BlindingScheme,
    pub null_result_policy: NullResultPolicy,
    pub maximum_total_units: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleAllocation {
    pub arm_id: String,
    pub units: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerProjection {
    pub effect_multiplier: f64,
    pub projected_effect: f64,
    pub projected_power: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceGate {
    pub gate_id: String,
    pub criterion: String,
    pub required: String,
    pub falsifier: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignPlanPayload {
    pub schema_version: String,
    pub feature_id: String,
    pub request_digest: ContentHash,
    pub study_id: String,
    pub allocations: Vec<SampleAllocation>,
    pub total_units: u64,
    pub z_alpha: f64,
    pub z_power: f64,
    pub projections: Vec<PowerProjection>,
    pub acceptance_gates: Vec<AcceptanceGate>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentDesignPlan {
    pub payload: DesignPlanPayload,
    pub artifact: TypedResearchArtifact,
}

#[derive(Debug, Error)]
pub enum ExperimentDesignError {
    #[error("design field `{field}` is missing or invalid")]
    InvalidField { field: String },
    #[error("design requires at least one control and one non-control arm")]
    MissingControl,
    #[error("design arm id `{arm_id}` is duplicated")]
    DuplicateArm { arm_id: String },
    #[error("declared total of {total} units exceeds the maximum of {maximum}")]
    ResourceLimit { total: u64, maximum: u64 },
    #[error("the declared design is outside the preclinical boundary")]
    BoundaryViolation,
    #[error("design artifact could not be serialized: {0}")]
    Serialization(String),
    #[error("design artifact verification failed: {0}")]
    Artifact(String),
}

impl ExperimentDesignRequest {
    pub fn validate(&self) -> Result<(), ExperimentDesignError> {
        if self.study_id.trim().is_empty() {
            return Err(ExperimentDesignError::InvalidField {
                field: "study_id".into(),
            });
        }
        if self.outcome.trim().is_empty() {
            return Err(ExperimentDesignError::InvalidField {
                field: "outcome".into(),
            });
        }
        if self.arms.len() < 2 {
            return Err(ExperimentDesignError::InvalidField {
                field: "arms (at least two)".into(),
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut has_control = false;
        let mut has_non_control = false;
        for arm in &self.arms {
            if arm.arm_id.trim().is_empty() || !seen.insert(arm.arm_id.clone()) {
                return Err(ExperimentDesignError::DuplicateArm {
                    arm_id: arm.arm_id.clone(),
                });
            }
            if !arm.allocation_weight.is_finite() || arm.allocation_weight <= 0.0 {
                return Err(ExperimentDesignError::InvalidField {
                    field: format!("allocation_weight:{}", arm.arm_id),
                });
            }
            match arm.kind {
                ArmKind::Control => has_control = true,
                ArmKind::Treatment | ArmKind::Comparator => has_non_control = true,
            }
        }
        if !has_control || !has_non_control {
            return Err(ExperimentDesignError::MissingControl);
        }
        for (field, value) in [
            ("expected_effect", self.expected_effect),
            ("outcome_variance", self.outcome_variance),
            ("alpha", self.alpha),
            ("target_power", self.target_power),
            ("attrition_fraction", self.attrition_fraction),
        ] {
            if !value.is_finite() {
                return Err(ExperimentDesignError::InvalidField {
                    field: field.into(),
                });
            }
        }
        if self.expected_effect <= 0.0 || self.outcome_variance <= 0.0 {
            return Err(ExperimentDesignError::InvalidField {
                field: "effect and variance must be positive".into(),
            });
        }
        if !(0.0 < self.alpha && self.alpha < 1.0) {
            return Err(ExperimentDesignError::InvalidField {
                field: "alpha".into(),
            });
        }
        if !(0.5 < self.target_power && self.target_power < 1.0) {
            return Err(ExperimentDesignError::InvalidField {
                field: "target_power".into(),
            });
        }
        if !(0.0 <= self.attrition_fraction && self.attrition_fraction < 1.0) {
            return Err(ExperimentDesignError::InvalidField {
                field: "attrition_fraction".into(),
            });
        }
        Ok(())
    }
}

impl ExperimentDesignPlan {
    pub fn validate(&self) -> Result<(), ExperimentDesignError> {
        if self.payload.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.payload.feature_id != FEATURE_ID
            || self.payload.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(ExperimentDesignError::BoundaryViolation);
        }
        if self.payload.allocations.is_empty()
            || self.payload.total_units
                != self
                    .payload
                    .allocations
                    .iter()
                    .map(|allocation| allocation.units)
                    .sum::<u64>()
        {
            return Err(ExperimentDesignError::InvalidField {
                field: "allocations".into(),
            });
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ExperimentDesignError::Artifact(error.to_string()))
    }

    pub fn verify_artifact(&self) -> Result<(), ExperimentDesignError> {
        let value = serde_json::to_value(&self.payload)
            .map_err(|error| ExperimentDesignError::Serialization(error.to_string()))?;
        self.artifact
            .verify_payload(&value)
            .map_err(|error| ExperimentDesignError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ExperimentDesignError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ExperimentDesignError::Serialization(error.to_string()))?;
        let bytes = to_canonical_bytes(&value)
            .map_err(|error| ExperimentDesignError::Serialization(error.to_string()))?;
        Ok(ContentHash::of_bytes(&bytes))
    }
}

pub fn compile_experiment_design(
    request: &ExperimentDesignRequest,
) -> Result<ExperimentDesignPlan, ExperimentDesignError> {
    request.validate()?;
    let request_value = serde_json::to_value(request)
        .map_err(|error| ExperimentDesignError::Serialization(error.to_string()))?;
    let request_digest = ContentHash::of_value(&request_value)
        .map_err(|error| ExperimentDesignError::Serialization(error.to_string()))?;
    let z_alpha = inverse_normal_cdf(match request.tail {
        TestTail::TwoSided => 1.0 - request.alpha / 2.0,
        TestTail::OneSided => 1.0 - request.alpha,
    })?;
    let z_power = inverse_normal_cdf(request.target_power)?;
    let raw_per_arm = 2.0 * (z_alpha + z_power).powi(2) * request.outcome_variance
        / request.expected_effect.powi(2)
        / (1.0 - request.attrition_fraction);
    let base_per_arm = raw_per_arm.ceil().max(2.0) as u64;
    let total_weight: f64 = request.arms.iter().map(|arm| arm.allocation_weight).sum();
    let target_total = (base_per_arm as f64 * total_weight).ceil() as u64;
    let mut allocations: Vec<SampleAllocation> = request
        .arms
        .iter()
        .map(|arm| SampleAllocation {
            arm_id: arm.arm_id.clone(),
            units: ((target_total as f64 * arm.allocation_weight / total_weight).round() as u64)
                .max(2),
        })
        .collect();
    rebalance_allocations(&mut allocations, target_total);
    let total_units = allocations.iter().map(|allocation| allocation.units).sum();
    if let Some(maximum) = request.maximum_total_units {
        if total_units > maximum {
            return Err(ExperimentDesignError::ResourceLimit {
                total: total_units,
                maximum,
            });
        }
    }
    let projections = [0.5, 0.75, 1.0, 1.25]
        .into_iter()
        .map(|multiplier| PowerProjection {
            effect_multiplier: multiplier,
            projected_effect: request.expected_effect * multiplier,
            projected_power: projected_power(
                total_units,
                request.expected_effect * multiplier,
                request.outcome_variance,
                z_alpha,
                request.tail,
            ),
        })
        .collect();
    let gates = vec![
        AcceptanceGate {
            gate_id: "power-target".into(),
            criterion: "projected power at declared effect".into(),
            required: format!(">= {:.3}", request.target_power),
            falsifier: "pilot variance or effect estimate moves the target below threshold".into(),
        },
        AcceptanceGate {
            gate_id: "null-result".into(),
            criterion: "predeclare handling of a null or negative result".into(),
            required: format!("{:?}", request.null_result_policy),
            falsifier: "analysis reports a confident mechanism despite a null result".into(),
        },
        AcceptanceGate {
            gate_id: "attrition-budget".into(),
            criterion: "planned units remain within the declared resource envelope".into(),
            required: request
                .maximum_total_units
                .map(|maximum| format!("<= {maximum}"))
                .unwrap_or_else(|| "declared maximum not supplied".into()),
            falsifier: "execution exceeds the planned allocation without a signed amendment".into(),
        },
    ];
    let payload = DesignPlanPayload {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_digest,
        study_id: request.study_id.clone(),
        allocations,
        total_units,
        z_alpha,
        z_power,
        projections,
        acceptance_gates: gates,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload_value = serde_json::to_value(&payload)
        .map_err(|error| ExperimentDesignError::Serialization(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("experiment-design:{}", request.study_id),
        "application/vnd.aurora.experiment-design+json",
        &payload_value,
        vec![bioprism_foundation::SemanticLoss {
            field: "inference".into(),
            reason: "design is a bounded projection from declared assumptions, not a scientific conclusion".into(),
            severity: LossSeverity::Bounded,
        }],
        vec![],
    )
    .map_err(|error| ExperimentDesignError::Artifact(error.to_string()))?;
    let plan = ExperimentDesignPlan { payload, artifact };
    plan.validate()?;
    plan.verify_artifact()?;
    Ok(plan)
}

fn rebalance_allocations(allocations: &mut [SampleAllocation], target_total: u64) {
    let mut current: i64 = allocations
        .iter()
        .map(|allocation| allocation.units as i64)
        .sum();
    let target = target_total as i64;
    let mut index = 0;
    while current < target {
        allocations[index % allocations.len()].units += 1;
        current += 1;
        index += 1;
    }
    let mut index = 0;
    while current > target {
        let slot = &mut allocations[index % allocations.len()];
        if slot.units > 2 {
            slot.units -= 1;
            current -= 1;
        }
        index += 1;
    }
}

fn projected_power(
    total_units: u64,
    effect: f64,
    variance: f64,
    z_alpha: f64,
    tail: TestTail,
) -> f64 {
    let noncentral = effect * (total_units as f64 / (2.0 * variance)).sqrt();
    let power = match tail {
        TestTail::OneSided => normal_cdf(noncentral - z_alpha),
        TestTail::TwoSided => {
            1.0 - normal_cdf(z_alpha - noncentral) + normal_cdf(-z_alpha - noncentral)
        }
    };
    power.clamp(0.0, 1.0)
}

// Acklam's rational approximation to the inverse standard normal CDF.
fn inverse_normal_cdf(p: f64) -> Result<f64, ExperimentDesignError> {
    if !(0.0 < p && p < 1.0) {
        return Err(ExperimentDesignError::InvalidField {
            field: "normal quantile probability".into(),
        });
    }
    const A: [f64; 6] = [
        -3.969683028665376e1,
        2.209460984245205e2,
        -2.759285104469687e2,
        1.38357751867269e2,
        -3.066479806614716e1,
        2.506628277459239,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e1,
        1.615858368580409e2,
        -1.556989798598866e2,
        6.680131188771972e1,
        -1.328068155288572e1,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-3,
        -3.223964580411365e-1,
        -2.400758277161838,
        -2.549732539343734,
        4.374664141464968,
        2.938163982698783,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-3,
        3.224671290700398e-1,
        2.445134137142996,
        3.754408661907416,
    ];
    let plow = 0.02425;
    let phigh = 1.0 - plow;
    if p < plow {
        let q = (-2.0 * p.ln()).sqrt();
        let numerator = ((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5];
        let denominator = (((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0;
        return Ok(numerator / denominator);
    }
    if p > phigh {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        let numerator = ((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5];
        let denominator = (((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0;
        return Ok(-numerator / denominator);
    }
    let q = p - 0.5;
    let r = q * q;
    let numerator = (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q;
    let denominator = ((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0;
    Ok(numerator / denominator)
}

fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf_approx(x / 2.0_f64.sqrt()))
}

fn erf_approx(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();
    sign * y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ExperimentDesignRequest {
        ExperimentDesignRequest {
            study_id: "organoid-dose-response-01".into(),
            population: StudyPopulation::Organoid,
            outcome: "growth_inhibition".into(),
            arms: vec![
                DesignArm {
                    arm_id: "control".into(),
                    kind: ArmKind::Control,
                    allocation_weight: 1.0,
                },
                DesignArm {
                    arm_id: "treatment".into(),
                    kind: ArmKind::Treatment,
                    allocation_weight: 1.0,
                },
            ],
            expected_effect: 0.8,
            outcome_variance: 1.0,
            alpha: 0.05,
            target_power: 0.8,
            attrition_fraction: 0.1,
            tail: TestTail::TwoSided,
            randomization: RandomizationScheme::Block,
            blinding: BlindingScheme::Assessor,
            null_result_policy: NullResultPolicy::EscalateForReplication,
            maximum_total_units: Some(200),
        }
    }

    #[test]
    fn compile_design_emits_power_curve_and_typed_artifact() {
        let plan = compile_experiment_design(&request()).unwrap();
        assert!(plan.payload.total_units >= 2);
        assert_eq!(plan.payload.projections.len(), 4);
        assert!(plan.payload.projections[2].projected_power >= 0.79);
        plan.verify_artifact().unwrap();
        plan.digest().unwrap();
    }

    #[test]
    fn design_is_deterministic_for_identical_inputs() {
        let left = compile_experiment_design(&request()).unwrap();
        let right = compile_experiment_design(&request()).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
        assert_eq!(left.payload, right.payload);
    }

    #[test]
    fn resource_limit_and_boundary_gates_fail_closed() {
        let mut limited = request();
        limited.maximum_total_units = Some(2);
        assert!(matches!(
            compile_experiment_design(&limited),
            Err(ExperimentDesignError::ResourceLimit { .. })
        ));
        let mut invalid = request();
        invalid.arms[0].kind = ArmKind::Treatment;
        assert!(matches!(
            compile_experiment_design(&invalid),
            Err(ExperimentDesignError::MissingControl)
        ));
    }

    #[test]
    fn unknown_boundary_is_not_representable_as_a_preclinical_design() {
        let mut invalid = request();
        invalid.study_id.clear();
        assert!(compile_experiment_design(&invalid).is_err());
    }
}
