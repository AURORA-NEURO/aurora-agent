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
const MAX_TEXT_BYTES: usize = 512;
const MAX_STEPS: usize = 16_384;
const MAX_SCENARIOS: usize = 8_192;
const MAX_RETRIES: u32 = 32;
const MAX_COST_UNITS: f64 = 1_000_000_000.0;

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
    pub input: ProtocolDraft,
    pub input_digest: ContentHash,
    pub protocol_id: String,
    pub design_digest: ContentHash,
    pub step_order: Vec<String>,
    pub max_retries: u32,
    pub resource_budget: f64,
    pub approval_required_for_effects: bool,
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
        validate_text("protocol_id", &self.protocol_id)?;
        validate_text("boundary", &self.boundary)?;
        if self.design_digest == ContentHash::of_bytes(b"") {
            return Err(ProtocolSimulationError::InvalidField(
                "design digest is required".into(),
            ));
        }
        if self.step_order.is_empty() || self.step_order.len() > MAX_STEPS {
            return Err(ProtocolSimulationError::InvalidField(
                "step order is outside its simulation bound".into(),
            ));
        }
        validate_unique_texts(&self.step_order, "step_order", MAX_STEPS)?;
        if self.max_retries > MAX_RETRIES
            || !self.resource_budget.is_finite()
            || self.resource_budget <= 0.0
            || self.resource_budget > MAX_COST_UNITS
        {
            return Err(ProtocolSimulationError::InvalidField(
                "simulation controls are outside their bounded contract".into(),
            ));
        }
        if self
            .passed
            .checked_add(self.failed_closed)
            .and_then(|count| count.checked_add(self.approval_required))
            != Some(self.results.len())
        {
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
        if self.omissions.len() > MAX_SCENARIOS || self.uncertainty.len() > MAX_SCENARIOS {
            return Err(ProtocolSimulationError::InvalidField(
                "simulation receipt notes exceed their item bound".into(),
            ));
        }
        validate_sorted_texts(&self.omissions, "omissions", MAX_SCENARIOS)?;
        validate_sorted_texts(&self.uncertainty, "uncertainty", MAX_SCENARIOS)?;
        validate_semantic_loss(&self.semantic_loss)?;
        let mut scenario_ids = BTreeSet::new();
        for result in &self.results {
            validate_text("scenario_id", &result.scenario_id)?;
            if !scenario_ids.insert(result.scenario_id.clone()) {
                return Err(ProtocolSimulationError::InvalidField(
                    "scenario results contain duplicate scenario ids".into(),
                ));
            }
            if result.executed_steps.len() > self.step_order.len()
                || self.step_order[..result.executed_steps.len()] != result.executed_steps
            {
                return Err(ProtocolSimulationError::InvalidField(format!(
                    "scenario {} executed steps are not a protocol prefix",
                    result.scenario_id
                )));
            }
            validate_unique_texts(&result.executed_steps, "result.executed_steps", MAX_STEPS)?;
            validate_unique_texts(&result.compensations, "result.compensations", MAX_STEPS)?;
            validate_unique_texts(&result.reasons, "result.reasons", MAX_SCENARIOS)?;
            if result
                .compensations
                .iter()
                .any(|step| !self.step_order.iter().any(|candidate| candidate == step))
            {
                return Err(ProtocolSimulationError::InvalidField(format!(
                    "scenario {} contains an unknown compensation step",
                    result.scenario_id
                )));
            }
            if result.retries > self.max_retries
                || !result.consumed_units.is_finite()
                || result.consumed_units < 0.0
            {
                return Err(ProtocolSimulationError::InvalidField(format!(
                    "scenario {} retry or consumption state is invalid",
                    result.scenario_id
                )));
            }
            if result.state == ProtocolSimulationState::Passed
                && result.executed_steps != self.step_order
            {
                return Err(ProtocolSimulationError::InvalidField(format!(
                    "passed scenario {} did not execute the complete protocol",
                    result.scenario_id
                )));
            }
        }
        if self.artifact.artifact_id != format!("protocol-simulation:{}", self.protocol_id)
            || self.artifact.content_type
                != "application/vnd.aurora.prospective-protocol-simulation+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || self.artifact.provenance
                != vec![ProvenanceLink {
                    source_id: self.protocol_id.clone(),
                    relation: "simulated-from-local-protocol-draft".into(),
                    digest: self.design_digest.clone(),
                }]
        {
            return Err(ProtocolSimulationError::Contract(
                "simulation artifact is not bound to the protocol draft".into(),
            ));
        }
        let payload = simulation_payload(
            &self.protocol_id,
            &self.design_digest,
            &self.step_order,
            self.max_retries,
            self.resource_budget,
            self.approval_required_for_effects,
            &self.results,
            self.passed,
            self.failed_closed,
            self.approval_required,
            &self.omissions,
            &self.uncertainty,
            &self.semantic_loss,
            self.raw_data_local,
            &self.boundary,
        );
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| ProtocolSimulationError::Contract(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| ProtocolSimulationError::Contract(error.to_string()))?;
        validate_draft(&self.input)?;
        if self.input_digest != protocol_input_digest(&self.input)? {
            return Err(ProtocolSimulationError::Contract(
                "protocol simulation retained input digest does not match the draft".into(),
            ));
        }
        let expected = build_protocol_simulation(&self.input)?;
        if self != &expected {
            return Err(ProtocolSimulationError::Contract(
                "protocol simulation receipt is not derived from its retained draft".into(),
            ));
        }
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

fn validate_text(field: &str, value: &str) -> Result<(), ProtocolSimulationError> {
    if value.is_empty() || value.trim() != value {
        return Err(ProtocolSimulationError::InvalidField(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(ProtocolSimulationError::InvalidField(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn protocol_input_digest(draft: &ProtocolDraft) -> Result<ContentHash, ProtocolSimulationError> {
    let value = serde_json::to_value(&canonical_protocol_draft(draft))
        .map_err(|error| ProtocolSimulationError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| ProtocolSimulationError::Serialization(error.to_string()))
}

fn canonical_protocol_draft(draft: &ProtocolDraft) -> ProtocolDraft {
    let mut canonical = draft.clone();
    canonical
        .scenarios
        .sort_by(|left, right| left.scenario_id.cmp(&right.scenario_id));
    canonical
}

fn validate_unique_texts(
    values: &[String],
    field: &str,
    max_items: usize,
) -> Result<(), ProtocolSimulationError> {
    if values.len() > max_items {
        return Err(ProtocolSimulationError::InvalidField(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(ProtocolSimulationError::InvalidField(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_texts(
    values: &[String],
    field: &str,
    max_items: usize,
) -> Result<(), ProtocolSimulationError> {
    validate_unique_texts(values, field, max_items)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ProtocolSimulationError::InvalidField(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_semantic_loss(semantic_loss: &[SemanticLoss]) -> Result<(), ProtocolSimulationError> {
    if semantic_loss.len() > MAX_SCENARIOS {
        return Err(ProtocolSimulationError::InvalidField(
            "semantic_loss exceeds its item bound".into(),
        ));
    }
    for loss in semantic_loss {
        validate_text("semantic_loss.field", &loss.field)?;
        validate_text("semantic_loss.reason", &loss.reason)?;
    }
    if semantic_loss.windows(2).any(|pair| {
        (
            pair[0].field.as_str(),
            pair[0].reason.as_str(),
            pair[0].severity,
        ) >= (
            pair[1].field.as_str(),
            pair[1].reason.as_str(),
            pair[1].severity,
        )
    }) {
        return Err(ProtocolSimulationError::InvalidField(
            "semantic_loss ordering is not canonical".into(),
        ));
    }
    Ok(())
}

fn simulation_payload(
    protocol_id: &str,
    design_digest: &ContentHash,
    step_order: &[String],
    max_retries: u32,
    resource_budget: f64,
    approval_required_for_effects: bool,
    results: &[ProtocolScenarioResult],
    passed: usize,
    failed_closed: usize,
    approval_required: usize,
    omissions: &[String],
    uncertainty: &[String],
    semantic_loss: &[SemanticLoss],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "protocol_id": protocol_id,
        "design_digest": design_digest,
        "step_order": step_order,
        "max_retries": max_retries,
        "resource_budget": resource_budget,
        "approval_required_for_effects": approval_required_for_effects,
        "results": results,
        "passed": passed,
        "failed_closed": failed_closed,
        "approval_required": approval_required,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "semantic_loss": semantic_loss,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
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
    let receipt = build_protocol_simulation(draft)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_protocol_simulation(
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
    let step_order = draft
        .steps
        .iter()
        .map(|step| step.step_id.clone())
        .collect::<Vec<_>>();
    let payload = simulation_payload(
        &draft.protocol_id,
        &draft.design_digest,
        &step_order,
        draft.max_retries,
        draft.resource_budget,
        draft.approval_required_for_effects,
        &results,
        passed,
        failed_closed,
        approval_required,
        &omissions,
        &uncertainty,
        &semantic_loss,
        draft.raw_data_local,
        &draft.boundary,
    );
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
        input: canonical_protocol_draft(draft),
        input_digest: protocol_input_digest(draft)?,
        protocol_id: draft.protocol_id.clone(),
        design_digest: draft.design_digest.clone(),
        step_order,
        max_retries: draft.max_retries,
        resource_budget: draft.resource_budget,
        approval_required_for_effects: draft.approval_required_for_effects,
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
    Ok(receipt)
}

fn simulate_scenario(draft: &ProtocolDraft, scenario: &ProtocolScenario) -> ProtocolScenarioResult {
    let mut consumed_units = 0.0;
    let budget = draft.resource_budget * scenario.budget_multiplier;
    let mut executed_steps = Vec::new();
    let mut compensations = Vec::new();
    let mut reasons = Vec::new();
    let mut retries = 0;
    for step in &draft.steps {
        let mut step_retries = 0;
        loop {
            if consumed_units + step.cost_units > budget {
                reasons.push(format!("resource budget exhausted before {}", step.step_id));
                return ProtocolScenarioResult {
                    scenario_id: scenario.scenario_id.clone(),
                    state: ProtocolSimulationState::FailedClosed,
                    executed_steps,
                    retries,
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
                    retries,
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
                    retries,
                    compensations,
                    consumed_units,
                    reasons,
                };
            }
            consumed_units += step.cost_units;
            if scenario.fail_at_step.as_deref() == Some(step.step_id.as_str()) {
                if step.idempotent && step_retries < draft.max_retries {
                    step_retries += 1;
                    retries += 1;
                    reasons.push(format!(
                        "idempotent step {} retried after injected failure ({}/{})",
                        step.step_id, step_retries, draft.max_retries
                    ));
                    continue;
                }
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
                    retries,
                    compensations,
                    consumed_units,
                    reasons,
                };
            }
            executed_steps.push(step.step_id.clone());
            break;
        }
    }
    reasons.push("all simulated protocol steps completed within declared budget".into());
    ProtocolScenarioResult {
        scenario_id: scenario.scenario_id.clone(),
        state: ProtocolSimulationState::Passed,
        executed_steps,
        retries,
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
    validate_text("protocol_id", &draft.protocol_id)?;
    validate_text("boundary", &draft.boundary)?;
    if draft.design_digest == ContentHash::of_bytes(b"") {
        return Err(ProtocolSimulationError::InvalidField(
            "design digest is required".into(),
        ));
    }
    if draft.steps.len() > MAX_STEPS
        || draft.scenarios.len() > MAX_SCENARIOS
        || draft.max_retries > MAX_RETRIES
    {
        return Err(ProtocolSimulationError::InvalidField(
            "protocol steps, scenarios, or retries exceed their bounds".into(),
        ));
    }
    if draft.resource_budget > MAX_COST_UNITS {
        return Err(ProtocolSimulationError::InvalidField(
            "resource budget exceeds its bound".into(),
        ));
    }
    let mut step_ids = BTreeSet::new();
    let mut preflight_seen = false;
    for step in &draft.steps {
        validate_text("step_id", &step.step_id)?;
        if !step_ids.insert(step.step_id.clone()) {
            return Err(ProtocolSimulationError::DuplicateStep(step.step_id.clone()));
        }
        if !step.cost_units.is_finite()
            || step.cost_units <= 0.0
            || step.cost_units > MAX_COST_UNITS
        {
            return Err(ProtocolSimulationError::InvalidField(format!(
                "cost_units:{}",
                step.step_id
            )));
        }
        if let Some(compensation) = &step.compensation_step {
            validate_text("compensation_step", compensation)?;
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
        validate_text("scenario_id", &scenario.scenario_id)?;
        if !scenario_ids.insert(scenario.scenario_id.clone()) {
            return Err(ProtocolSimulationError::DuplicateScenario(
                scenario.scenario_id.clone(),
            ));
        }
        if !scenario.budget_multiplier.is_finite()
            || scenario.budget_multiplier <= 0.0
            || scenario.budget_multiplier > MAX_COST_UNITS
        {
            return Err(ProtocolSimulationError::InvalidField(format!(
                "budget_multiplier:{}",
                scenario.scenario_id
            )));
        }
        if let Some(step_id) = &scenario.fail_at_step {
            validate_text("fail_at_step", step_id)?;
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

    #[test]
    fn idempotent_failure_uses_only_the_declared_retry_budget() {
        let mut draft = draft();
        draft.scenarios = vec![ProtocolScenario {
            scenario_id: "scenario:retry".into(),
            fail_at_step: Some("preflight".into()),
            network_partition: false,
            budget_multiplier: 1.0,
        }];
        let receipt = simulate_protocol_draft(&draft).unwrap();
        assert_eq!(receipt.failed_closed, 1);
        assert_eq!(receipt.results[0].retries, 1);
        assert_eq!(receipt.results[0].consumed_units, 2.0);
    }

    #[test]
    fn receipt_rejects_non_prefix_execution_state() {
        let mut receipt = simulate_protocol_draft(&draft()).unwrap();
        receipt.results[0].executed_steps = vec!["cleanup".into()];
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn simulation_artifact_payload_is_verified() {
        let mut receipt = simulate_protocol_draft(&draft()).unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_draft_tampering_is_rejected() {
        let mut receipt = simulate_protocol_draft(&draft()).unwrap();
        receipt.input.protocol_id = "protocol:tampered".into();
        assert!(receipt.validate().is_err());
    }
}
