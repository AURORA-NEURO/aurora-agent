//! Decision-context to autonomous-mission execution bridge.
//!
//! P04 already compiles typed claims into a dependency-closed action graph.  This module makes
//! that graph executable by the P07 mission controller.  The bridge is intentionally small but
//! consequential: it preserves the immutable context/graph digests, carries graph dependencies
//! into the autonomous selector, and returns a held result when unresolved decision debt is not
//! explicitly allowed.  It does not reinterpret claims or bypass the local executor seam.

use super::{DecisionActionGraph, DecisionActionGraphDisposition, DecisionContext};
use crate::glioma::programs::p07_protocol_simulation::{
    execute_glioma_autonomous_research_mission, GliomaActionExecutor,
    GliomaAutonomousResearchMission, GliomaMissionError, GliomaMissionGates, GliomaMissionRequest,
};
use crate::glioma_engine::GliomaSelectionConfig;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F25";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionMissionBridge1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionMissionBridgeRequest {
    pub mission_id: String,
    pub objective: String,
    pub context: DecisionContext,
    pub graph: DecisionActionGraph,
    /// Canonical actions already completed by an earlier local run.  This makes the bridge
    /// resumable without mutating the original context or graph digest.
    #[serde(default)]
    pub completed_action_order: Vec<String>,
    pub selection: GliomaSelectionConfig,
    pub gates: GliomaMissionGates,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub require_artifacts: bool,
    pub stop_on_negative: bool,
    /// Allow execution when the graph is partial (for example, it carries explicit unresolved
    /// paths) rather than requiring a fully qualified decision context.
    pub allow_partial_graph: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionMissionBridgeDisposition {
    Executed,
    Held,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionMissionBridgeRun {
    pub feature_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub context_digest: ContentHash,
    pub graph_digest: ContentHash,
    pub action_order: Vec<String>,
    pub omitted_action_order: Vec<String>,
    pub mission: Option<GliomaAutonomousResearchMission>,
    pub disposition: DecisionMissionBridgeDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionMissionBridgeError {
    #[error("decision mission bridge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision mission bridge input is invalid: {0}")]
    InvalidInput(String),
    #[error("decision mission bridge execution failed: {0}")]
    Execution(#[from] GliomaMissionError),
    #[error("decision mission bridge output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision mission bridge digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(run: &DecisionMissionBridgeRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "mission_id": run.mission_id,
        "objective": run.objective,
        "context_digest": run.context_digest,
        "graph_digest": run.graph_digest,
        "action_order": run.action_order,
        "omitted_action_order": run.omitted_action_order,
        "mission": run.mission,
        "disposition": run.disposition,
        "next_step": run.next_step,
    })
}

impl DecisionMissionBridgeRun {
    pub fn validate(&self) -> Result<(), DecisionMissionBridgeError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.context_digest.as_str().len() != 64
            || self.graph_digest.as_str().len() != 64
            || !canonical(&self.action_order)
            || !canonical(&self.omitted_action_order)
            || self
                .omitted_action_order
                .iter()
                .any(|id| self.action_order.binary_search(id).is_err())
            || matches!(self.disposition, DecisionMissionBridgeDisposition::Held)
                == self.mission.is_some()
        {
            return Err(DecisionMissionBridgeError::InvalidOutput(
                "identity, canonical action partition, mission presence, or digest binding is invalid".into(),
            ));
        }
        if let Some(mission) = &self.mission {
            mission
                .validate()
                .map_err(DecisionMissionBridgeError::Execution)?;
            if mission.mission_id != self.mission_id || mission.objective != self.objective {
                return Err(DecisionMissionBridgeError::InvalidOutput(
                    "nested mission identity does not match bridge identity".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionMissionBridgeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionMissionBridgeError::InvalidOutput(
                "bridge digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &DecisionMissionBridgeRequest,
) -> Result<(), DecisionMissionBridgeError> {
    if request.mission_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.selection.budget_units == 0
        || request.selection.max_actions == 0
        || request.max_rounds == 0
        || !canonical(&request.completed_action_order)
        || request
            .completed_action_order
            .iter()
            .any(|id| id.trim().is_empty() || request.graph.node_order.binary_search(id).is_err())
    {
        return Err(DecisionMissionBridgeError::InvalidRequest(
            "mission identity, objective, positive budget/action limits, and rounds are required"
                .into(),
        ));
    }
    request
        .context
        .validate()
        .map_err(|error| DecisionMissionBridgeError::InvalidInput(error.to_string()))?;
    request
        .graph
        .validate()
        .map_err(|error| DecisionMissionBridgeError::InvalidInput(error.to_string()))?;
    if request.objective != request.context.objective
        || request.objective != request.graph.objective
        || request.graph.context_digest != request.context.digest
    {
        return Err(DecisionMissionBridgeError::InvalidRequest(
            "objective and context/graph digest bindings must match".into(),
        ));
    }
    Ok(())
}

/// Execute a decision graph through the science-aware autonomous mission controller.
pub fn execute_glioma_decision_mission<E: GliomaActionExecutor>(
    request: &DecisionMissionBridgeRequest,
    executor: &mut E,
) -> Result<DecisionMissionBridgeRun, DecisionMissionBridgeError> {
    validate_request(request)?;
    if !request.allow_partial_graph
        && request.graph.disposition != DecisionActionGraphDisposition::Qualified
    {
        let mut output = DecisionMissionBridgeRun {
            feature_id: FEATURE_ID.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            mission_id: request.mission_id.clone(),
            objective: request.objective.clone(),
            context_digest: request.context.digest.clone(),
            graph_digest: request.graph.digest.clone(),
            action_order: request.graph.node_order.clone(),
            omitted_action_order: request.graph.node_order.clone(),
            mission: None,
            disposition: DecisionMissionBridgeDisposition::Held,
            next_step: "resolve the graph's explicit missing, unresolved, negative, or budget-blocked frontier before dispatch".into(),
            digest: ContentHash::of_bytes(b"unsealed-glioma-decision-mission-bridge"),
        };
        output.digest = ContentHash::of_value(&digest_input(&output))
            .map_err(|error| DecisionMissionBridgeError::Digest(error.to_string()))?;
        output.validate()?;
        return Ok(output);
    }
    let candidates = request
        .graph
        .nodes
        .iter()
        .map(|node| node.action.clone())
        .collect::<Vec<_>>();
    let mission_request = GliomaMissionRequest {
        mission_id: request.mission_id.clone(),
        objective: request.objective.clone(),
        candidates,
        completed_action_order: request.completed_action_order.clone(),
        selection: request.selection.clone(),
        gates: request.gates.clone(),
        max_rounds: request.max_rounds,
        max_retries: request.max_retries,
        require_artifacts: request.require_artifacts,
        stop_on_negative: request.stop_on_negative,
    };
    let mission = execute_glioma_autonomous_research_mission(&mission_request, executor)?;
    let omitted = mission
        .unresolved_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let disposition = match mission.disposition {
        crate::glioma::programs::p07_protocol_simulation::GliomaMissionDisposition::Blocked => {
            DecisionMissionBridgeDisposition::Blocked
        }
        _ => DecisionMissionBridgeDisposition::Executed,
    };
    let next_step = match disposition {
        DecisionMissionBridgeDisposition::Executed => {
            "inspect the mission's typed outcomes, then feed remaining action debt into the next decision graph"
        }
        DecisionMissionBridgeDisposition::Blocked => {
            "hold downstream work until the mission's failed, unsafe, or budget-blocked frontier is resolved"
        }
        DecisionMissionBridgeDisposition::Held => unreachable!(),
    }
    .to_string();
    let mut output = DecisionMissionBridgeRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        mission_id: request.mission_id.clone(),
        objective: request.objective.clone(),
        context_digest: request.context.digest.clone(),
        graph_digest: request.graph.digest.clone(),
        action_order: request.graph.node_order.clone(),
        omitted_action_order: omitted,
        mission: Some(mission),
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-mission-bridge"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DecisionMissionBridgeError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}
