//! Clone-aware preclinical perturbation-panel design.
//!
//! This capability closes a useful loop after clonal-evolution inference: it chooses a bounded
//! set of typed perturbation or readout candidates that covers the most important evolutionary
//! branches for a declared budget. It is a design artifact only. It does not execute a drug,
//! edit a specimen, infer clinical resistance, or turn an unmeasured marker into a finding.

use crate::glioma::programs::p05_mechanism_exploration::ClonalEvolutionGraph;
use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F30";
pub const OUTPUT_SCHEMA: &str = "GliomaClonePerturbationPanel1@2";
pub const MAX_CANDIDATES: usize = 4_096;
pub const MAX_SELECTED: usize = 256;
pub const MAX_BRANCHES: usize = 8_192;
pub const MAX_BUDGET_MILLI: u64 = 1_000_000_000;
const PANEL_BEAM_WIDTH: usize = 96;
const MAX_PANEL_SEARCH_DEPTH: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClonePerturbationKind {
    Inhibit,
    Activate,
    Deplete,
    Rescue,
    Control,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonePerturbationCandidate {
    pub candidate_id: String,
    pub kind: ClonePerturbationKind,
    pub target_marker_order: Vec<String>,
    pub cost_milli: u64,
    pub expected_effect_milli: u16,
    pub purpose: String,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonePerturbationPanelRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub budget_milli: u64,
    pub min_coverage_milli: u16,
    pub max_selected: usize,
    pub require_branch_coverage: bool,
    pub allow_uncertain_targets: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneBranchCoverage {
    pub branch_id: String,
    pub parent_node_id: String,
    pub child_node_id: String,
    pub weight_milli: u32,
    pub target_marker_order: Vec<String>,
    pub unresolved_marker_order: Vec<String>,
    pub selected_candidate_order: Vec<String>,
    pub coverage_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonePerturbationDecision {
    pub candidate_id: String,
    pub kind: ClonePerturbationKind,
    pub selected: bool,
    pub covered_branch_order: Vec<String>,
    pub uncertain_branch_order: Vec<String>,
    pub marginal_gain_milli: u32,
    pub score_milli: u64,
    pub cost_milli: u64,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClonePerturbationPanelDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonePerturbationPanel {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub branch_order: Vec<String>,
    pub branch_coverage: Vec<CloneBranchCoverage>,
    pub decisions: Vec<ClonePerturbationDecision>,
    pub total_cost_milli: u64,
    pub weighted_coverage_milli: u16,
    pub uncovered_branch_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: ClonePerturbationPanelDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClonePerturbationPanelError {
    #[error("clone perturbation panel request is invalid: {0}")]
    InvalidRequest(String),
    #[error("clone perturbation candidate is invalid: {0}")]
    InvalidCandidate(String),
    #[error("clone perturbation panel output is invalid: {0}")]
    InvalidOutput(String),
    #[error("clone perturbation panel digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-@>".contains(&byte))
}

#[derive(Debug, Clone)]
struct BranchUnit {
    branch_id: String,
    parent_node_id: String,
    child_node_id: String,
    weight_milli: u32,
    target_markers: BTreeSet<String>,
    unresolved_markers: BTreeSet<String>,
}

fn digest_input(output: &ClonePerturbationPanel) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "candidate_order": output.candidate_order,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "branch_order": output.branch_order,
        "branch_coverage": output.branch_coverage,
        "decisions": output.decisions,
        "total_cost_milli": output.total_cost_milli,
        "weighted_coverage_milli": output.weighted_coverage_milli,
        "uncovered_branch_order": output.uncovered_branch_order,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &ClonePerturbationPanelRequest,
) -> Result<(), ClonePerturbationPanelError> {
    if !valid_identifier(&request.study_id)
        || request.budget_milli == 0
        || request.budget_milli > MAX_BUDGET_MILLI
        || request.min_coverage_milli > 1_000
        || request.max_selected == 0
        || request.max_selected > MAX_SELECTED
    {
        return Err(ClonePerturbationPanelError::InvalidRequest(
            "study, budget, coverage, or selection bounds are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_candidate(
    candidate: &ClonePerturbationCandidate,
) -> Result<(), ClonePerturbationPanelError> {
    if !valid_identifier(&candidate.candidate_id)
        || candidate
            .target_marker_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || candidate
            .target_marker_order
            .iter()
            .any(|marker| !valid_identifier(marker))
        || candidate.cost_milli == 0
        || candidate.cost_milli > MAX_BUDGET_MILLI
        || candidate.expected_effect_milli == 0
        || candidate.expected_effect_milli > 1_000
        || candidate.purpose.trim().is_empty()
        || (candidate.kind != ClonePerturbationKind::Control
            && candidate.target_marker_order.is_empty())
    {
        return Err(ClonePerturbationPanelError::InvalidCandidate(
            "candidate identity, target ordering, cost, effect, purpose, or control contract is invalid".into(),
        ));
    }
    candidate
        .artifact
        .validate()
        .map_err(|error| ClonePerturbationPanelError::InvalidCandidate(error.to_string()))
}

fn build_branches(
    graph: &ClonalEvolutionGraph,
) -> Result<Vec<BranchUnit>, ClonePerturbationPanelError> {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| (node.node_id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let mut branches = Vec::new();
    for edge in &graph.edges {
        let parent = nodes.get(edge.parent_node_id.as_str()).ok_or_else(|| {
            ClonePerturbationPanelError::InvalidOutput("graph parent node is missing".into())
        })?;
        let child = nodes.get(edge.child_node_id.as_str()).ok_or_else(|| {
            ClonePerturbationPanelError::InvalidOutput("graph child node is missing".into())
        })?;
        let mut target_markers = child
            .present_marker_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        target_markers.extend(edge.gained_marker_order.iter().cloned());
        let mut unresolved_markers = parent
            .unmeasured_marker_order
            .iter()
            .chain(child.unmeasured_marker_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        unresolved_markers.retain(|marker| !target_markers.contains(marker));
        branches.push(BranchUnit {
            branch_id: edge.edge_id.clone(),
            parent_node_id: edge.parent_node_id.clone(),
            child_node_id: edge.child_node_id.clone(),
            weight_milli: child.abundance_milli.max(1),
            target_markers,
            unresolved_markers,
        });
    }
    if branches.is_empty() {
        for node in &graph.nodes {
            branches.push(BranchUnit {
                branch_id: format!("node:{}", node.node_id),
                parent_node_id: String::new(),
                child_node_id: node.node_id.clone(),
                weight_milli: node.abundance_milli.max(1),
                target_markers: node.present_marker_order.iter().cloned().collect(),
                unresolved_markers: node.unmeasured_marker_order.iter().cloned().collect(),
            });
        }
    }
    branches.sort_by(|left, right| left.branch_id.cmp(&right.branch_id));
    branches.dedup_by(|left, right| left.branch_id == right.branch_id);
    if branches.is_empty() || branches.len() > MAX_BRANCHES {
        return Err(ClonePerturbationPanelError::InvalidOutput(
            "clonal graph has no bounded branch units".into(),
        ));
    }
    Ok(branches)
}

fn matching_branches(
    candidate: &ClonePerturbationCandidate,
    branches: &[BranchUnit],
    covered: &BTreeSet<String>,
    allow_uncertain: bool,
) -> (Vec<String>, Vec<String>, u32) {
    let targets = candidate
        .target_marker_order
        .iter()
        .collect::<BTreeSet<_>>();
    let mut covered_order = Vec::new();
    let mut uncertain_order = Vec::new();
    let mut gain = 0u32;
    for branch in branches {
        let exact = targets
            .iter()
            .any(|marker| branch.target_markers.contains(*marker));
        let uncertain = targets
            .iter()
            .any(|marker| branch.unresolved_markers.contains(*marker));
        if exact && !covered.contains(&branch.branch_id) {
            covered_order.push(branch.branch_id.clone());
            gain = gain.saturating_add(
                ((u64::from(branch.weight_milli) * u64::from(candidate.expected_effect_milli))
                    / 1_000)
                    .min(u64::from(u32::MAX)) as u32,
            );
        } else if uncertain && allow_uncertain {
            uncertain_order.push(branch.branch_id.clone());
            gain = gain.saturating_add(
                ((u64::from(branch.weight_milli) * u64::from(candidate.expected_effect_milli))
                    / 2_000)
                    .min(u64::from(u32::MAX)) as u32,
            );
        }
    }
    (covered_order, uncertain_order, gain)
}

#[derive(Debug, Clone)]
struct ClonePanelState {
    selected_ids: Vec<String>,
    covered_branches: BTreeSet<String>,
    uncertain_branches: BTreeSet<String>,
    total_cost: u64,
    total_gain: u64,
    utility: u64,
    selections: BTreeMap<String, (u32, u64, Vec<String>, Vec<String>)>,
}

fn weighted_covered_milli(state: &ClonePanelState, branches: &[BranchUnit]) -> u64 {
    let total_weight = branches
        .iter()
        .map(|branch| u64::from(branch.weight_milli))
        .sum::<u64>()
        .max(1);
    let covered_weight = branches
        .iter()
        .filter(|branch| state.covered_branches.contains(&branch.branch_id))
        .map(|branch| u64::from(branch.weight_milli))
        .sum::<u64>();
    (covered_weight * 1_000) / total_weight
}

fn clone_panel_state_complete(
    state: &ClonePanelState,
    request: &ClonePerturbationPanelRequest,
    branches: &[BranchUnit],
) -> bool {
    weighted_covered_milli(state, branches) >= u64::from(request.min_coverage_milli)
        && (!request.require_branch_coverage || state.covered_branches.len() == branches.len())
}

fn clone_panel_state_better(
    left: &ClonePanelState,
    right: &ClonePanelState,
    request: &ClonePerturbationPanelRequest,
    branches: &[BranchUnit],
) -> bool {
    let left_complete = clone_panel_state_complete(left, request, branches);
    let right_complete = clone_panel_state_complete(right, request, branches);
    if left_complete != right_complete {
        return left_complete;
    }
    let left_coverage = weighted_covered_milli(left, branches);
    let right_coverage = weighted_covered_milli(right, branches);
    left_coverage > right_coverage
        || (left_coverage == right_coverage
            && (left.total_gain > right.total_gain
                || (left.total_gain == right.total_gain
                    && (left.utility > right.utility
                        || (left.utility == right.utility
                            && (left.total_cost < right.total_cost
                                || (left.total_cost == right.total_cost
                                    && left.selected_ids < right.selected_ids)))))))
}

fn select_clone_panel_portfolio(
    request: &ClonePerturbationPanelRequest,
    branches: &[BranchUnit],
    candidates: &[ClonePerturbationCandidate],
) -> (ClonePanelState, bool) {
    let initial = ClonePanelState {
        selected_ids: Vec::new(),
        covered_branches: BTreeSet::new(),
        uncertain_branches: BTreeSet::new(),
        total_cost: 0,
        total_gain: 0,
        utility: 0,
        selections: BTreeMap::new(),
    };
    let mut beam = vec![initial.clone()];
    let mut best = initial;
    let search_depth = request.max_selected.min(MAX_PANEL_SEARCH_DEPTH);
    for _ in 0..search_depth {
        let mut expanded = Vec::new();
        for state in &beam {
            for candidate in candidates {
                if state
                    .selected_ids
                    .last()
                    .is_some_and(|last| candidate.candidate_id <= *last)
                    || state.selected_ids.contains(&candidate.candidate_id)
                    || state.total_cost.saturating_add(candidate.cost_milli) > request.budget_milli
                {
                    continue;
                }
                let (covered, uncertain, gain) = matching_branches(
                    candidate,
                    branches,
                    &state.covered_branches,
                    request.allow_uncertain_targets,
                );
                if gain == 0 {
                    continue;
                }
                let score = (u64::from(gain) * 1_000_000) / candidate.cost_milli.max(1);
                let mut selected_ids = state.selected_ids.clone();
                selected_ids.push(candidate.candidate_id.clone());
                let mut covered_branches = state.covered_branches.clone();
                covered_branches.extend(covered.iter().cloned());
                let mut uncertain_branches = state.uncertain_branches.clone();
                uncertain_branches.extend(uncertain.iter().cloned());
                let mut selections = state.selections.clone();
                selections.insert(
                    candidate.candidate_id.clone(),
                    (gain, score, covered, uncertain),
                );
                expanded.push(ClonePanelState {
                    selected_ids,
                    covered_branches,
                    uncertain_branches,
                    total_cost: state.total_cost.saturating_add(candidate.cost_milli),
                    total_gain: state.total_gain.saturating_add(u64::from(gain)),
                    utility: state.utility.saturating_add(score),
                    selections,
                });
            }
        }
        if expanded.is_empty() {
            break;
        }
        expanded.sort_by(|left, right| {
            if clone_panel_state_better(left, right, request, branches) {
                std::cmp::Ordering::Less
            } else if clone_panel_state_better(right, left, request, branches) {
                std::cmp::Ordering::Greater
            } else {
                left.selected_ids.cmp(&right.selected_ids)
            }
        });
        expanded.truncate(PANEL_BEAM_WIDTH);
        for candidate in &expanded {
            if clone_panel_state_better(candidate, &best, request, branches) {
                best = candidate.clone();
            }
        }
        beam = expanded;
        if clone_panel_state_complete(&best, request, branches) {
            break;
        }
    }
    (best, request.max_selected > MAX_PANEL_SEARCH_DEPTH)
}

fn branch_coverage(
    branches: &[BranchUnit],
    selected: &[ClonePerturbationCandidate],
) -> Vec<CloneBranchCoverage> {
    branches
        .iter()
        .map(|branch| {
            let mut selected_candidate_order = BTreeSet::new();
            let mut coverage = 0u16;
            for candidate in selected {
                let exact = candidate
                    .target_marker_order
                    .iter()
                    .any(|marker| branch.target_markers.contains(marker));
                if exact {
                    selected_candidate_order.insert(candidate.candidate_id.clone());
                    coverage = coverage.max(candidate.expected_effect_milli);
                } else if candidate
                    .target_marker_order
                    .iter()
                    .any(|marker| branch.unresolved_markers.contains(marker))
                {
                    selected_candidate_order.insert(candidate.candidate_id.clone());
                }
            }
            CloneBranchCoverage {
                branch_id: branch.branch_id.clone(),
                parent_node_id: branch.parent_node_id.clone(),
                child_node_id: branch.child_node_id.clone(),
                weight_milli: branch.weight_milli,
                target_marker_order: branch.target_markers.iter().cloned().collect(),
                unresolved_marker_order: branch.unresolved_markers.iter().cloned().collect(),
                selected_candidate_order: selected_candidate_order.into_iter().collect(),
                coverage_milli: coverage,
            }
        })
        .collect()
}

impl ClonePerturbationPanel {
    pub fn validate(&self) -> Result<(), ClonePerturbationPanelError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_identifier(&self.study_id)
            || !canonical(&self.candidate_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.branch_order)
            || !canonical(&self.uncovered_branch_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.selected_order.len() > MAX_SELECTED
            || self.total_cost_milli > MAX_BUDGET_MILLI
            || self.weighted_coverage_milli > 1_000
            || self.branch_coverage.len() != self.branch_order.len()
            || self.decisions.len() != self.candidate_order.len()
        {
            return Err(ClonePerturbationPanelError::InvalidOutput(
                "panel identity, ordering, bounds, or cardinality invariant failed".into(),
            ));
        }
        let candidates = self.candidate_order.iter().collect::<BTreeSet<_>>();
        let selected = self.selected_order.iter().collect::<BTreeSet<_>>();
        let deferred = self.deferred_order.iter().collect::<BTreeSet<_>>();
        let branches = self.branch_order.iter().collect::<BTreeSet<_>>();
        if selected.intersection(&deferred).next().is_some()
            || !selected.is_subset(&candidates)
            || !deferred.is_subset(&candidates)
            || self.branch_coverage.iter().any(|row| {
                !branches.contains(&row.branch_id)
                    || !canonical(&row.target_marker_order)
                    || !canonical(&row.unresolved_marker_order)
                    || !canonical(&row.selected_candidate_order)
                    || row.coverage_milli > 1_000
            })
            || self.decisions.iter().any(|decision| {
                !candidates.contains(&decision.candidate_id)
                    || !canonical(&decision.covered_branch_order)
                    || !canonical(&decision.uncertain_branch_order)
            })
        {
            return Err(ClonePerturbationPanelError::InvalidOutput(
                "panel partitions, branch rows, or candidate decisions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ClonePerturbationPanelError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ClonePerturbationPanelError::InvalidOutput(
                "panel digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Select a bounded, clone-aware preclinical perturbation/readout panel from a validated
/// clonal-evolution graph. A sequential portfolio beam evaluates joint branch coverage under
/// cost, effect, uncertainty, and cardinality gates; it is a design recommendation for an
/// approved local experiment executor.
pub fn plan_glioma_clone_perturbation_panel(
    request: &ClonePerturbationPanelRequest,
    graph: &ClonalEvolutionGraph,
    candidates: &[ClonePerturbationCandidate],
) -> Result<ClonePerturbationPanel, ClonePerturbationPanelError> {
    validate_request(request)?;
    graph
        .validate()
        .map_err(|error| ClonePerturbationPanelError::InvalidOutput(error.to_string()))?;
    if graph.study_id != request.study_id || graph.model_system != request.model_system {
        return Err(ClonePerturbationPanelError::InvalidRequest(
            "graph study/model binding does not match panel request".into(),
        ));
    }
    if candidates.is_empty() || candidates.len() > MAX_CANDIDATES {
        return Err(ClonePerturbationPanelError::InvalidRequest(
            "candidate set is empty or exceeds the bounded panel limit".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    for candidate in candidates {
        if !seen.insert(candidate.candidate_id.clone()) {
            return Err(ClonePerturbationPanelError::InvalidCandidate(
                "candidate identifiers must be unique".into(),
            ));
        }
        validate_candidate(candidate)?;
    }
    let branches = build_branches(graph)?;
    let mut ordered_candidates = candidates.to_vec();
    ordered_candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let candidate_order = ordered_candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let (best_portfolio, search_depth_bounded) =
        select_clone_panel_portfolio(request, &branches, &ordered_candidates);
    let selected_ids = best_portfolio
        .selected_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let covered_branches = best_portfolio.covered_branches.clone();
    let total_cost = best_portfolio.total_cost;
    let mut decisions_by_id = BTreeMap::<String, ClonePerturbationDecision>::new();
    for candidate in &ordered_candidates {
        if let Some((gain, score, covered, uncertain)) =
            best_portfolio.selections.get(&candidate.candidate_id)
        {
            decisions_by_id.insert(
                candidate.candidate_id.clone(),
                ClonePerturbationDecision {
                    candidate_id: candidate.candidate_id.clone(),
                    kind: candidate.kind,
                    selected: true,
                    covered_branch_order: covered.clone(),
                    uncertain_branch_order: uncertain.clone(),
                    marginal_gain_milli: *gain,
                    score_milli: *score,
                    cost_milli: candidate.cost_milli,
                    rationale:
                        "selected by sequential branch-coverage portfolio beam under declared cost"
                            .into(),
                },
            );
        }
    }
    let selected_candidates = ordered_candidates
        .iter()
        .filter(|candidate| selected_ids.contains(&candidate.candidate_id))
        .cloned()
        .collect::<Vec<_>>();
    let selected_order = selected_candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let deferred_order = ordered_candidates
        .iter()
        .filter(|candidate| !selected_ids.contains(&candidate.candidate_id))
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    for candidate in &ordered_candidates {
        decisions_by_id
            .entry(candidate.candidate_id.clone())
            .or_insert_with(|| {
                let (covered, uncertain, gain) = matching_branches(
                    candidate,
                    &branches,
                    &covered_branches,
                    request.allow_uncertain_targets,
                );
                ClonePerturbationDecision {
                    candidate_id: candidate.candidate_id.clone(),
                    kind: candidate.kind,
                    selected: false,
                    covered_branch_order: covered,
                    uncertain_branch_order: uncertain,
                    marginal_gain_milli: gain,
                    score_milli: 0,
                    cost_milli: candidate.cost_milli,
                    rationale: if total_cost.saturating_add(candidate.cost_milli)
                        > request.budget_milli
                    {
                        "deferred because the declared panel budget is exhausted".into()
                    } else {
                        "deferred because it adds no uncovered branch coverage under the current panel".into()
                    },
                }
            });
    }
    let decisions = candidate_order
        .iter()
        .map(|id| {
            decisions_by_id
                .remove(id)
                .expect("decision for every candidate")
        })
        .collect::<Vec<_>>();
    let branch_coverage = branch_coverage(&branches, &selected_candidates);
    let total_weight = branches
        .iter()
        .map(|branch| u64::from(branch.weight_milli))
        .sum::<u64>()
        .max(1);
    let weighted_covered = branch_coverage
        .iter()
        .filter(|row| row.coverage_milli > 0)
        .map(|row| u64::from(row.weight_milli))
        .sum::<u64>();
    let weighted_coverage_milli = ((weighted_covered * 1_000) / total_weight).min(1_000) as u16;
    let branch_order = branches
        .iter()
        .map(|branch| branch.branch_id.clone())
        .collect::<Vec<_>>();
    let uncovered_branch_order = branch_coverage
        .iter()
        .filter(|row| row.coverage_milli == 0)
        .map(|row| row.branch_id.clone())
        .collect::<Vec<_>>();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    for item in &graph.uncertainty {
        uncertainty.insert(format!("graph:{item}"));
    }
    for row in &branch_coverage {
        for marker in &row.unresolved_marker_order {
            uncertainty.insert(format!("unmeasured-target:{}:{marker}", row.branch_id));
        }
    }
    if search_depth_bounded {
        uncertainty.insert("portfolio-search-depth-bounded".into());
    }
    for branch_id in &uncovered_branch_order {
        uncertainty.insert(format!("uncovered-branch:{branch_id}"));
        negative_evidence.insert(format!("no-selected-candidate-for-branch:{branch_id}"));
    }
    if total_cost >= request.budget_milli && !deferred_order.is_empty() {
        uncertainty.insert("panel-budget-exhausted".into());
    }
    if selected_order.is_empty() {
        uncertainty.insert("no-candidate-provided-positive-branch-gain".into());
    }
    if request.require_branch_coverage && !uncovered_branch_order.is_empty() {
        uncertainty.insert("required-branch-coverage-not-met".into());
    }
    for candidate in &ordered_candidates {
        if candidate.target_marker_order.is_empty() {
            negative_evidence.insert(format!(
                "control-candidate:{}-has-no-target-marker",
                candidate.candidate_id
            ));
        }
    }
    let disposition = if selected_order.is_empty() {
        ClonePerturbationPanelDisposition::Unresolved
    } else if weighted_coverage_milli >= request.min_coverage_milli
        && (!request.require_branch_coverage || uncovered_branch_order.is_empty())
        && graph.disposition == crate::glioma::programs::p05_mechanism_exploration::ClonalEvolutionDisposition::Qualified
    {
        ClonePerturbationPanelDisposition::Qualified
    } else {
        ClonePerturbationPanelDisposition::Partial
    };
    let mut output = ClonePerturbationPanel {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        candidate_order,
        selected_order,
        deferred_order,
        branch_order,
        branch_coverage,
        decisions,
        total_cost_milli: total_cost,
        weighted_coverage_milli,
        uncovered_branch_order,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-clone-perturbation-panel"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ClonePerturbationPanelError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::{
        ClonalEvolutionRequest, CloneMarker, CloneMarkerState, CloneProfile,
        analyze_glioma_clonal_evolution,
    };
    use bioprism_ids::ContentHash;

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

    fn graph() -> ClonalEvolutionGraph {
        let profile = |id: &str, clone_id: &str, timepoint: u32, markers: &[&str]| CloneProfile {
            profile_id: id.into(),
            study_id: "panel-study".into(),
            sample_lineage: "lineage-a".into(),
            clone_id: clone_id.into(),
            timepoint,
            model_system: GliomaModelSystem::Organoid,
            abundance_milli: if timepoint == 0 { 400 } else { 600 },
            artifact: artifact(id),
            markers: markers
                .iter()
                .map(|marker_id| CloneMarker {
                    marker_id: (*marker_id).into(),
                    state: CloneMarkerState::Present,
                    confidence_milli: 900,
                })
                .collect(),
        };
        analyze_glioma_clonal_evolution(
            &ClonalEvolutionRequest {
                study_id: "panel-study".into(),
                model_system: GliomaModelSystem::Organoid,
                min_shared_markers: 1,
                min_parent_score_milli: 500,
                max_time_gap: 5,
                min_abundance_milli: 1,
                allow_parallel_branches: true,
                max_parent_candidates: 2,
            },
            &[
                profile("root", "clone-a", 0, &["egfr", "tp53"]),
                profile("branch-egfr", "clone-b", 1, &["egfr", "tp53", "ecDNA"]),
                profile("branch-tp53", "clone-c", 1, &["egfr", "tp53", "pten"]),
            ],
        )
        .unwrap()
    }

    fn weighted_graph() -> ClonalEvolutionGraph {
        let profile =
            |id: &str, clone_id: &str, abundance_milli: u32, markers: &[&str]| CloneProfile {
                profile_id: id.into(),
                study_id: "panel-study".into(),
                sample_lineage: "lineage-a".into(),
                clone_id: clone_id.into(),
                timepoint: if clone_id == "clone-a" { 0 } else { 1 },
                model_system: GliomaModelSystem::Organoid,
                abundance_milli,
                artifact: artifact(id),
                markers: markers
                    .iter()
                    .map(|marker_id| CloneMarker {
                        marker_id: (*marker_id).into(),
                        state: CloneMarkerState::Present,
                        confidence_milli: 900,
                    })
                    .collect(),
            };
        analyze_glioma_clonal_evolution(
            &ClonalEvolutionRequest {
                study_id: "panel-study".into(),
                model_system: GliomaModelSystem::Organoid,
                min_shared_markers: 1,
                min_parent_score_milli: 500,
                max_time_gap: 5,
                min_abundance_milli: 1,
                allow_parallel_branches: true,
                max_parent_candidates: 3,
            },
            &[
                profile("root", "clone-a", 1_000, &["shared"]),
                profile("branch-ecdn", "clone-b", 450, &["shared", "ecDNA"]),
                profile("branch-pten", "clone-c", 450, &["shared", "pten"]),
                profile("branch-nf1", "clone-d", 100, &["shared", "nf1"]),
            ],
        )
        .unwrap()
    }

    fn request() -> ClonePerturbationPanelRequest {
        ClonePerturbationPanelRequest {
            study_id: "panel-study".into(),
            model_system: GliomaModelSystem::Organoid,
            budget_milli: 10,
            min_coverage_milli: 500,
            max_selected: 2,
            require_branch_coverage: true,
            allow_uncertain_targets: true,
        }
    }

    fn candidate(id: &str, marker: &str, cost_milli: u64) -> ClonePerturbationCandidate {
        ClonePerturbationCandidate {
            candidate_id: id.into(),
            kind: ClonePerturbationKind::Inhibit,
            target_marker_order: vec![marker.into()],
            cost_milli,
            expected_effect_milli: 900,
            purpose: "branch-selective perturbation readout".into(),
            artifact: artifact(id),
        }
    }

    fn multi_candidate(id: &str, markers: &[&str], cost_milli: u64) -> ClonePerturbationCandidate {
        ClonePerturbationCandidate {
            candidate_id: id.into(),
            kind: ClonePerturbationKind::Inhibit,
            target_marker_order: markers.iter().map(|marker| (*marker).into()).collect(),
            cost_milli,
            expected_effect_milli: 1_000,
            purpose: "joint branch-discriminating perturbation readout".into(),
            artifact: artifact(id),
        }
    }

    #[test]
    fn panel_selects_branch_covering_candidates_and_replays() {
        let candidates = vec![
            candidate("pten-panel", "pten", 5),
            candidate("ecdn-panel", "ecDNA", 5),
        ];
        let first =
            plan_glioma_clone_perturbation_panel(&request(), &graph(), &candidates).unwrap();
        let second =
            plan_glioma_clone_perturbation_panel(&request(), &graph(), &candidates).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            ClonePerturbationPanelDisposition::Qualified
        );
        assert_eq!(first.selected_order.len(), 2);
        assert!(first.weighted_coverage_milli >= 500);
    }

    #[test]
    fn candidate_input_permutation_is_replay_stable() {
        let candidates = vec![
            candidate("pten-panel", "pten", 5),
            candidate("ecdn-panel", "ecDNA", 5),
        ];
        let reversed = candidates.iter().cloned().rev().collect::<Vec<_>>();
        assert_eq!(
            plan_glioma_clone_perturbation_panel(&request(), &graph(), &candidates).unwrap(),
            plan_glioma_clone_perturbation_panel(&request(), &graph(), &reversed).unwrap()
        );
    }

    #[test]
    fn budget_shortfall_is_partial_and_negative() {
        let mut request = request();
        request.budget_milli = 5;
        let output = plan_glioma_clone_perturbation_panel(
            &request,
            &graph(),
            &[
                candidate("pten-panel", "pten", 5),
                candidate("ecdn-panel", "ecDNA", 5),
            ],
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            ClonePerturbationPanelDisposition::Partial
        );
        assert!(
            output
                .negative_evidence
                .iter()
                .any(|item| item.contains("no-selected-candidate-for-branch"))
        );
        assert!(
            output
                .uncertainty
                .iter()
                .any(|item| item == "panel-budget-exhausted")
        );
    }

    #[test]
    fn candidate_human_artifact_is_refused() {
        let mut item = candidate("bad", "ecDNA", 1);
        item.artifact.contains_human_data = true;
        assert!(matches!(
            plan_glioma_clone_perturbation_panel(&request(), &graph(), &[item]),
            Err(ClonePerturbationPanelError::InvalidCandidate(_))
        ));
    }

    #[test]
    fn portfolio_beam_prefers_complementary_branch_coverage_over_ratio_greedy_choice() {
        let mut request = request();
        request.budget_milli = 5;
        request.max_selected = 2;
        let output = plan_glioma_clone_perturbation_panel(
            &request,
            &weighted_graph(),
            &[
                multi_candidate("a-broad", &["ecDNA", "pten"], 4),
                multi_candidate("b-ecdn", &["ecDNA"], 2),
                multi_candidate("c-pten-nf1", &["nf1", "pten"], 3),
            ],
        )
        .unwrap();
        assert_eq!(output.selected_order, vec!["b-ecdn", "c-pten-nf1"]);
        assert!(output.uncovered_branch_order.is_empty());
        assert_eq!(output.total_cost_milli, 5);
    }
}
