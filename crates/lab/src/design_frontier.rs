//! Scenario frontier for power-aware preclinical experiment designs.
//!
//! Atlas feature: `AFA-lab-P09-F02`.
//!
//! Replays the existing deterministic design compiler under declared effect,
//! variance, attrition, and resource scenarios. A scenario that exceeds a
//! budget or violates a typed design gate remains a blocked result with its
//! reason; it is never dropped from the frontier.

use crate::experiment_design::{
    compile_experiment_design, ExperimentDesignPlan, ExperimentDesignRequest,
};
use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-lab-P09-F02";
pub const FEATURE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignScenario {
    pub scenario_id: String,
    pub effect_multiplier: f64,
    pub variance_multiplier: f64,
    pub attrition_fraction: Option<f64>,
    pub maximum_total_units: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignFrontierRequest {
    pub base: ExperimentDesignRequest,
    pub scenarios: Vec<DesignScenario>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioDisposition {
    Feasible,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignScenarioResult {
    pub scenario_id: String,
    pub disposition: ScenarioDisposition,
    pub request_digest: Option<ContentHash>,
    pub plan_digest: Option<ContentHash>,
    pub total_units: Option<u64>,
    pub minimum_projected_power: Option<f64>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignFrontierReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub study_id: String,
    pub feasible_scenarios: usize,
    pub blocked_scenarios: usize,
    pub scenarios: Vec<DesignScenarioResult>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl DesignFrontierReceipt {
    pub fn validate(&self) -> Result<(), DesignFrontierError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(DesignFrontierError::InvalidField(
                "schema, feature, or boundary".into(),
            ));
        }
        if self.study_id.trim().is_empty()
            || self.scenarios.is_empty()
            || self.feasible_scenarios + self.blocked_scenarios != self.scenarios.len()
            || self
                .scenarios
                .iter()
                .any(|scenario| scenario.reasons.is_empty())
        {
            return Err(DesignFrontierError::InvalidField(
                "frontier identity, counts, or reasons".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| DesignFrontierError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, DesignFrontierError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| DesignFrontierError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| DesignFrontierError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum DesignFrontierError {
    #[error("invalid design frontier field: {0}")]
    InvalidField(String),
    #[error("duplicate design scenario {0}")]
    DuplicateScenario(String),
    #[error("invalid design scenario measurement: {0}")]
    InvalidMeasurement(String),
    #[error("design compiler error: {0}")]
    Compiler(String),
    #[error("frontier artifact error: {0}")]
    Artifact(String),
    #[error("frontier serialization error: {0}")]
    Serialization(String),
}

pub fn design_frontier_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "lab".into(),
        consumers: ["preclinical neuroscientist".into(), "study statistician".into()].into(),
        behavior: "replays a typed power-aware experiment design across declared uncertainty and resource scenarios, retaining feasible and blocked frontier cells".into(),
        value: "makes design robustness and budget fragility visible before a protocol is approved".into(),
        inputs: vec![TypedPort { name: "design_frontier_request".into(), schema: "DesignFrontierRequest@1".into(), required: true }],
        outputs: vec![TypedPort { name: "design_frontier_receipt".into(), schema: "DesignFrontierReceipt@1".into(), required: true }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:local-design-assumptions".into(), "write:local-frontier-receipt".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "fixture:lab-design-frontier".into(), state: EvidenceState::Supported, locator: Some("fixtures/design-frontier".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "study statistician".into(), reason: "scenario assumptions are declared design inputs and cannot be silently inferred".into() }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn evaluate_design_frontier(
    request: &DesignFrontierRequest,
) -> Result<DesignFrontierReceipt, DesignFrontierError> {
    validate_request(request)?;
    let mut scenarios = request.scenarios.clone();
    scenarios.sort_by(|left, right| left.scenario_id.cmp(&right.scenario_id));
    let mut results = Vec::with_capacity(scenarios.len());
    for scenario in &scenarios {
        let mut design = request.base.clone();
        design.expected_effect *= scenario.effect_multiplier;
        design.outcome_variance *= scenario.variance_multiplier;
        if let Some(attrition) = scenario.attrition_fraction {
            design.attrition_fraction = attrition;
        }
        design.maximum_total_units = scenario.maximum_total_units;
        match compile_experiment_design(&design) {
            Ok(plan) => results.push(feasible_result(scenario, &plan)?),
            Err(error) => results.push(DesignScenarioResult {
                scenario_id: scenario.scenario_id.clone(),
                disposition: ScenarioDisposition::Blocked,
                request_digest: None,
                plan_digest: None,
                total_units: None,
                minimum_projected_power: None,
                reasons: vec![format!("scenario blocked: {error}")],
            }),
        }
    }
    let feasible_scenarios = results
        .iter()
        .filter(|result| result.disposition == ScenarioDisposition::Feasible)
        .count();
    let blocked_scenarios = results.len() - feasible_scenarios;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "study_id": request.base.study_id,
        "feasible_scenarios": feasible_scenarios,
        "blocked_scenarios": blocked_scenarios,
        "scenarios": results,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("design-frontier:{}", request.base.study_id),
        "application/vnd.aurora.design-frontier+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| DesignFrontierError::Artifact(error.to_string()))?;
    let receipt = DesignFrontierReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        study_id: request.base.study_id.clone(),
        feasible_scenarios,
        blocked_scenarios,
        scenarios: serde_json::from_value(payload["scenarios"].clone())
            .map_err(|error| DesignFrontierError::Serialization(error.to_string()))?,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn feasible_result(
    scenario: &DesignScenario,
    plan: &ExperimentDesignPlan,
) -> Result<DesignScenarioResult, DesignFrontierError> {
    let plan_digest = plan
        .digest()
        .map_err(|error| DesignFrontierError::Compiler(error.to_string()))?;
    let request_digest = plan.payload.request_digest.clone();
    let minimum_projected_power = plan
        .payload
        .projections
        .iter()
        .map(|projection| projection.projected_power)
        .min_by(f64::total_cmp);
    Ok(DesignScenarioResult {
        scenario_id: scenario.scenario_id.clone(),
        disposition: ScenarioDisposition::Feasible,
        request_digest: Some(request_digest),
        plan_digest: Some(plan_digest),
        total_units: Some(plan.payload.total_units),
        minimum_projected_power,
        reasons: vec!["scenario compiled within its declared resource envelope".into()],
    })
}

fn validate_request(request: &DesignFrontierRequest) -> Result<(), DesignFrontierError> {
    request
        .base
        .validate()
        .map_err(|error| DesignFrontierError::Compiler(error.to_string()))?;
    if request.scenarios.is_empty() {
        return Err(DesignFrontierError::InvalidField(
            "at least one scenario is required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for scenario in &request.scenarios {
        if scenario.scenario_id.trim().is_empty() || !ids.insert(scenario.scenario_id.clone()) {
            return Err(DesignFrontierError::DuplicateScenario(
                scenario.scenario_id.clone(),
            ));
        }
        for (name, value) in [
            ("effect_multiplier", scenario.effect_multiplier),
            ("variance_multiplier", scenario.variance_multiplier),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(DesignFrontierError::InvalidMeasurement(name.into()));
            }
        }
        if scenario
            .attrition_fraction
            .is_some_and(|value| !value.is_finite() || !(0.0..1.0).contains(&value))
        {
            return Err(DesignFrontierError::InvalidMeasurement(
                "attrition_fraction".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiment_design::{
        ArmKind, BlindingScheme, DesignArm, NullResultPolicy, RandomizationScheme, StudyPopulation,
        TestTail,
    };

    fn request() -> DesignFrontierRequest {
        DesignFrontierRequest {
            base: ExperimentDesignRequest {
                study_id: "study:frontier".into(),
                population: StudyPopulation::Organoid,
                outcome: "neurite length".into(),
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
                expected_effect: 1.0,
                outcome_variance: 1.0,
                alpha: 0.05,
                target_power: 0.8,
                attrition_fraction: 0.1,
                tail: TestTail::TwoSided,
                randomization: RandomizationScheme::Block,
                blinding: BlindingScheme::Assessor,
                null_result_policy: NullResultPolicy::PublishWithUncertainty,
                maximum_total_units: Some(200),
            },
            scenarios: vec![
                DesignScenario {
                    scenario_id: "nominal".into(),
                    effect_multiplier: 1.0,
                    variance_multiplier: 1.0,
                    attrition_fraction: None,
                    maximum_total_units: Some(200),
                },
                DesignScenario {
                    scenario_id: "underpowered".into(),
                    effect_multiplier: 0.2,
                    variance_multiplier: 2.0,
                    attrition_fraction: Some(0.3),
                    maximum_total_units: Some(20),
                },
            ],
        }
    }

    #[test]
    fn frontier_retains_feasible_and_blocked_scenarios() {
        let receipt = evaluate_design_frontier(&request()).unwrap();
        assert_eq!(receipt.scenarios.len(), 2);
        assert_eq!(receipt.feasible_scenarios, 1);
        assert_eq!(receipt.blocked_scenarios, 1);
    }

    #[test]
    fn scenario_order_does_not_change_digest() {
        let left = evaluate_design_frontier(&request()).unwrap();
        let mut reversed = request();
        reversed.scenarios.reverse();
        let right = evaluate_design_frontier(&reversed).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
    }
}
