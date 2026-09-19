//! Knowledge-frontier selection cycle for the autonomous glioma engine.
//!
//! This is the first P02 surface that consumes the knowledge-action bridge and invokes the
//! existing dependency-aware portfolio selector in one deterministic operation. It is still a
//! planning capability: selected candidates are returned for a caller-owned executor, while
//! completed work, budget holds, blocked dependencies, and omitted bindings remain explicit.

use super::action_bridge::{
    bridge_glioma_knowledge_actions, KnowledgeActionBridgeError, KnowledgeActionBridgeRequest,
};
use super::action_compiler::KnowledgeActionPlan;
use crate::glioma_engine::{select_glioma_actions, GliomaActionSelection, GliomaSelectionConfig};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F18";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeSelectionCycle1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeActionSelectionCycleRequest {
    pub objective: String,
    pub plan_digest: ContentHash,
    pub max_candidates: usize,
    pub completed_source_action_ids: BTreeSet<String>,
    pub selection_config: GliomaSelectionConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeActionSelectionCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub plan_digest: ContentHash,
    pub bridge_digest: ContentHash,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub selected_source_order: Vec<String>,
    pub deferred_source_order: Vec<String>,
    pub blocked_source_order: Vec<String>,
    pub omitted_source_order: Vec<String>,
    pub selection: GliomaActionSelection,
    pub dispatch: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeActionSelectionCycleError {
    #[error("knowledge action selection-cycle request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge action selection-cycle input is invalid: {0}")]
    InvalidInput(String),
    #[error("knowledge action selection-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge action selection-cycle bridge failed: {0}")]
    Bridge(#[from] KnowledgeActionBridgeError),
    #[error("knowledge action selection-cycle selector failed: {0}")]
    Selector(String),
    #[error("knowledge action selection-cycle digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique(values: &[String]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(output: &KnowledgeActionSelectionCycle) -> serde_json::Value {
    json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "plan_digest": output.plan_digest,
        "bridge_digest": output.bridge_digest,
        "candidate_order": output.candidate_order,
        "selected_order": output.selected_order,
        "selected_source_order": output.selected_source_order,
        "deferred_source_order": output.deferred_source_order,
        "blocked_source_order": output.blocked_source_order,
        "omitted_source_order": output.omitted_source_order,
        "selection": output.selection,
        "dispatch": output.dispatch,
    })
}

fn source_id(candidate_id: &str) -> Option<String> {
    candidate_id
        .strip_prefix("knowledge-action:")
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string)
}

