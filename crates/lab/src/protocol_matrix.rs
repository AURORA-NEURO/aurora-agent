//! Bounded factorial protocol robustness matrix built on the deterministic simulator.
//!
//! Atlas feature: `AFA-lab-P10-F02`.

use crate::protocol_simulation::{
    simulate_protocol, ProtocolScenario, ProtocolSimulationReport, ProtocolSimulationRequest,
    ProtocolStep, ScenarioStatus,
};
use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-lab-P10-F02";
pub const FEATURE_VERSION: &str = "0.1.0";
pub const MAX_MATRIX_CELLS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MatrixCondition {
    Nominal,
    NetworkPartition,
    FailAtStep(String),
    BudgetMultiplier(f64),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatrixFactor {
    pub factor_id: String,
    pub levels: Vec<MatrixCondition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtocolMatrixRequest {
    pub protocol_id: String,
    pub design_digest: ContentHash,
    pub steps: Vec<ProtocolStep>,
    pub factors: Vec<MatrixFactor>,
    pub max_retries: u32,
    pub resource_budget: f64,
    pub allow_compensation: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatrixCellResult {
    pub cell_id: String,
    pub assignments: BTreeMap<String, String>,
    pub status: ScenarioStatus,
    pub consumed_units: f64,
    pub reasons: Vec<String>,
    pub report_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtocolMatrixReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub protocol_id: String,
    pub total_cells: usize,
    pub passed_cells: usize,
    pub failed_closed_cells: usize,
    pub approval_cells: usize,
    pub cells: Vec<MatrixCellResult>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl ProtocolMatrixReceipt {
    pub fn validate(&self) -> Result<(), ProtocolMatrixError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(ProtocolMatrixError::InvalidField(
                "schema or feature".into(),
            ));
        }
        if self.protocol_id.trim().is_empty()
            || self.cells.is_empty()
            || self.total_cells != self.cells.len()
        {
            return Err(ProtocolMatrixError::InvalidField(
                "protocol, cells, or total_cells".into(),
            ));
        }
        if self.passed_cells + self.failed_closed_cells + self.approval_cells != self.total_cells {
            return Err(ProtocolMatrixError::InvalidField(
                "cell status counts".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ProtocolMatrixError::Artifact(error.to_string()))?;
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ProtocolMatrixError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ProtocolMatrixError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ProtocolMatrixError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ProtocolMatrixError {
    #[error("invalid protocol matrix field: {0}")]
    InvalidField(String),
    #[error("duplicate matrix factor {0}")]
    DuplicateFactor(String),
    #[error("matrix factor {0} has no levels")]
    EmptyFactor(String),
    #[error("matrix exceeds the {MAX_MATRIX_CELLS} cell safety bound")]
    TooManyCells,
    #[error("matrix condition references unknown step {0}")]
    UnknownStep(String),
    #[error("matrix condition has invalid budget multiplier")]
    InvalidBudgetMultiplier,
    #[error("protocol simulation failed: {0}")]
    Simulation(String),
    #[error("matrix artifact error: {0}")]
    Artifact(String),
    #[error("matrix serialization error: {0}")]
    Serialization(String),
}

pub fn protocol_matrix_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "lab".into(),
        consumers: ["protocol engineer".into(), "instrument operator".into()].into(),
        behavior: "enumerates a bounded factorial matrix of protocol failures, partitions, and budget conditions using deterministic simulation".into(),
        value: "reveals fail-closed protocol cells and approval boundaries before physical execution".into(),
        inputs: vec![TypedPort {
            name: "protocol_matrix_request".into(),
            schema: "ProtocolMatrixRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "protocol_matrix_receipt".into(),
            schema: "ProtocolMatrixReceipt@1".into(),
            required: true,
        }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:local-protocol-design".into(), "write:local-simulation-receipt".into()]
            .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "ga4gh-wes".into(),
            state: EvidenceState::Supported,
            locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()),
        }],
        authority_requirements: vec![AuthorityRequirement {
            role: "protocol curator".into(),
            reason: "factor definitions and bounded simulation budgets are accountable design inputs".into(),
        }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn simulate_protocol_matrix(
    request: &ProtocolMatrixRequest,
) -> Result<ProtocolMatrixReceipt, ProtocolMatrixError> {
    validate_request(request)?;
    // Factor order is an input convenience, not an identity. Canonical sorting
    // keeps cell IDs, assignments, and receipt digests byte-stable across clients.
    let mut factors = request.factors.clone();
    factors.sort_by(|left, right| left.factor_id.cmp(&right.factor_id));
    let combinations = combinations(&factors);
    let mut cells = Vec::with_capacity(combinations.len());
    for (index, combination) in combinations.into_iter().enumerate() {
        let mut network_partition = false;
        let mut fail_at_step = None;
        let mut budget_multiplier = 1.0;
        let mut assignments = BTreeMap::new();
        for (factor, condition) in factors.iter().zip(combination.iter()) {
            assignments.insert(factor.factor_id.clone(), condition_label(condition));
            match condition {
                MatrixCondition::NetworkPartition => network_partition = true,
                MatrixCondition::FailAtStep(step) => fail_at_step = Some(step.clone()),
                MatrixCondition::BudgetMultiplier(multiplier) => budget_multiplier *= *multiplier,
                MatrixCondition::Nominal => {}
            }
        }
        let scenario = ProtocolScenario {
            scenario_id: format!("matrix-cell-{index:04}"),
            fail_at_step,
            network_partition,
            budget_multiplier,
        };
        let simulation_request = ProtocolSimulationRequest {
            protocol_id: request.protocol_id.clone(),
            design_digest: request.design_digest.clone(),
            steps: request.steps.clone(),
            scenarios: vec![scenario],
            max_retries: request.max_retries,
            resource_budget: request.resource_budget,
            allow_compensation: request.allow_compensation,
        };
        let report = simulate_protocol(&simulation_request)
            .map_err(|error| ProtocolMatrixError::Simulation(error.to_string()))?;
        cells.push(cell_result(index, assignments, &report)?);
    }
    let passed_cells = cells
        .iter()
        .filter(|cell| cell.status == ScenarioStatus::Passed)
        .count();
    let failed_closed_cells = cells
        .iter()
        .filter(|cell| cell.status == ScenarioStatus::FailedClosed)
        .count();
    let approval_cells = cells
        .iter()
        .filter(|cell| cell.status == ScenarioStatus::RequiresApproval)
        .count();
    let total_cells = cells.len();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "protocol_id": request.protocol_id,
        "total_cells": total_cells,
        "passed_cells": passed_cells,
        "failed_closed_cells": failed_closed_cells,
        "approval_cells": approval_cells,
        "cells": cells,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("protocol-matrix:{}", request.protocol_id),
        "application/vnd.aurora.protocol-matrix+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ProtocolMatrixError::Artifact(error.to_string()))?;
    let receipt = ProtocolMatrixReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        protocol_id: request.protocol_id.clone(),
        total_cells,
        passed_cells,
        failed_closed_cells,
        approval_cells,
        cells: serde_json::from_value(payload["cells"].clone())
            .map_err(|error| ProtocolMatrixError::Serialization(error.to_string()))?,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ProtocolMatrixRequest) -> Result<(), ProtocolMatrixError> {
    if request.protocol_id.trim().is_empty()
        || request.steps.is_empty()
        || request.factors.is_empty()
    {
        return Err(ProtocolMatrixError::InvalidField(
            "protocol, steps, and factors".into(),
        ));
    }
    if !request.resource_budget.is_finite() || request.resource_budget <= 0.0 {
        return Err(ProtocolMatrixError::InvalidField("resource_budget".into()));
    }
    let step_ids = request
        .steps
        .iter()
        .map(|step| step.step_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut factor_ids = BTreeSet::new();
    let mut cell_count = 1usize;
    for factor in &request.factors {
        if factor.factor_id.trim().is_empty() || !factor_ids.insert(factor.factor_id.clone()) {
            return Err(ProtocolMatrixError::DuplicateFactor(
                factor.factor_id.clone(),
            ));
        }
        if factor.levels.is_empty() {
            return Err(ProtocolMatrixError::EmptyFactor(factor.factor_id.clone()));
        }
        cell_count = cell_count.saturating_mul(factor.levels.len());
        if cell_count > MAX_MATRIX_CELLS {
            return Err(ProtocolMatrixError::TooManyCells);
        }
        for condition in &factor.levels {
            if let MatrixCondition::FailAtStep(step) = condition {
                if !step_ids.contains(step.as_str()) {
                    return Err(ProtocolMatrixError::UnknownStep(step.clone()));
                }
            }
            if let MatrixCondition::BudgetMultiplier(multiplier) = condition {
                if !multiplier.is_finite() || *multiplier <= 0.0 {
                    return Err(ProtocolMatrixError::InvalidBudgetMultiplier);
                }
            }
        }
    }
    Ok(())
}

fn combinations(factors: &[MatrixFactor]) -> Vec<Vec<MatrixCondition>> {
    factors.iter().fold(vec![Vec::new()], |prefixes, factor| {
        prefixes
            .into_iter()
            .flat_map(|prefix| {
                factor.levels.iter().cloned().map(move |level| {
                    let mut next = prefix.clone();
                    next.push(level);
                    next
                })
            })
            .collect()
    })
}

fn condition_label(condition: &MatrixCondition) -> String {
    match condition {
        MatrixCondition::Nominal => "nominal".into(),
        MatrixCondition::NetworkPartition => "network_partition".into(),
        MatrixCondition::FailAtStep(step) => format!("fail_at:{step}"),
        MatrixCondition::BudgetMultiplier(multiplier) => {
            format!("budget_multiplier:{multiplier:.6}")
        }
    }
}

fn cell_result(
    index: usize,
    assignments: BTreeMap<String, String>,
    report: &ProtocolSimulationReport,
) -> Result<MatrixCellResult, ProtocolMatrixError> {
    let scenario =
        report.payload.results.first().ok_or_else(|| {
            ProtocolMatrixError::Simulation("simulation returned no scenario".into())
        })?;
    let report_digest = report
        .digest()
        .map_err(|error| ProtocolMatrixError::Simulation(error.to_string()))?;
    Ok(MatrixCellResult {
        cell_id: format!("matrix-cell-{index:04}"),
        assignments,
        status: scenario.status,
        consumed_units: scenario.consumed_units,
        reasons: scenario.reasons.clone(),
        report_digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol_simulation::ProtocolOperation;

    fn request() -> ProtocolMatrixRequest {
        ProtocolMatrixRequest {
            protocol_id: "protocol:matrix-1".into(),
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
                    step_id: "export".into(),
                    operation: ProtocolOperation::FederationExport,
                    cost_units: 1.0,
                    idempotent: true,
                    compensation_step: None,
                },
            ],
            factors: vec![
                MatrixFactor {
                    factor_id: "network".into(),
                    levels: vec![MatrixCondition::Nominal, MatrixCondition::NetworkPartition],
                },
                MatrixFactor {
                    factor_id: "budget".into(),
                    levels: vec![
                        MatrixCondition::BudgetMultiplier(1.0),
                        MatrixCondition::BudgetMultiplier(0.5),
                    ],
                },
            ],
            max_retries: 1,
            resource_budget: 3.0,
            allow_compensation: true,
        }
    }

    #[test]
    fn matrix_is_deterministic_and_reports_all_cells() {
        let mut reversed = request();
        reversed.factors.reverse();
        let left = simulate_protocol_matrix(&request()).unwrap();
        let right = simulate_protocol_matrix(&reversed).unwrap();
        assert_eq!(left.total_cells, 4);
        assert_eq!(
            left.passed_cells + left.failed_closed_cells + left.approval_cells,
            4
        );
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
    }

    #[test]
    fn unknown_failure_step_is_rejected() {
        let mut request = request();
        request.factors[0]
            .levels
            .push(MatrixCondition::FailAtStep("missing".into()));
        assert!(matches!(
            simulate_protocol_matrix(&request).unwrap_err(),
            ProtocolMatrixError::UnknownStep(_)
        ));
    }

    #[test]
    fn matrix_cell_bound_is_fail_closed() {
        let mut request = request();
        request.factors = (0..13)
            .map(|index| MatrixFactor {
                factor_id: format!("factor-{index}"),
                levels: vec![MatrixCondition::Nominal, MatrixCondition::NetworkPartition],
            })
            .collect();
        assert!(matches!(
            simulate_protocol_matrix(&request).unwrap_err(),
            ProtocolMatrixError::TooManyCells
        ));
    }
}
