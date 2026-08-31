//! Deterministic protocol simulation and preflight failure analysis.
//!
//! Atlas feature: `AFA-lab-P10-F01`.
//!
//! The simulator is intentionally not an instrument driver. It runs a typed
//! protocol against declared failure scenarios so an operator can inspect
//! retries, partitions, budget termination, interlock requirements, and
//! compensations before a physical execution grant is requested.

use bioprism_foundation::{
    Effect, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::{to_canonical_bytes, ContentHash};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-lab-P10-F01";
pub const FEATURE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolOperation {
    LocalComputation,
    InstrumentPreflight,
    InstrumentExecution,
    WriteArtifact,
    FederationExport,
}

impl ProtocolOperation {
    fn effect(self) -> Effect {
        match self {
            Self::LocalComputation => Effect::ExecuteLocalComputation,
            Self::InstrumentPreflight | Self::InstrumentExecution => Effect::InstrumentExecution,
            Self::WriteArtifact => Effect::WriteLocalArtifact,
            Self::FederationExport => Effect::FederationExport,
        }
    }
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
pub struct ProtocolSimulationRequest {
    pub protocol_id: String,
    pub design_digest: ContentHash,
    pub steps: Vec<ProtocolStep>,
    pub scenarios: Vec<ProtocolScenario>,
    pub max_retries: u32,
    pub resource_budget: f64,
    pub allow_compensation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioStatus {
    Passed,
    FailedClosed,
    RequiresApproval,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub scenario_id: String,
    pub status: ScenarioStatus,
    pub executed_steps: Vec<String>,
    pub retries: u32,
    pub compensations: Vec<String>,
    pub consumed_units: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtocolSimulationPayload {
    pub schema_version: String,
    pub feature_id: String,
    pub protocol_id: String,
    pub request_digest: ContentHash,
    pub results: Vec<ScenarioResult>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtocolSimulationReport {
    pub payload: ProtocolSimulationPayload,
    pub artifact: TypedResearchArtifact,
}

#[derive(Debug, Error)]
pub enum ProtocolSimulationError {
    #[error("protocol field `{field}` is missing or invalid")]
    InvalidField { field: String },
    #[error("protocol step `{step_id}` is duplicated")]
    DuplicateStep { step_id: String },
    #[error("protocol scenario `{scenario_id}` is duplicated")]
    DuplicateScenario { scenario_id: String },
    #[error("protocol step `{step_id}` is referenced but not defined")]
    UnknownStep { step_id: String },
    #[error("instrument execution step `{step_id}` has no earlier preflight")]
    MissingPreflight { step_id: String },
    #[error("protocol artifact error: {0}")]
    Artifact(String),
    #[error("protocol serialization error: {0}")]
    Serialization(String),
}

impl ProtocolSimulationRequest {
    pub fn validate(&self) -> Result<(), ProtocolSimulationError> {
        if self.protocol_id.trim().is_empty() || self.steps.is_empty() || self.scenarios.is_empty()
        {
            return Err(ProtocolSimulationError::InvalidField {
                field: "protocol_id, steps, and scenarios".into(),
            });
        }
        if !self.resource_budget.is_finite() || self.resource_budget <= 0.0 {
            return Err(ProtocolSimulationError::InvalidField {
                field: "resource_budget".into(),
            });
        }
        let mut step_ids = std::collections::BTreeSet::new();
        let mut preflight_seen = false;
        for step in &self.steps {
            if step.step_id.trim().is_empty() || !step_ids.insert(step.step_id.clone()) {
                return Err(ProtocolSimulationError::DuplicateStep {
                    step_id: step.step_id.clone(),
                });
            }
            if !step.cost_units.is_finite() || step.cost_units <= 0.0 {
                return Err(ProtocolSimulationError::InvalidField {
                    field: format!("cost_units:{}", step.step_id),
                });
            }
            if step.operation == ProtocolOperation::InstrumentPreflight {
                preflight_seen = true;
            }
            if step.operation == ProtocolOperation::InstrumentExecution && !preflight_seen {
                return Err(ProtocolSimulationError::MissingPreflight {
                    step_id: step.step_id.clone(),
                });
            }
            if let Some(compensation) = &step.compensation_step {
                if !step_ids.contains(compensation) {
                    // A forward declaration is permitted; it is checked after the full walk.
                }
            }
        }
        for step in &self.steps {
            if let Some(compensation) = &step.compensation_step {
                if !step_ids.contains(compensation) {
                    return Err(ProtocolSimulationError::UnknownStep {
                        step_id: compensation.clone(),
                    });
                }
            }
        }
        let mut scenario_ids = std::collections::BTreeSet::new();
        for scenario in &self.scenarios {
            if scenario.scenario_id.trim().is_empty()
                || !scenario_ids.insert(scenario.scenario_id.clone())
            {
                return Err(ProtocolSimulationError::DuplicateScenario {
                    scenario_id: scenario.scenario_id.clone(),
                });
            }
            if !scenario.budget_multiplier.is_finite() || scenario.budget_multiplier <= 0.0 {
                return Err(ProtocolSimulationError::InvalidField {
                    field: format!("budget_multiplier:{}", scenario.scenario_id),
                });
            }
            if let Some(step_id) = &scenario.fail_at_step {
                if !step_ids.contains(step_id) {
                    return Err(ProtocolSimulationError::UnknownStep {
                        step_id: step_id.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

impl ProtocolSimulationReport {
    pub fn validate(&self) -> Result<(), ProtocolSimulationError> {
        if self.payload.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.payload.feature_id != FEATURE_ID
            || self.payload.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(ProtocolSimulationError::InvalidField {
                field: "research contract boundary".into(),
            });
        }
        if self.payload.results.is_empty() {
            return Err(ProtocolSimulationError::InvalidField {
                field: "results".into(),
            });
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ProtocolSimulationError::Artifact(error.to_string()))
    }

    pub fn verify_artifact(&self) -> Result<(), ProtocolSimulationError> {
        let value = serde_json::to_value(&self.payload)
            .map_err(|error| ProtocolSimulationError::Serialization(error.to_string()))?;
        self.artifact
            .verify_payload(&value)
            .map_err(|error| ProtocolSimulationError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ProtocolSimulationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ProtocolSimulationError::Serialization(error.to_string()))?;
        let bytes = to_canonical_bytes(&value)
            .map_err(|error| ProtocolSimulationError::Serialization(error.to_string()))?;
        Ok(ContentHash::of_bytes(&bytes))
    }
}

pub fn simulate_protocol(
    request: &ProtocolSimulationRequest,
) -> Result<ProtocolSimulationReport, ProtocolSimulationError> {
    request.validate()?;
    let request_value = serde_json::to_value(request)
        .map_err(|error| ProtocolSimulationError::Serialization(error.to_string()))?;
    let request_digest = ContentHash::of_value(&request_value)
        .map_err(|error| ProtocolSimulationError::Serialization(error.to_string()))?;
    let results = request
        .scenarios
        .iter()
        .map(|scenario| simulate_scenario(request, scenario))
        .collect();
    let payload = ProtocolSimulationPayload {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        protocol_id: request.protocol_id.clone(),
        request_digest,
        results,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload_value = serde_json::to_value(&payload)
        .map_err(|error| ProtocolSimulationError::Serialization(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("protocol-simulation:{}", request.protocol_id),
        "application/vnd.aurora.protocol-simulation+json",
        &payload_value,
        vec![],
        vec![],
    )
    .map_err(|error| ProtocolSimulationError::Artifact(error.to_string()))?;
    let report = ProtocolSimulationReport { payload, artifact };
    report.validate()?;
    report.verify_artifact()?;
    Ok(report)
}

fn simulate_scenario(
    request: &ProtocolSimulationRequest,
    scenario: &ProtocolScenario,
) -> ScenarioResult {
    let mut consumed_units = 0.0;
    let budget = request.resource_budget * scenario.budget_multiplier;
    let mut retries = 0;
    let mut executed_steps = Vec::new();
    let mut compensations = Vec::new();
    let mut reasons = Vec::new();
    for step in &request.steps {
        if consumed_units + step.cost_units > budget {
            reasons.push(format!("resource budget exhausted before {}", step.step_id));
            return ScenarioResult {
                scenario_id: scenario.scenario_id.clone(),
                status: ScenarioStatus::FailedClosed,
                executed_steps,
                retries,
                compensations,
                consumed_units,
                reasons,
            };
        }
        if scenario.network_partition && step.operation == ProtocolOperation::FederationExport {
            reasons
                .push("federation partition requires approval and local-only continuation".into());
            return ScenarioResult {
                scenario_id: scenario.scenario_id.clone(),
                status: ScenarioStatus::RequiresApproval,
                executed_steps,
                retries,
                compensations,
                consumed_units,
                reasons,
            };
        }
        consumed_units += step.cost_units;
        if scenario.fail_at_step.as_deref() == Some(step.step_id.as_str()) {
            if step.idempotent && retries < request.max_retries {
                retries += 1;
                reasons.push(format!(
                    "retry {} admitted for idempotent {}",
                    retries, step.step_id
                ));
                if consumed_units + step.cost_units > budget {
                    reasons.push("retry would exceed budget".into());
                    return ScenarioResult {
                        scenario_id: scenario.scenario_id.clone(),
                        status: ScenarioStatus::FailedClosed,
                        executed_steps,
                        retries,
                        compensations,
                        consumed_units,
                        reasons,
                    };
                }
                consumed_units += step.cost_units;
            } else {
                if request.allow_compensation {
                    if let Some(compensation) = &step.compensation_step {
                        compensations.push(compensation.clone());
                        reasons.push(format!("compensation {} scheduled", compensation));
                    }
                }
                reasons.push(format!(
                    "non-retryable failure injected at {}",
                    step.step_id
                ));
                return ScenarioResult {
                    scenario_id: scenario.scenario_id.clone(),
                    status: ScenarioStatus::FailedClosed,
                    executed_steps,
                    retries,
                    compensations,
                    consumed_units,
                    reasons,
                };
            }
        }
        executed_steps.push(format!("{}:{:?}", step.step_id, step.operation.effect()));
    }
    reasons.push("all declared protocol steps completed in simulation".into());
    ScenarioResult {
        scenario_id: scenario.scenario_id.clone(),
        status: ScenarioStatus::Passed,
        executed_steps,
        retries,
        compensations,
        consumed_units,
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ProtocolSimulationRequest {
        ProtocolSimulationRequest {
            protocol_id: "protocol:organoid-imaging-v1".into(),
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
                    step_id: "capture".into(),
                    operation: ProtocolOperation::InstrumentExecution,
                    cost_units: 4.0,
                    idempotent: false,
                    compensation_step: Some("cleanup".into()),
                },
                ProtocolStep {
                    step_id: "cleanup".into(),
                    operation: ProtocolOperation::LocalComputation,
                    cost_units: 1.0,
                    idempotent: true,
                    compensation_step: None,
                },
            ],
            scenarios: vec![
                ProtocolScenario {
                    scenario_id: "nominal".into(),
                    fail_at_step: None,
                    network_partition: false,
                    budget_multiplier: 1.0,
                },
                ProtocolScenario {
                    scenario_id: "capture-failure".into(),
                    fail_at_step: Some("capture".into()),
                    network_partition: false,
                    budget_multiplier: 1.0,
                },
            ],
            max_retries: 2,
            resource_budget: 20.0,
            allow_compensation: true,
        }
    }

    #[test]
    fn nominal_and_failure_scenarios_are_explicitly_simulated() {
        let report = simulate_protocol(&request()).unwrap();
        assert_eq!(report.payload.results[0].status, ScenarioStatus::Passed);
        assert_eq!(
            report.payload.results[1].status,
            ScenarioStatus::FailedClosed
        );
        assert_eq!(report.payload.results[1].compensations, vec!["cleanup"]);
        report.verify_artifact().unwrap();
    }

    #[test]
    fn instrument_execution_without_preflight_is_rejected_before_simulation() {
        let mut invalid = request();
        invalid.steps.remove(0);
        assert!(matches!(
            simulate_protocol(&invalid),
            Err(ProtocolSimulationError::MissingPreflight { .. })
        ));
    }

    #[test]
    fn partitioned_federation_is_not_silently_marked_passed() {
        let mut partition = request();
        partition.steps.push(ProtocolStep {
            step_id: "export".into(),
            operation: ProtocolOperation::FederationExport,
            cost_units: 1.0,
            idempotent: true,
            compensation_step: None,
        });
        partition.scenarios = vec![ProtocolScenario {
            scenario_id: "partition".into(),
            fail_at_step: None,
            network_partition: true,
            budget_multiplier: 1.0,
        }];
        let report = simulate_protocol(&partition).unwrap();
        assert_eq!(
            report.payload.results[0].status,
            ScenarioStatus::RequiresApproval
        );
    }

    #[test]
    fn identical_protocol_requests_have_identical_report_digests() {
        assert_eq!(
            simulate_protocol(&request()).unwrap().digest().unwrap(),
            simulate_protocol(&request()).unwrap().digest().unwrap()
        );
    }
}
