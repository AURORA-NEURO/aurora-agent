//! Dependency-closed decision-action graph compilation for preclinical glioma research.
//!
//! P04's context compiler produces useful actions, while P02 composition produces explicit claim
//! paths. This module joins them into an execution-ready DAG: upstream claim actions precede
//! downstream claims, independent branches are grouped into deterministic waves, and missing
//! claims or budget shortfalls remain visible. The graph is planning-only; a caller-owned selector
//! or executor still controls every effect.

use super::context_compiler::DecisionContext;
use crate::glioma::programs::p02_evidence_knowledge::{
    KnowledgeComposition, KnowledgeCompositionDisposition,
};
use crate::glioma_engine::{GliomaActionCandidate, GliomaStageKind};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F10";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionActionGraph1@1";
pub const MAX_NODES: usize = 256;
pub const MAX_WAVES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionActionGraphRequest {
    pub objective: String,
    pub max_nodes: usize,
    pub max_waves: usize,
    pub budget_units: u32,
    pub require_qualified_composition: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionGraphNode {
    pub action_id: String,
    pub claim_id: String,
    pub stage_kind: GliomaStageKind,
    pub action: GliomaActionCandidate,
    pub dependency_order: Vec<String>,
    pub path_order: Vec<String>,
    pub wave: usize,
    pub cumulative_cost_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionActionGraphDisposition {
    Qualified,
    Partial,
    Unresolved,
    BudgetBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionActionGraph {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub context_digest: ContentHash,
    pub composition_digest: ContentHash,
    pub nodes: Vec<DecisionGraphNode>,
    pub node_order: Vec<String>,
    pub topological_order: Vec<String>,
    pub parallel_waves: Vec<Vec<String>>,
    pub missing_claim_order: Vec<String>,
    pub unresolved_path_order: Vec<String>,
    pub critical_path_units: u32,
    pub total_cost_units: u32,
    pub bottleneck_action_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: DecisionActionGraphDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionActionGraphError {
    #[error("decision-action graph request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision-action graph input is invalid: {0}")]
    InvalidInput(String),
    #[error("decision-action graph output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision-action graph digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &DecisionActionGraph) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "context_digest": output.context_digest,
        "composition_digest": output.composition_digest,
        "nodes": output.nodes,
        "node_order": output.node_order,
        "topological_order": output.topological_order,
        "parallel_waves": output.parallel_waves,
        "missing_claim_order": output.missing_claim_order,
        "unresolved_path_order": output.unresolved_path_order,
        "critical_path_units": output.critical_path_units,
        "total_cost_units": output.total_cost_units,
        "bottleneck_action_order": output.bottleneck_action_order,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &DecisionActionGraphRequest) -> Result<(), DecisionActionGraphError> {
    if request.objective.trim().is_empty()
        || request.max_nodes == 0
        || request.max_nodes > MAX_NODES
        || request.max_waves == 0
        || request.max_waves > MAX_WAVES
        || request.budget_units == 0
    {
        return Err(DecisionActionGraphError::InvalidRequest(
            "objective, bounded node/wave counts, and positive budget are required".into(),
        ));
    }
    Ok(())
}

impl DecisionActionGraph {
    pub fn validate(&self) -> Result<(), DecisionActionGraphError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.context_digest.as_str().len() != 64
            || self.composition_digest.as_str().len() != 64
            || !canonical(&self.node_order)
            || !canonical(&self.topological_order)
            || !canonical(&self.missing_claim_order)
            || !canonical(&self.unresolved_path_order)
            || !canonical(&self.bottleneck_action_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.nodes.len() != self.node_order.len()
            || self.nodes.iter().any(|node| {
                node.action_id != node.action.action_id
                    || node.action_id.trim().is_empty()
                    || node.claim_id.trim().is_empty()
                    || !canonical(&node.dependency_order)
                    || !canonical(&node.path_order)
                    || node.wave >= MAX_WAVES
                    || node.action.cost_units == 0
                    || node.cumulative_cost_units < node.action.cost_units
            })
            || self.parallel_waves.len() > MAX_WAVES
            || self.parallel_waves.iter().any(|wave| !canonical(wave))
        {
            return Err(DecisionActionGraphError::InvalidOutput(
                "identity, node ordering, dependency, wave, cost, or score invariants are invalid"
                    .into(),
            ));
        }
        let node_ids = self.node_order.iter().collect::<BTreeSet<_>>();
        if self
            .nodes
            .iter()
            .map(|node| &node.action_id)
            .collect::<BTreeSet<_>>()
            != node_ids
            || self
                .topological_order
                .iter()
                .any(|id| !node_ids.contains(id))
            || self
                .parallel_waves
                .iter()
                .flatten()
                .any(|id| !node_ids.contains(id))
            || self
                .nodes
                .iter()
                .flat_map(|node| node.dependency_order.iter())
                .any(|id| !node_ids.contains(id))
        {
            return Err(DecisionActionGraphError::InvalidOutput(
                "graph references an unknown or duplicate node".into(),
            ));
        }
        if self
            .parallel_waves
            .iter()
            .flatten()
            .collect::<BTreeSet<_>>()
            != node_ids
        {
            return Err(DecisionActionGraphError::InvalidOutput(
                "parallel waves do not cover the graph exactly once".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionActionGraphError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionActionGraphError::InvalidOutput(
                "decision graph digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

struct TopologySummary {
    topological_order: Vec<String>,
    parallel_waves: Vec<Vec<String>>,
    critical_path_units: u32,
    bottleneck_action_order: Vec<String>,
}

fn topo_and_waves(
    nodes: &BTreeMap<String, DecisionGraphNode>,
    max_waves: usize,
) -> Result<TopologySummary, DecisionActionGraphError> {
    let mut indegree = nodes
        .iter()
        .map(|(id, node)| (id.clone(), node.dependency_order.len()))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<String, Vec<String>>::new();
    for node in nodes.values() {
        for dependency in &node.dependency_order {
            outgoing
                .entry(dependency.clone())
                .or_default()
                .push(node.action_id.clone());
        }
    }
    for successors in outgoing.values_mut() {
        successors.sort();
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut topological_order = Vec::new();
    let mut waves = Vec::new();
    let mut cumulative_by_id = BTreeMap::<String, u32>::new();
    while !ready.is_empty() {
        let current = ready.iter().cloned().collect::<Vec<_>>();
        ready.clear();
        let wave_index = waves.len();
        if wave_index >= max_waves {
            return Err(DecisionActionGraphError::InvalidInput(
                "dependency graph exceeds the configured wave bound".into(),
            ));
        }
        let mut current_wave = Vec::new();
        for id in current {
            let node = nodes.get(&id).ok_or_else(|| {
                DecisionActionGraphError::InvalidOutput("topological node is missing".into())
            })?;
            let dependency_cost = node
                .dependency_order
                .iter()
                .filter_map(|dependency| cumulative_by_id.get(dependency))
                .copied()
                .max()
                .unwrap_or(0);
            cumulative_by_id.insert(
                id.clone(),
                dependency_cost.saturating_add(node.action.cost_units),
            );
            current_wave.push(id.clone());
            topological_order.push(id.clone());
            for successor in outgoing.get(&id).into_iter().flatten() {
                let degree = indegree.get_mut(successor).ok_or_else(|| {
                    DecisionActionGraphError::InvalidOutput("edge target is missing".into())
                })?;
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    ready.insert(successor.clone());
                }
            }
        }
        current_wave.sort();
        waves.push(current_wave);
    }
    if topological_order.len() != nodes.len() {
        return Err(DecisionActionGraphError::InvalidInput(
            "claim-path action graph contains a dependency cycle".into(),
        ));
    }
    let critical_path_units = cumulative_by_id.values().copied().max().unwrap_or(0);
    let bottleneck_action_order = cumulative_by_id
        .iter()
        .filter(|(_, cost)| **cost == critical_path_units)
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    Ok(TopologySummary {
        topological_order,
        parallel_waves: waves,
        critical_path_units,
        bottleneck_action_order,
    })
}

/// Compile composed claim paths into a dependency-closed action DAG with deterministic parallel
/// waves. The output can be handed to the existing selector after a caller applies its policy.
pub fn compile_decision_action_graph(
    request: &DecisionActionGraphRequest,
    context: &DecisionContext,
    composition: &KnowledgeComposition,
) -> Result<DecisionActionGraph, DecisionActionGraphError> {
    validate_request(request)?;
    context
        .validate()
        .map_err(|error| DecisionActionGraphError::InvalidInput(error.to_string()))?;
    composition
        .validate()
        .map_err(|error| DecisionActionGraphError::InvalidInput(error.to_string()))?;
    if request.objective != context.objective || request.objective != composition.objective {
        return Err(DecisionActionGraphError::InvalidRequest(
            "request objective must match both context and composition".into(),
        ));
    }
    if request.require_qualified_composition
        && composition.disposition != KnowledgeCompositionDisposition::Qualified
    {
        return Err(DecisionActionGraphError::InvalidInput(
            "qualified composition is required before action graph compilation".into(),
        ));
    }
    let actions_by_claim = context
        .actions
        .iter()
        .map(|action| (action.claim_id.clone(), action))
        .collect::<BTreeMap<_, _>>();
    let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
    let mut paths_by_action = BTreeMap::<String, BTreeSet<String>>::new();
    let mut missing_claims = BTreeSet::new();
    let selected_paths = composition
        .paths
        .iter()
        .filter(|path| {
            composition
                .selected_path_order
                .binary_search(&path.path_id)
                .is_ok()
        })
        .collect::<Vec<_>>();
    for path in selected_paths {
        for claim_id in &path.claim_order {
            let Some(action) = actions_by_claim.get(claim_id) else {
                missing_claims.insert(claim_id.clone());
                continue;
            };
            paths_by_action
                .entry(action.action_id.clone())
                .or_default()
                .insert(path.path_id.clone());
            dependencies.entry(action.action_id.clone()).or_default();
        }
        for pair in path.claim_order.windows(2) {
            let Some(from) = actions_by_claim.get(&pair[0]) else {
                missing_claims.insert(pair[0].clone());
                continue;
            };
            let Some(to) = actions_by_claim.get(&pair[1]) else {
                missing_claims.insert(pair[1].clone());
                continue;
            };
            dependencies
                .entry(to.action_id.clone())
                .or_default()
                .insert(from.action_id.clone());
        }
    }
    if dependencies.len() > request.max_nodes {
        return Err(DecisionActionGraphError::InvalidInput(
            "selected claim paths exceed the configured node bound".into(),
        ));
    }
    let mut nodes = BTreeMap::new();
    for (claim_id, action) in &actions_by_claim {
        let Some(dependency_set) = dependencies.get(&action.action_id) else {
            continue;
        };
        let mut candidate = action.candidate.clone();
        candidate.depends_on = dependency_set.iter().cloned().collect();
        nodes.insert(
            action.action_id.clone(),
            DecisionGraphNode {
                action_id: action.action_id.clone(),
                claim_id: claim_id.clone(),
                stage_kind: candidate.stage_kind,
                action: candidate,
                dependency_order: dependency_set.iter().cloned().collect(),
                path_order: paths_by_action
                    .get(&action.action_id)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect(),
                wave: 0,
                cumulative_cost_units: 0,
            },
        );
    }
    let topology = topo_and_waves(&nodes, request.max_waves)?;
    for (wave, ids) in topology.parallel_waves.iter().enumerate() {
        for id in ids {
            let dependency_cost = nodes
                .get(id)
                .ok_or_else(|| {
                    DecisionActionGraphError::InvalidOutput("wave references missing node".into())
                })?
                .dependency_order
                .iter()
                .filter_map(|dependency| {
                    nodes
                        .get(dependency)
                        .map(|parent| parent.cumulative_cost_units)
                })
                .max()
                .unwrap_or(0);
            let node = nodes.get_mut(id).ok_or_else(|| {
                DecisionActionGraphError::InvalidOutput("wave references missing node".into())
            })?;
            node.wave = wave;
            node.cumulative_cost_units = node.action.cost_units + dependency_cost;
        }
    }
    let total_cost_units = nodes
        .values()
        .map(|node| node.action.cost_units)
        .sum::<u32>();
    let unresolved_path_order = composition
        .paths
        .iter()
        .filter(|path| path.disposition != crate::glioma::programs::p02_evidence_knowledge::KnowledgePathDisposition::Qualified)
        .map(|path| path.path_id.clone())
        .collect::<Vec<_>>();
    let negative_evidence_order = composition.negative_evidence_order.clone();
    let mut uncertainty_order = composition.uncertainty_order.clone();
    uncertainty_order.extend(
        missing_claims
            .iter()
            .map(|claim| format!("missing-claim:{claim}")),
    );
    uncertainty_order.sort();
    uncertainty_order.dedup();
    let disposition = if nodes.is_empty() || !missing_claims.is_empty() {
        DecisionActionGraphDisposition::Unresolved
    } else if total_cost_units > request.budget_units {
        DecisionActionGraphDisposition::BudgetBlocked
    } else if !unresolved_path_order.is_empty() || !negative_evidence_order.is_empty() {
        DecisionActionGraphDisposition::Partial
    } else {
        DecisionActionGraphDisposition::Qualified
    };
    let node_order = nodes.keys().cloned().collect::<Vec<_>>();
    let mut output = DecisionActionGraph {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        context_digest: context.digest.clone(),
        composition_digest: composition.digest.clone(),
        nodes: nodes.into_values().collect(),
        node_order,
        topological_order: topology.topological_order,
        parallel_waves: topology.parallel_waves,
        missing_claim_order: missing_claims.into_iter().collect(),
        unresolved_path_order,
        critical_path_units: topology.critical_path_units,
        total_cost_units,
        bottleneck_action_order: topology.bottleneck_action_order,
        negative_evidence_order,
        uncertainty_order,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-action-graph"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DecisionActionGraphError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::composition::{
        compose_knowledge_graph, KnowledgeCompositionRequest, KnowledgeRelation,
        KnowledgeRelationKind,
    };
    use crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma::programs::p04_decision_context::compile_decision_context;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
    use bioprism_foundation::PRECLINICAL_BOUNDARY;
    use bioprism_ids::ContentHash;
    use std::collections::BTreeSet;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn context_and_composition() -> (DecisionContext, KnowledgeComposition) {
        let records = [
            EvidenceRecord {
                evidence_id: "e1".into(),
                source_artifact: artifact("e1"),
                source_kind: EvidenceSourceKind::Dataset,
                claim: "EGFR signaling increases invasion".into(),
                scope: "preclinical glioma".into(),
                modality: GliomaModality::Genomics,
                model_system: Some(GliomaModelSystem::Organoid),
                state: EvidenceState::Supported,
                relevance_milli: 900,
                quality_milli: 900,
                reproducibility_milli: 900,
                release_epoch: 1,
            },
            EvidenceRecord {
                evidence_id: "e2".into(),
                source_artifact: artifact("e2"),
                source_kind: EvidenceSourceKind::Dataset,
                claim: "invasion increases dissemination".into(),
                scope: "preclinical glioma".into(),
                modality: GliomaModality::Genomics,
                model_system: Some(GliomaModelSystem::Organoid),
                state: EvidenceState::Supported,
                relevance_milli: 900,
                quality_milli: 900,
                reproducibility_milli: 900,
                release_epoch: 1,
            },
        ];
        let knowledge = compile_typed_knowledge(
            &KnowledgeRequest {
                objective: "compose invasion mechanism".into(),
                required_modalities: BTreeSet::new(),
                required_model_systems: BTreeSet::new(),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &records,
        )
        .unwrap();
        let ids = knowledge.claim_order.clone();
        let composition = compose_knowledge_graph(
            &KnowledgeCompositionRequest {
                objective: "compose invasion mechanism".into(),
                min_path_length: 2,
                max_paths: 8,
                min_strength_milli: 500,
                max_contradiction_milli: 500,
                require_supported_root: true,
            },
            &knowledge,
            &[KnowledgeRelation {
                relation_id: "r1".into(),
                from_claim_id: ids[0].clone(),
                to_claim_id: ids[1].clone(),
                kind: KnowledgeRelationKind::Supports,
                strength_milli: 900,
            }],
        )
        .unwrap();
        let context = compile_decision_context(
            &crate::glioma::programs::p04_decision_context::DecisionContextRequest {
                objective: "compose invasion mechanism".into(),
                max_actions: 8,
                default_cost_units: 4,
            },
            &knowledge,
        )
        .unwrap();
        (context, composition)
    }

    #[test]
    fn compiles_path_dependencies_into_parallel_waves() {
        let (context, composition) = context_and_composition();
        let output = compile_decision_action_graph(
            &DecisionActionGraphRequest {
                objective: "compose invasion mechanism".into(),
                max_nodes: 8,
                max_waves: 8,
                budget_units: 100,
                require_qualified_composition: true,
            },
            &context,
            &composition,
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            DecisionActionGraphDisposition::Qualified
        );
        assert_eq!(output.nodes.len(), 2);
        assert_eq!(output.parallel_waves.len(), 2);
        assert_eq!(output.critical_path_units, 8);
        output.validate().unwrap();
    }

    #[test]
    fn budget_and_objective_gates_remain_explicit() {
        let (context, composition) = context_and_composition();
        let output = compile_decision_action_graph(
            &DecisionActionGraphRequest {
                objective: "compose invasion mechanism".into(),
                max_nodes: 8,
                max_waves: 8,
                budget_units: 1,
                require_qualified_composition: true,
            },
            &context,
            &composition,
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            DecisionActionGraphDisposition::BudgetBlocked
        );
        let error = compile_decision_action_graph(
            &DecisionActionGraphRequest {
                objective: "other objective".into(),
                max_nodes: 8,
                max_waves: 8,
                budget_units: 100,
                require_qualified_composition: true,
            },
            &context,
            &composition,
        )
        .unwrap_err();
        assert!(error.to_string().contains("objective"));
    }

    #[test]
    fn preclinical_boundary_fixture_is_constant() {
        assert!(PRECLINICAL_BOUNDARY.contains("preclinical"));
    }
}
