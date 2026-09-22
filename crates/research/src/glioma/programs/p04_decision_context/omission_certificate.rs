//! Deterministic omission certification for question-to-decision contexts.
//!
//! A decision context is not complete merely because it contains claims or actions.  This
//! capability audits the requested claim, modality, and model-system closure against the compiled
//! context and its dependency graph.  It keeps omitted, blocked, and unmeasured coverage
//! separate, emits the smallest bounded next-action set, and never upgrades planning coverage into
//! a scientific result.

use super::action_graph::{DecisionActionGraph, DecisionActionGraphDisposition};
use super::context_compiler::DecisionContext;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F11";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionOmissionCertificate1@1";
pub const MAX_REQUIREMENTS: usize = 256;
pub const MAX_NEXT_ACTIONS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionOmissionCertificateRequest {
    pub objective: String,
    pub required_claim_order: Vec<String>,
    pub required_modality_order: Vec<GliomaModality>,
    pub required_model_system_order: Vec<GliomaModelSystem>,
    pub require_dependency_closed: bool,
    pub max_next_actions: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionCoverageState {
    Closed,
    Omitted,
    Blocked,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionOmissionEntry {
    pub key: String,
    pub state: DecisionCoverageState,
    pub supporting_action_order: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionOmissionDisposition {
    Qualified,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionOmissionCertificate {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub context_digest: ContentHash,
    pub graph_digest: ContentHash,
    pub entries: Vec<DecisionOmissionEntry>,
    pub closed_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unmeasured_order: Vec<String>,
    pub next_action_order: Vec<String>,
    pub completeness_milli: u16,
    pub disposition: DecisionOmissionDisposition,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionOmissionCertificateError {
    #[error("decision-omission request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision-omission input is invalid: {0}")]
    InvalidInput(String),
    #[error("decision-omission output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision-omission digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &DecisionOmissionCertificate) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "context_digest": output.context_digest,
        "graph_digest": output.graph_digest,
        "entries": output.entries,
        "closed_order": output.closed_order,
        "omission_order": output.omission_order,
        "blocked_order": output.blocked_order,
        "unmeasured_order": output.unmeasured_order,
        "next_action_order": output.next_action_order,
        "completeness_milli": output.completeness_milli,
        "disposition": output.disposition,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
    })
}

fn modality_key(modality: GliomaModality) -> String {
    serde_json::to_value(modality)
        .expect("GliomaModality is serializable")
        .as_str()
        .expect("GliomaModality serializes as a string")
        .to_owned()
}

fn model_system_key(model_system: GliomaModelSystem) -> String {
    serde_json::to_value(model_system)
        .expect("GliomaModelSystem is serializable")
        .as_str()
        .expect("GliomaModelSystem serializes as a string")
        .to_owned()
}

fn validate_request(
    request: &DecisionOmissionCertificateRequest,
) -> Result<(), DecisionOmissionCertificateError> {
    let requirement_count = request.required_claim_order.len()
        + request.required_modality_order.len()
        + request.required_model_system_order.len();
    if request.objective.trim().is_empty()
        || requirement_count == 0
        || requirement_count > MAX_REQUIREMENTS
        || request.max_next_actions == 0
        || request.max_next_actions > MAX_NEXT_ACTIONS
        || request
            .required_claim_order
            .iter()
            .any(|id| id.trim().is_empty())
        || !canonical(&request.required_claim_order)
        || !canonical(&request.required_modality_order)
        || !canonical(&request.required_model_system_order)
    {
        return Err(DecisionOmissionCertificateError::InvalidRequest(
            "objective, at least one bounded requirement, canonical requirement ordering, and a bounded next-action count are required".into(),
        ));
    }
    Ok(())
}

impl DecisionOmissionCertificate {
    pub fn validate(&self) -> Result<(), DecisionOmissionCertificateError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.context_digest.as_str().len() != 64
            || self.graph_digest.as_str().len() != 64
            || !canonical(
                &self
                    .entries
                    .iter()
                    .map(|entry| entry.key.clone())
                    .collect::<Vec<_>>(),
            )
            || !canonical(&self.closed_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.unmeasured_order)
            || !canonical(&self.next_action_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.completeness_milli > 1_000
            || self.entries.iter().any(|entry| {
                entry.key.trim().is_empty()
                    || entry.reason.trim().is_empty()
                    || !canonical(&entry.supporting_action_order)
            })
        {
            return Err(DecisionOmissionCertificateError::InvalidOutput(
                "identity, ordering, bounded completeness, and entry invariants are invalid".into(),
            ));
        }
        let entry_states = self
            .entries
            .iter()
            .map(|entry| (entry.key.as_str(), entry.state))
            .collect::<BTreeMap<_, _>>();
        let state_order = [
            (DecisionCoverageState::Closed, &self.closed_order),
            (DecisionCoverageState::Omitted, &self.omission_order),
            (DecisionCoverageState::Blocked, &self.blocked_order),
            (DecisionCoverageState::Unmeasured, &self.unmeasured_order),
        ];
        let mut seen = BTreeSet::new();
        for (state, order) in state_order {
            for key in order {
                if entry_states.get(key.as_str()) != Some(&state) || !seen.insert(key) {
                    return Err(DecisionOmissionCertificateError::InvalidOutput(
                        "state indexes do not partition entries exactly".into(),
                    ));
                }
            }
        }
        if seen.len() != self.entries.len()
            || self.next_action_order.iter().any(|id| id.trim().is_empty())
            || self.next_action_order.len() > MAX_NEXT_ACTIONS
        {
            return Err(DecisionOmissionCertificateError::InvalidOutput(
                "state indexes or next actions are incomplete or unbounded".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionOmissionCertificateError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionOmissionCertificateError::InvalidOutput(
                "digest is not bound to the omission certificate".into(),
            ));
        }
        Ok(())
    }
}

/// Audit requested coverage against an immutable decision context and its action graph.
pub fn certify_decision_omissions(
    request: &DecisionOmissionCertificateRequest,
    context: &DecisionContext,
    graph: &DecisionActionGraph,
) -> Result<DecisionOmissionCertificate, DecisionOmissionCertificateError> {
    validate_request(request)?;
    context
        .validate()
        .map_err(|error| DecisionOmissionCertificateError::InvalidInput(error.to_string()))?;
    graph
        .validate()
        .map_err(|error| DecisionOmissionCertificateError::InvalidInput(error.to_string()))?;
    if request.objective != context.objective
        || request.objective != graph.objective
        || context.digest != graph.context_digest
    {
        return Err(DecisionOmissionCertificateError::InvalidRequest(
            "request objective and graph context digest must match the decision context".into(),
        ));
    }

    let context_actions = context
        .actions
        .iter()
        .map(|action| (action.claim_id.as_str(), action))
        .collect::<BTreeMap<_, _>>();
    let graph_by_action = graph
        .nodes
        .iter()
        .map(|node| (node.action_id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let graph_by_claim = graph
        .nodes
        .iter()
        .map(|node| (node.claim_id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let graph_blocked = matches!(
        graph.disposition,
        DecisionActionGraphDisposition::BudgetBlocked
    );
    let graph_unresolved = matches!(
        graph.disposition,
        DecisionActionGraphDisposition::Unresolved
    );
    let mut entries = BTreeMap::<String, DecisionOmissionEntry>::new();

    for claim_id in &request.required_claim_order {
        let key = format!("claim:{claim_id}");
        let Some(action) = context_actions.get(claim_id.as_str()) else {
            let state = if context.claim_order.binary_search(claim_id).is_ok() {
                DecisionCoverageState::Omitted
            } else {
                DecisionCoverageState::Unmeasured
            };
            entries.insert(
                key,
                DecisionOmissionEntry {
                    key: format!("claim:{claim_id}"),
                    state,
                    supporting_action_order: Vec::new(),
                    reason: if state == DecisionCoverageState::Omitted {
                        "claim is in the typed context but no bounded decision action was retained"
                            .into()
                    } else {
                        "claim is not represented in the compiled typed context".into()
                    },
                },
            );
            continue;
        };
        let state = if graph_blocked {
            DecisionCoverageState::Blocked
        } else if graph_unresolved || !graph_by_claim.contains_key(claim_id.as_str()) {
            DecisionCoverageState::Omitted
        } else {
            DecisionCoverageState::Closed
        };
        entries.insert(
            key.clone(),
            DecisionOmissionEntry {
                key,
                state,
                supporting_action_order: vec![action.action_id.clone()],
                reason: match state {
                    DecisionCoverageState::Closed => {
                        "claim has a graph-represented typed action".into()
                    }
                    DecisionCoverageState::Blocked => {
                        "claim action is represented but the graph is budget-blocked".into()
                    }
                    DecisionCoverageState::Omitted => {
                        "claim action exists but is not in the selected dependency graph".into()
                    }
                    DecisionCoverageState::Unmeasured => unreachable!(),
                },
            },
        );
    }

    let modality_set = context
        .actions
        .iter()
        .filter(|action| graph_by_action.contains_key(action.action_id.as_str()))
        .map(|action| action.target_modality)
        .collect::<BTreeSet<_>>();
    for modality in &request.required_modality_order {
        let key = format!("modality:{}", modality_key(*modality));
        let state = if modality_set.contains(modality) {
            if graph_blocked {
                DecisionCoverageState::Blocked
            } else {
                DecisionCoverageState::Closed
            }
        } else if context
            .actions
            .iter()
            .any(|action| action.target_modality == *modality)
        {
            DecisionCoverageState::Omitted
        } else {
            DecisionCoverageState::Unmeasured
        };
        let supporting_action_order = context
            .actions
            .iter()
            .filter(|action| action.target_modality == *modality)
            .map(|action| action.action_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        entries.insert(
            key.clone(),
            DecisionOmissionEntry {
                key,
                state,
                supporting_action_order,
                reason: match state {
                    DecisionCoverageState::Closed => {
                        "required modality is represented by a graph action".into()
                    }
                    DecisionCoverageState::Blocked => {
                        "required modality is represented but graph execution is budget-blocked"
                            .into()
                    }
                    DecisionCoverageState::Omitted => {
                        "required modality has context actions outside the selected graph".into()
                    }
                    DecisionCoverageState::Unmeasured => {
                        "no context action targets the required modality".into()
                    }
                },
            },
        );
    }

    let model_set = context
        .actions
        .iter()
        .filter(|action| graph_by_action.contains_key(action.action_id.as_str()))
        .map(|action| action.target_model_system)
        .collect::<BTreeSet<_>>();
    for model_system in &request.required_model_system_order {
        let key = format!("model_system:{}", model_system_key(*model_system));
        let state = if model_set.contains(model_system) {
            if graph_blocked {
                DecisionCoverageState::Blocked
            } else {
                DecisionCoverageState::Closed
            }
        } else if context
            .actions
            .iter()
            .any(|action| action.target_model_system == *model_system)
        {
            DecisionCoverageState::Omitted
        } else {
            DecisionCoverageState::Unmeasured
        };
        let supporting_action_order = context
            .actions
            .iter()
            .filter(|action| action.target_model_system == *model_system)
            .map(|action| action.action_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        entries.insert(
            key.clone(),
            DecisionOmissionEntry {
                key,
                state,
                supporting_action_order,
                reason: match state {
                    DecisionCoverageState::Closed => {
                        "required model system is represented by a graph action".into()
                    }
                    DecisionCoverageState::Blocked => {
                        "required model system is represented but graph execution is budget-blocked"
                            .into()
                    }
                    DecisionCoverageState::Omitted => {
                        "required model system has context actions outside the selected graph"
                            .into()
                    }
                    DecisionCoverageState::Unmeasured => {
                        "no context action targets the required model system".into()
                    }
                },
            },
        );
    }

    if request.require_dependency_closed {
        for claim_id in &graph.missing_claim_order {
            let key = format!("dependency:missing-claim:{claim_id}");
            entries.insert(
                key.clone(),
                DecisionOmissionEntry {
                    key,
                    state: DecisionCoverageState::Blocked,
                    supporting_action_order: Vec::new(),
                    reason: "selected claim path references a claim without a typed action".into(),
                },
            );
        }
        for path_id in &graph.unresolved_path_order {
            let key = format!("dependency:unresolved-path:{path_id}");
            entries.insert(
                key.clone(),
                DecisionOmissionEntry {
                    key,
                    state: DecisionCoverageState::Blocked,
                    supporting_action_order: Vec::new(),
                    reason: "selected claim path is not qualified for dependency-closed execution"
                        .into(),
                },
            );
        }
    }

    let entries = entries.into_values().collect::<Vec<_>>();
    let mut closed_order = Vec::new();
    let mut omission_order = Vec::new();
    let mut blocked_order = Vec::new();
    let mut unmeasured_order = Vec::new();
    let mut uncertainty_order = graph.uncertainty_order.clone();
    for entry in &entries {
        match entry.state {
            DecisionCoverageState::Closed => closed_order.push(entry.key.clone()),
            DecisionCoverageState::Omitted => omission_order.push(entry.key.clone()),
            DecisionCoverageState::Blocked => {
                blocked_order.push(entry.key.clone());
                uncertainty_order.push(entry.key.clone());
            }
            DecisionCoverageState::Unmeasured => {
                unmeasured_order.push(entry.key.clone());
                uncertainty_order.push(entry.key.clone());
            }
        }
    }
    uncertainty_order.sort();
    uncertainty_order.dedup();
    let mut next_action_order = entries
        .iter()
        .filter(|entry| entry.state != DecisionCoverageState::Closed)
        .flat_map(|entry| {
            if entry.supporting_action_order.is_empty() {
                vec![format!("close:{}", entry.key)]
            } else {
                entry.supporting_action_order.clone()
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    next_action_order.truncate(request.max_next_actions);
    let total = entries.len() as u32;
    let completeness_milli = if total == 0 {
        0
    } else {
        ((closed_order.len() as u32 * 1_000) / total).min(1_000) as u16
    };
    let disposition =
        if blocked_order.is_empty() && omission_order.is_empty() && unmeasured_order.is_empty() {
            DecisionOmissionDisposition::Qualified
        } else if !closed_order.is_empty() {
            if !blocked_order.is_empty() {
                DecisionOmissionDisposition::Blocked
            } else {
                DecisionOmissionDisposition::Partial
            }
        } else if !blocked_order.is_empty() {
            DecisionOmissionDisposition::Blocked
        } else {
            DecisionOmissionDisposition::Unresolved
        };
    let mut output = DecisionOmissionCertificate {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        context_digest: context.digest.clone(),
        graph_digest: graph.digest.clone(),
        entries,
        closed_order,
        omission_order,
        blocked_order,
        unmeasured_order,
        next_action_order,
        completeness_milli,
        disposition,
        negative_evidence_order: graph.negative_evidence_order.clone(),
        uncertainty_order,
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-omission-certificate"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DecisionOmissionCertificateError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p04_decision_context::action_graph::{
        DecisionActionGraph, DecisionGraphNode,
    };
    use crate::glioma::programs::p04_decision_context::context_compiler::{
        DecisionAction, DecisionActionKind, DecisionContext, DecisionContextDisposition,
    };
    use crate::glioma_engine::{GliomaActionCandidate, GliomaStageKind};
    use bioprism_foundation::{AutonomyTier, Effect};

    fn fixture(budget_blocked: bool) -> (DecisionContext, DecisionActionGraph) {
        let candidate = GliomaActionCandidate {
            action_id: "decision-egfr".into(),
            stage_kind: GliomaStageKind::MechanismExploration,
            modality: GliomaModality::Genomics,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            cost_units: 5,
            information_gain_milli: 700,
            frontier_novelty_milli: 700,
            workflow_leverage_milli: 700,
            cross_stage_unlock_milli: 700,
            reproducibility_safety_milli: 900,
            federation_value_milli: 500,
            feasibility_milli: 800,
            autonomy_tier: AutonomyTier::A1,
            effects: BTreeSet::from([
                Effect::ReadLocalData,
                Effect::ExecuteLocalComputation,
                Effect::WriteLocalArtifact,
            ]),
        };
        let action = DecisionAction {
            action_id: candidate.action_id.clone(),
            claim_id: "claim-egfr".into(),
            kind: DecisionActionKind::ValidateMechanism,
            rationale: "validate mechanism".into(),
            target_modality: GliomaModality::Genomics,
            target_model_system: GliomaModelSystem::Organoid,
            priority_milli: 700,
            candidate: candidate.clone(),
        };
        let mut context = DecisionContext {
            feature_id: super::super::context_compiler::FEATURE_ID.into(),
            output_schema: super::super::context_compiler::OUTPUT_SCHEMA.into(),
            objective: "egfr invasion".into(),
            claim_order: vec!["claim-egfr".into()],
            actions: vec![action],
            action_order: vec!["decision-egfr".into()],
            deferred_action_order: Vec::new(),
            omission_order: Vec::new(),
            negative_evidence_order: Vec::new(),
            uncertainty_order: Vec::new(),
            disposition: DecisionContextDisposition::Qualified,
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        let context_digest = serde_json::json!({
            "feature_id": context.feature_id,
            "output_schema": context.output_schema,
            "objective": context.objective,
            "claim_order": context.claim_order,
            "actions": context.actions,
            "action_order": context.action_order,
            "deferred_action_order": context.deferred_action_order,
            "omission_order": context.omission_order,
            "negative_evidence_order": context.negative_evidence_order,
            "uncertainty_order": context.uncertainty_order,
            "disposition": context.disposition,
        });
        context.digest = ContentHash::of_value(&context_digest).unwrap();
        let node = DecisionGraphNode {
            action_id: candidate.action_id.clone(),
            claim_id: "claim-egfr".into(),
            stage_kind: candidate.stage_kind,
            action: candidate,
            dependency_order: Vec::new(),
            path_order: vec!["path-egfr".into()],
            wave: 0,
            cumulative_cost_units: 5,
        };
        let disposition = if budget_blocked {
            DecisionActionGraphDisposition::BudgetBlocked
        } else {
            DecisionActionGraphDisposition::Qualified
        };
        let mut graph = DecisionActionGraph {
            feature_id: super::super::action_graph::FEATURE_ID.into(),
            output_schema: super::super::action_graph::OUTPUT_SCHEMA.into(),
            objective: "egfr invasion".into(),
            context_digest: context.digest.clone(),
            composition_digest: ContentHash::of_bytes(b"composition"),
            nodes: vec![node],
            node_order: vec!["decision-egfr".into()],
            topological_order: vec!["decision-egfr".into()],
            parallel_waves: vec![vec!["decision-egfr".into()]],
            missing_claim_order: Vec::new(),
            unresolved_path_order: Vec::new(),
            critical_path_units: 5,
            total_cost_units: if budget_blocked { 10 } else { 5 },
            bottleneck_action_order: vec!["decision-egfr".into()],
            negative_evidence_order: Vec::new(),
            uncertainty_order: Vec::new(),
            disposition,
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        let graph_digest = serde_json::json!({
            "feature_id": graph.feature_id,
            "output_schema": graph.output_schema,
            "objective": graph.objective,
            "context_digest": graph.context_digest,
            "composition_digest": graph.composition_digest,
            "nodes": graph.nodes,
            "node_order": graph.node_order,
            "topological_order": graph.topological_order,
            "parallel_waves": graph.parallel_waves,
            "missing_claim_order": graph.missing_claim_order,
            "unresolved_path_order": graph.unresolved_path_order,
            "critical_path_units": graph.critical_path_units,
            "total_cost_units": graph.total_cost_units,
            "bottleneck_action_order": graph.bottleneck_action_order,
            "negative_evidence_order": graph.negative_evidence_order,
            "uncertainty_order": graph.uncertainty_order,
            "disposition": graph.disposition,
        });
        graph.digest = ContentHash::of_value(&graph_digest).unwrap();
        (context, graph)
    }

    #[test]
    fn closes_claim_modality_and_model_coverage() {
        let (context, graph) = fixture(false);
        let certificate = certify_decision_omissions(
            &DecisionOmissionCertificateRequest {
                objective: "egfr invasion".into(),
                required_claim_order: vec!["claim-egfr".into()],
                required_modality_order: vec![GliomaModality::Genomics],
                required_model_system_order: vec![GliomaModelSystem::Organoid],
                require_dependency_closed: true,
                max_next_actions: 8,
            },
            &context,
            &graph,
        )
        .unwrap();
        assert_eq!(certificate.completeness_milli, 1_000);
        assert_eq!(
            certificate.disposition,
            DecisionOmissionDisposition::Qualified
        );
        certificate.validate().unwrap();
    }

    #[test]
    fn preserves_unmeasured_and_budget_blocked_states() {
        let (context, graph) = fixture(true);
        let certificate = certify_decision_omissions(
            &DecisionOmissionCertificateRequest {
                objective: "egfr invasion".into(),
                required_claim_order: vec!["claim-egfr".into(), "claim-missing".into()],
                required_modality_order: vec![GliomaModality::Spatial],
                required_model_system_order: vec![GliomaModelSystem::MouseModel],
                require_dependency_closed: true,
                max_next_actions: 8,
            },
            &context,
            &graph,
        )
        .unwrap();
        assert!(certificate
            .entries
            .iter()
            .any(|entry| entry.state == DecisionCoverageState::Blocked));
        assert!(certificate
            .entries
            .iter()
            .any(|entry| entry.state == DecisionCoverageState::Unmeasured));
        assert_eq!(
            certificate.disposition,
            DecisionOmissionDisposition::Blocked
        );
    }

    #[test]
    fn requirement_permutation_is_rejected_until_canonicalized() {
        let (context, graph) = fixture(false);
        let error = certify_decision_omissions(
            &DecisionOmissionCertificateRequest {
                objective: "egfr invasion".into(),
                required_claim_order: vec!["claim-z".into(), "claim-a".into()],
                required_modality_order: vec![GliomaModality::Genomics],
                required_model_system_order: vec![GliomaModelSystem::Organoid],
                require_dependency_closed: false,
                max_next_actions: 8,
            },
            &context,
            &graph,
        )
        .unwrap_err();
        assert!(error.to_string().contains("canonical"));
    }
}