fn source_ids(selection_ids: &[String]) -> Result<Vec<String>, KnowledgeActionSelectionCycleError> {
    let values = selection_ids
        .iter()
        .map(|id| {
            source_id(id).ok_or_else(|| {
                KnowledgeActionSelectionCycleError::InvalidOutput(
                    "selector emitted a candidate outside the knowledge-action namespace".into(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !unique(&values) {
        return Err(KnowledgeActionSelectionCycleError::InvalidOutput(
            "selector emitted a duplicate knowledge-action id".into(),
        ));
    }
    Ok(values)
}

impl KnowledgeActionSelectionCycle {
    pub fn validate(&self) -> Result<(), KnowledgeActionSelectionCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.dispatch != "not_started"
            || !canonical(&self.candidate_order)
            || !unique(&self.selected_order)
            || !unique(&self.selected_source_order)
            || !canonical(&self.deferred_source_order)
            || !canonical(&self.blocked_source_order)
            || !canonical(&self.omitted_source_order)
            || self.selection.candidate_order != self.candidate_order
        {
            return Err(KnowledgeActionSelectionCycleError::InvalidOutput(
                "selection-cycle identity, planning boundary, or ordering is invalid".into(),
            ));
        }
        self.selection.validate().map_err(|error| {
            KnowledgeActionSelectionCycleError::InvalidOutput(error.to_string())
        })?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeActionSelectionCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeActionSelectionCycleError::InvalidOutput(
                "selection-cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Bridge the knowledge action plan and select its next bounded portfolio. Completed source ids
/// are translated into the selector namespace before selection, preventing already executed
/// research work from being re-admitted under a new wrapper id.
pub fn execute_glioma_knowledge_selection_cycle(
    request: &KnowledgeActionSelectionCycleRequest,
    plan: &KnowledgeActionPlan,
) -> Result<KnowledgeActionSelectionCycle, KnowledgeActionSelectionCycleError> {
    if request.objective.trim().is_empty()
        || request.objective != plan.objective
        || request.plan_digest != plan.digest
    {
        return Err(KnowledgeActionSelectionCycleError::InvalidRequest(
            "objective and plan digest must match the knowledge-action plan".into(),
        ));
    }
    let bridge = bridge_glioma_knowledge_actions(
        &KnowledgeActionBridgeRequest {
            objective: request.objective.clone(),
            plan_digest: request.plan_digest.clone(),
            max_candidates: request.max_candidates,
        },
        plan,
    )?;
    if bridge.candidates.is_empty() {
        return Err(KnowledgeActionSelectionCycleError::InvalidInput(
            "knowledge plan produced no selector candidates after blocked/omitted gates".into(),
        ));
    }
    let candidates = bridge
        .candidates
        .iter()
        .map(|entry| entry.candidate.clone())
        .collect::<Vec<_>>();
    let completed = request
        .completed_source_action_ids
        .iter()
        .map(|id| format!("knowledge-action:{id}"))
        .collect::<BTreeSet<_>>();
    let selection = select_glioma_actions(&candidates, &completed, &request.selection_config)
        .map_err(|error| KnowledgeActionSelectionCycleError::Selector(error.to_string()))?;
    let selected_source_order = source_ids(&selection.selected_order)?;
    let deferred_source_order = source_ids(&selection.deferred_order)?;
    let blocked_source_order = source_ids(&selection.blocked_order)?;
    let mut output = KnowledgeActionSelectionCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        plan_digest: request.plan_digest.clone(),
        bridge_digest: bridge.digest.clone(),
        candidate_order: bridge.candidate_order.clone(),
        selected_order: selection.selected_order.clone(),
        selected_source_order,
        deferred_source_order,
        blocked_source_order,
        omitted_source_order: bridge.omitted_order.clone(),
        selection,
        dispatch: "not_started".into(),
        digest: ContentHash::of_bytes(b"unsealed-knowledge-selection-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeActionSelectionCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p02_evidence_knowledge::action_compiler::{
        digest_input as plan_digest_input, CompiledActionDisposition, CompiledResearchAction,
        KnowledgeActionPlanDisposition,
    };
    use crate::glioma::programs::p02_evidence_knowledge::claim_frontier::FrontierActionKind;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};

    fn plan() -> KnowledgeActionPlan {
        let mut plan = KnowledgeActionPlan {
            feature_id: "GAF-GLIOMA-P02-F16".into(),
            output_schema: "GliomaKnowledgeActionCompiler1@1".into(),
            objective: "select invasion validation".into(),
            knowledge_digest: ContentHash::of_bytes(b"knowledge"),
            frontier_digest: ContentHash::of_bytes(b"frontier"),
            action_order: vec!["validate-invasion".into()],
            selected_order: vec!["validate-invasion".into()],
            deferred_order: Vec::new(),
            blocked_order: Vec::new(),
            actions: vec![CompiledResearchAction {
                action_id: "validate-invasion".into(),
                claim_id: "claim-invasion".into(),
                action_kind: FrontierActionKind::ValidateSupported,
                modality_order: vec![GliomaModality::Imaging],
                model_system_order: vec![GliomaModelSystem::Organoid],
                information_gain_milli: 800,
                cost_milli: 100,
                risk_milli: 100,
                dependency_order: Vec::new(),
                priority_milli: 900,
                disposition: CompiledActionDisposition::Selected,
                reason: "independent validation admitted".into(),
            }],
            selected_cost_milli: 100,
            selected_risk_milli: 100,
            selected_information_milli: 800,
            negative_evidence_order: Vec::new(),
            uncertainty_order: Vec::new(),
            disposition: KnowledgeActionPlanDisposition::Qualified,
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        plan.digest = ContentHash::of_value(&plan_digest_input(&plan)).expect("plan digest");
        plan
    }

    #[test]
    fn selection_cycle_admits_knowledge_action_once() {
        let plan = plan();
        let cycle = execute_glioma_knowledge_selection_cycle(
            &KnowledgeActionSelectionCycleRequest {
                objective: plan.objective.clone(),
                plan_digest: plan.digest.clone(),
                max_candidates: 4,
                completed_source_action_ids: BTreeSet::new(),
                selection_config: GliomaSelectionConfig::default(),
            },
            &plan,
        )
        .expect("selection cycle");
        assert_eq!(cycle.selected_source_order, vec!["validate-invasion"]);
        assert_eq!(cycle.dispatch, "not_started");
        assert!(cycle.validate().is_ok());
    }

    #[test]
    fn completed_knowledge_action_is_not_reselected() {
        let plan = plan();
        let cycle = execute_glioma_knowledge_selection_cycle(
            &KnowledgeActionSelectionCycleRequest {
                objective: plan.objective.clone(),
                plan_digest: plan.digest.clone(),
                max_candidates: 4,
                completed_source_action_ids: BTreeSet::from(["validate-invasion".into()]),
                selection_config: GliomaSelectionConfig::default(),
            },
            &plan,
        )
        .expect("completed action remains an explicit selector block");
        assert!(cycle.selected_source_order.is_empty());
        assert_eq!(cycle.blocked_source_order, vec!["validate-invasion"]);
    }
}
