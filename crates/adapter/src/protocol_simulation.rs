//! Prospective high-throughput protocol-state simulation for adapter plans.
//!
//! Atlas feature: `AFA-adapter-P10-F03`.
//!
//! This adapter-side simulator consumes a typed protocol draft and produces a replayable state
//! machine report before any instrument gateway is contacted. It models budget exhaustion,
//! failure compensation, network partitions, preflight ordering, and approval boundaries. It is
//! deliberately metadata-only: physical execution belongs to the A3 instrument gateway.

use bioprism_foundation::{
    LossSeverity, ProvenanceLink, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P10-F03";
pub const CONTRACT_VERSION: &str = "prospective-protocol-simulation/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolOperation {
    LocalComputation,
    InstrumentPreflight,
    InstrumentExecution,
    WriteArtifact,
    FederationExport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtocolStep {
    pub step_id: String,
    pub operation: ProtocolOperation,
    pub cost_units: f64,
    pub idempotent: bool,
    pub compensation_step: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtocolScenario {
    pub scenario_id: String,
    pub fail_at_step: Option<String>,
    pub network_partition: bool,
    pub budget_multiplier: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtocolDraft {
    pub protocol_id: String,
    pub design_digest: ContentHash,
    pub steps: Vec<ProtocolStep>,
    pub scenarios: Vec<ProtocolScenario>,
    pub max_retries: u32,
    pub resource_budget: f64,
    pub approval_required_for_effects: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolSimulationState {
    Passed,
    FailedClosed,
    ApprovalRequired,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtocolScenarioResult {
    pub scenario_id: String,
    pub state: ProtocolSimulationState,
    pub executed_steps: Vec<String>,
    pub retries: u32,
    pub compensations: Vec<String>,
    pub consumed_units: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtocolSimulationReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub protocol_id: String,
    pub design_digest: ContentHash,
    pub results: Vec<ProtocolScenarioResult>,
    pub passed: usize,
    pub failed_closed: usize,
    pub approval_required: usize,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl ProtocolSimulationReceipt {
    pub fn validate(&self) -> Result<(), ProtocolSimulationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(ProtocolSimulationError::Contract(
                "protocol simulation contract identity mismatch".into(),
            ));
        }
        if self.protocol_id.trim().is_empty()
            || self.results.is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
        {
            return Err(ProtocolSimulationError::InvalidField(
                "protocol identity, results, boundary, and locality are required".into(),
            ));
        }
        if self.passed + self.failed_closed + self.approval_required != self.results.len() {
            return Err(ProtocolSimulationError::InvalidField(
                "scenario state counts do not match results".into(),
            ));
        }
        if self
            .results
            .windows(2)
            .any(|pair| pair[0].scenario_id >= pair[1].scenario_id)
        {
            return Err(ProtocolSimulationError::InvalidField(
                "scenario results must be sorted by scenario id".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ProtocolSimulationError::Contract(error.to_string()))?;
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ProtocolSimulationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ProtocolSimulationError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ProtocolSimulationError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ProtocolSimulationError {
    #[error("invalid protocol simulation field: {0}")]
    InvalidField(String),
    #[error("duplicate protocol step {0}")]
    DuplicateStep(String),
    #[error("duplicate protocol scenario {0}")]
    DuplicateScenario(String),
    #[error("unknown protocol step {0}")]
    UnknownStep(String),
    #[error("instrument execution step {0} has no earlier preflight")]
    MissingPreflight(String),
    #[error("protocol simulation contract rejected: {0}")]
    Contract(String),
    #[error("protocol simulation serialization failed: {0}")]
    Serialization(String),
}

pub fn simulate_protocol_draft(
    draft: &ProtocolDraft,
) -> Result<ProtocolSimulationReceipt, ProtocolSimulationError> {
    validate_draft(draft)?;
    let mut scenarios = draft.scenarios.clone();
    scenarios.sort_by(|left, right| left.scenario_id.cmp(&right.scenario_id));
    let results = scenarios
        .iter()
        .map(|scenario| simulate_scenario(draft, scenario))
        .collect::<Vec<_>>();
    let passed = results
        .iter()
        .filter(|result| result.state == ProtocolSimulationState::Passed)
        .count();
    let failed_closed = results
        .iter()
        .filter(|result| result.state == ProtocolSimulationState::FailedClosed)
        .count();
    let approval_required = results
        .iter()
        .filter(|result| result.state == ProtocolSimulationState::ApprovalRequired)
        .count();
    let omissions = results
        .iter()
        .filter(|result| result.state != ProtocolSimulationState::Passed)
        .map(|result| format!("scenario {} did not complete as passed", result.scenario_id))
        .collect::<Vec<_>>();
    let uncertainty = if failed_closed > 0 {
        vec!["failure scenarios establish no positive claim about untested executions".into()]
    } else {
        Vec::new()
    };
    let semantic_loss = if draft
        .steps
        .iter()
        .any(|step| step.operation == ProtocolOperation::FederationExport)
    {
        vec![SemanticLoss {
            field: "federation_export".into(),
            reason: "simulation records export policy but never observes remote institution state"
                .into(),
            severity: LossSeverity::Bounded,
        }]
    } else {
        Vec::new()
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "protocol_id": draft.protocol_id,
        "design_digest": draft.design_digest,
        "results": results,
        "passed": passed,
        "failed_closed": failed_closed,
        "approval_required": approval_required,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "semantic_loss": semantic_loss,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("protocol-simulation:{}", draft.protocol_id),
        "application/vnd.aurora.prospective-protocol-simulation+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: draft.protocol_id.clone(),
            relation: "simulated-from-local-protocol-draft".into(),
            digest: draft.design_digest.clone(),
        }],
    )
    .map_err(|error| ProtocolSimulationError::Contract(error.to_string()))?;
    let receipt = ProtocolSimulationReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        protocol_id: draft.protocol_id.clone(),
        design_digest: draft.design_digest.clone(),
        results,
        passed,
        failed_closed,
        approval_required,
        omissions,
        uncertainty,
        semantic_loss,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn simulate_scenario(draft: &ProtocolDraft, scenario: &ProtocolScenario) -> ProtocolScenarioResult {
    let mut consumed_units = 0.0;
    let budget = draft.resource_budget * scenario.budget_multiplier;
    let mut executed_steps = Vec::new();
    let mut compensations = Vec::new();
    let mut reasons = Vec::new();
    for step in &draft.steps {
        if consumed_units + step.cost_units > budget {
            reasons.push(format!("resource budget exhausted before {}", step.step_id));
            return ProtocolScenarioResult {
                scenario_id: scenario.scenario_id.clone(),
                state: ProtocolSimulationState::FailedClosed,
                executed_steps,
                retries: 0,
                compensations,
                consumed_units,
                reasons,
            };
        }
        if scenario.network_partition && step.operation == ProtocolOperation::FederationExport {
            reasons.push(
                "network partition blocks federation export; local continuation requires approval"
                    .into(),
            );
            return ProtocolScenarioResult {
                scenario_id: scenario.scenario_id.clone(),
                state: ProtocolSimulationState::ApprovalRequired,
                executed_steps,
                retries: 0,
                compensations,
                consumed_units,
                reasons,
            };
        }
        if draft.approval_required_for_effects
            && matches!(
                step.operation,
                ProtocolOperation::InstrumentExecution | ProtocolOperation::FederationExport
            )
        {
            reasons.push(format!(
                "{} requires an external effect approval",
                step.step_id
            ));
            return ProtocolScenarioResult {
                scenario_id: scenario.scenario_id.clone(),
                state: ProtocolSimulationState::ApprovalRequired,
                executed_steps,
                retries: 0,
                compensations,
                consumed_units,
                reasons,
            };
        }
        consumed_units += step.cost_units;
        if scenario.fail_at_step.as_deref() == Some(step.step_id.as_str()) {
            reasons.push(format!("scenario failure injected at {}", step.step_id));
            if let Some(compensation) = &step.compensation_step {
                if draft
                    .steps
                    .iter()
                    .any(|candidate| candidate.step_id == *compensation)
                {
                    compensations.push(compensation.clone());
                    reasons.push(format!("compensation {} recorded", compensation));
                }
            }
            return ProtocolScenarioResult {
                scenario_id: scenario.scenario_id.clone(),
                state: ProtocolSimulationState::FailedClosed,
                executed_steps,
                retries: 0,
                compensations,
                consumed_units,
                reasons,
            };
        }
        executed_steps.push(step.step_id.clone());
    }
    reasons.push("all simulated protocol steps completed within declared budget".into());
    ProtocolScenarioResult {
        scenario_id: scenario.scenario_id.clone(),
        state: ProtocolSimulationState::Passed,
        executed_steps,
        retries: 0,
        compensations,
        consumed_units,
        reasons,
    }
}

fn validate_draft(draft: &ProtocolDraft) -> Result<(), ProtocolSimulationError> {
    if draft.protocol_id.trim().is_empty()
        || draft.steps.is_empty()
        || draft.scenarios.is_empty()
        || !draft.raw_data_local
        || draft.boundary != PRECLINICAL_BOUNDARY
        || !draft.resource_budget.is_finite()
        || draft.resource_budget <= 0.0
    {
        return Err(ProtocolSimulationError::InvalidField(
            "protocol, steps, scenarios, positive budget, locality, and boundary are required"
                .into(),
        ));
    }
    let mut step_ids = BTreeSet::new();
    let mut preflight_seen = false;
    for step in &draft.steps {
        if step.step_id.trim().is_empty() || !step_ids.insert(step.step_id.clone()) {
            return Err(ProtocolSimulationError::DuplicateStep(step.step_id.clone()));
        }
        if !step.cost_units.is_finite() || step.cost_units <= 0.0 {
            return Err(ProtocolSimulationError::InvalidField(format!(
                "cost_units:{}",
                step.step_id
            )));
        }
        if step.operation == ProtocolOperation::InstrumentPreflight {
            preflight_seen = true;
        }
        if step.operation == ProtocolOperation::InstrumentExecution && !preflight_seen {
            return Err(ProtocolSimulationError::MissingPreflight(
                step.step_id.clone(),
            ));
        }
    }
    for step in &draft.steps {
        if let Some(compensation) = &step.compensation_step {
            if !step_ids.contains(compensation) {
                return Err(ProtocolSimulationError::UnknownStep(compensation.clone()));
            }
        }
    }
    let mut scenario_ids = BTreeSet::new();
    for scenario in &draft.scenarios {
        if scenario.scenario_id.trim().is_empty()
            || !scenario_ids.insert(scenario.scenario_id.clone())
        {
            return Err(ProtocolSimulationError::DuplicateScenario(
                scenario.scenario_id.clone(),
            ));
        }
        if !scenario.budget_multiplier.is_finite() || scenario.budget_multiplier <= 0.0 {
            return Err(ProtocolSimulationError::InvalidField(format!(
                "budget_multiplier:{}",
                scenario.scenario_id
            )));
        }
        if let Some(step_id) = &scenario.fail_at_step {
            if !step_ids.contains(step_id) {
                return Err(ProtocolSimulationError::UnknownStep(step_id.clone()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> ProtocolDraft {
        ProtocolDraft {
            protocol_id: "protocol:organoid".into(),
            design_digest: ContentHash::of_bytes(b"design"),
            steps: vec![
                ProtocolStep {
                    step_id: "preflight".into(),
                    operation: ProtocolOperation::InstrumentPreflight,
                    cost_units: 1.0,
                    idempotent: true,
                    compensation_step: None,
                },
                ProtocolStep {
                    step_id: "image".into(),
                    operation: ProtocolOperation::InstrumentExecution,
                    cost_units: 2.0,
                    idempotent: false,
                    compensation_step: Some("cleanup".into()),
                },
                ProtocolStep {
                    step_id: "cleanup".into(),
                    operation: ProtocolOperation::WriteArtifact,
                    cost_units: 1.0,
                    idempotent: true,
                    compensation_step: None,
                },
            ],
            scenarios: vec![
                ProtocolScenario {
                    scenario_id: "scenario:nominal".into(),
                    fail_at_step: None,
                    network_partition: false,
                    budget_multiplier: 1.0,
                },
                ProtocolScenario {
                    scenario_id: "scenario:failure".into(),
                    fail_at_step: Some("image".into()),
                    network_partition: false,
                    budget_multiplier: 1.0,
                },
            ],
            max_retries: 1,
            resource_budget: 10.0,
            approval_required_for_effects: false,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn simulation_is_sorted_and_replayable() {
        let mut reversed = draft();
        reversed.scenarios.reverse();
        let left = simulate_protocol_draft(&draft()).unwrap();
        let right = simulate_protocol_draft(&reversed).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
        assert_eq!(left.passed, 1);
        assert_eq!(left.failed_closed, 1);
    }

    #[test]
    fn instrument_execution_without_preflight_is_rejected() {
        let mut draft = draft();
        draft.steps.remove(0);
        assert!(matches!(
            simulate_protocol_draft(&draft).unwrap_err(),
            ProtocolSimulationError::MissingPreflight(_)
        ));
    }

    #[test]
    fn partitioned_export_requires_approval() {
        let mut draft = draft();
        draft.steps.push(ProtocolStep {
            step_id: "export".into(),
            operation: ProtocolOperation::FederationExport,
            cost_units: 1.0,
            idempotent: true,
            compensation_step: None,
        });
        draft.scenarios = vec![ProtocolScenario {
            scenario_id: "scenario:partition".into(),
            fail_at_step: None,
            network_partition: true,
            budget_multiplier: 1.0,
        }];
        let receipt = simulate_protocol_draft(&draft).unwrap();
        assert_eq!(receipt.approval_required, 1);
    }

    #[test]
    fn budget_exhaustion_fails_closed() {
        let mut draft = draft();
        draft.resource_budget = 2.0;
        let receipt = simulate_protocol_draft(&draft).unwrap();
        assert!(receipt.failed_closed >= 1);
        assert!(!receipt.uncertainty.is_empty());
    }
}
