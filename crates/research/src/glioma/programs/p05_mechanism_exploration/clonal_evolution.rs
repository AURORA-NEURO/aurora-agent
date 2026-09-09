//! Bounded clonal-evolution graph inference for preclinical glioma models.
//!
//! Longitudinal clone profiles are joined only inside a declared preclinical sample lineage. The
//! planner proposes parent-child edges from marker-set overlap, temporal order, and abundance
//! change; it never turns an ambiguous graph into a single certain phylogeny. Gained/lost marker
//! sets, weak-parent candidates, unparented nodes, and parallel branches remain first-class output
//! so mechanism and experiment programs can choose what to measure next.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F30";
pub const OUTPUT_SCHEMA: &str = "GliomaClonalEvolutionGraph1@1";
pub const MAX_PROFILES: usize = 8_192;
pub const MAX_MARKERS_PER_PROFILE: usize = 4_096;
pub const MAX_NODES: usize = 8_192;
pub const MAX_EDGES: usize = 16_384;
pub const MAX_TIME_GAP: u32 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloneMarkerState {
    Present,
    Absent,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneMarker {
    pub marker_id: String,
    pub state: CloneMarkerState,
    pub confidence_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneProfile {
    pub profile_id: String,
    pub study_id: String,
    pub sample_lineage: String,
    pub clone_id: String,
    pub timepoint: u32,
    pub model_system: GliomaModelSystem,
    pub abundance_milli: u32,
    pub artifact: LocalArtifactRef,
    pub markers: Vec<CloneMarker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonalEvolutionRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub min_shared_markers: usize,
    pub min_parent_score_milli: u16,
    pub max_time_gap: u32,
    pub min_abundance_milli: u32,
    pub allow_parallel_branches: bool,
    pub max_parent_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonalNode {
    pub node_id: String,
    pub profile_id: String,
    pub sample_lineage: String,
    pub clone_id: String,
    pub timepoint: u32,
    pub abundance_milli: u32,
    pub present_marker_order: Vec<String>,
    pub absent_marker_order: Vec<String>,
    pub unmeasured_marker_order: Vec<String>,
    pub artifact: LocalArtifactRef,
    pub signature: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClonalRelation {
    Expansion,
    Contraction,
    Divergence,
    Stable,
    Mixed,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonalEdge {
    pub edge_id: String,
    pub parent_node_id: String,
    pub child_node_id: String,
    pub shared_marker_order: Vec<String>,
    pub gained_marker_order: Vec<String>,
    pub lost_marker_order: Vec<String>,
    pub parent_score_milli: u16,
    pub abundance_delta_milli: i64,
    pub relation: ClonalRelation,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClonalEvolutionDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonalEvolutionGraph {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub node_order: Vec<String>,
    pub edge_order: Vec<String>,
    pub priority_edge_order: Vec<String>,
    pub root_order: Vec<String>,
    pub unparented_order: Vec<String>,
    pub branch_order: Vec<String>,
    pub gained_marker_order: Vec<String>,
    pub lost_marker_order: Vec<String>,
    pub nodes: Vec<ClonalNode>,
    pub edges: Vec<ClonalEdge>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ClonalEvolutionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClonalEvolutionError {
    #[error("clonal evolution request is invalid: {0}")]
    InvalidRequest(String),
    #[error("clonal profile is invalid: {0}")]
    InvalidProfile(String),
    #[error("clonal evolution output is invalid: {0}")]
    InvalidOutput(String),
    #[error("clonal evolution digest failed: {0}")]
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

fn node_id(profile: &CloneProfile) -> String {
    format!(
        "{}:{}@{}:{}",
        profile.sample_lineage, profile.clone_id, profile.timepoint, profile.profile_id
    )
}

fn edge_id(parent: &ClonalNode, child: &ClonalNode) -> String {
    format!("{}->{}", parent.node_id, child.node_id)
}

fn present_markers(profile: &CloneProfile) -> BTreeSet<String> {
    profile
        .markers
        .iter()
        .filter(|marker| marker.state == CloneMarkerState::Present)
        .map(|marker| marker.marker_id.clone())
        .collect()
}

fn absent_markers(profile: &CloneProfile) -> BTreeSet<String> {
    profile
        .markers
        .iter()
        .filter(|marker| marker.state == CloneMarkerState::Absent)
        .map(|marker| marker.marker_id.clone())
        .collect()
}

fn unmeasured_markers(profile: &CloneProfile) -> BTreeSet<String> {
    profile
        .markers
        .iter()
        .filter(|marker| marker.state == CloneMarkerState::Unmeasured)
        .map(|marker| marker.marker_id.clone())
        .collect()
}

fn digest_input(output: &ClonalEvolutionGraph) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "node_order": output.node_order,
        "edge_order": output.edge_order,
        "priority_edge_order": output.priority_edge_order,
        "root_order": output.root_order,
        "unparented_order": output.unparented_order,
        "branch_order": output.branch_order,
        "gained_marker_order": output.gained_marker_order,
        "lost_marker_order": output.lost_marker_order,
        "nodes": output.nodes,
        "edges": output.edges,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl ClonalNode {
    fn validate(&self) -> Result<(), ClonalEvolutionError> {
        if !valid_identifier(&self.node_id)
            || !valid_identifier(&self.profile_id)
            || !valid_identifier(&self.sample_lineage)
            || !valid_identifier(&self.clone_id)
            || !canonical(&self.present_marker_order)
            || !canonical(&self.absent_marker_order)
            || !canonical(&self.unmeasured_marker_order)
            || self
                .present_marker_order
                .iter()
                .chain(self.absent_marker_order.iter())
                .chain(self.unmeasured_marker_order.iter())
                .any(|marker| !valid_identifier(marker))
            || self.abundance_milli > 1_000_000
            || self.artifact.validate().is_err()
            || self.signature.as_str().len() != 64
        {
            return Err(ClonalEvolutionError::InvalidOutput(
                "node identity, marker ordering, abundance, or signature invariant failed".into(),
            ));
        }
        let present = self.present_marker_order.iter().collect::<BTreeSet<_>>();
        let absent = self.absent_marker_order.iter().collect::<BTreeSet<_>>();
        let unmeasured = self.unmeasured_marker_order.iter().collect::<BTreeSet<_>>();
        if present.intersection(&absent).next().is_some()
            || present.intersection(&unmeasured).next().is_some()
            || absent.intersection(&unmeasured).next().is_some()
        {
            return Err(ClonalEvolutionError::InvalidOutput(
                "node marker states must be disjoint".into(),
            ));
        }
        Ok(())
    }
}

impl ClonalEdge {
    fn validate(&self) -> Result<(), ClonalEvolutionError> {
        if !valid_identifier(&self.edge_id)
            || self.parent_node_id == self.child_node_id
            || !valid_identifier(&self.parent_node_id)
            || !valid_identifier(&self.child_node_id)
            || !canonical(&self.shared_marker_order)
            || !canonical(&self.gained_marker_order)
            || !canonical(&self.lost_marker_order)
            || self
                .shared_marker_order
                .iter()
                .chain(self.gained_marker_order.iter())
                .chain(self.lost_marker_order.iter())
                .any(|marker| !valid_identifier(marker))
            || self.parent_score_milli > 1_000
            || self.uncertainty.iter().any(|item| item.trim().is_empty())
        {
            return Err(ClonalEvolutionError::InvalidOutput(
                "edge identity, marker ordering, score, or uncertainty invariant failed".into(),
            ));
        }
        let shared = self.shared_marker_order.iter().collect::<BTreeSet<_>>();
        let gained = self.gained_marker_order.iter().collect::<BTreeSet<_>>();
        let lost = self.lost_marker_order.iter().collect::<BTreeSet<_>>();
        if shared.intersection(&gained).next().is_some()
            || shared.intersection(&lost).next().is_some()
            || gained.intersection(&lost).next().is_some()
        {
            return Err(ClonalEvolutionError::InvalidOutput(
                "edge marker partitions must be disjoint".into(),
            ));
        }
        Ok(())
    }
}

impl ClonalEvolutionGraph {
    pub fn validate(&self) -> Result<(), ClonalEvolutionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_identifier(&self.study_id)
            || !canonical(&self.node_order)
            || !canonical(&self.edge_order)
            || !canonical(&self.root_order)
            || !canonical(&self.unparented_order)
            || !canonical(&self.branch_order)
            || !canonical(&self.gained_marker_order)
            || !canonical(&self.lost_marker_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.nodes.len() != self.node_order.len()
            || self.edges.len() != self.edge_order.len()
            || self.nodes.len() > MAX_NODES
            || self.edges.len() > MAX_EDGES
            || self.nodes.iter().any(|node| node.validate().is_err())
            || self.edges.iter().any(|edge| edge.validate().is_err())
        {
            return Err(ClonalEvolutionError::InvalidOutput(
                "graph identity, ordering, cardinality, or nested invariant failed".into(),
            ));
        }
        let node_ids = self
            .nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect::<BTreeSet<_>>();
        let edge_ids = self
            .edges
            .iter()
            .map(|edge| edge.edge_id.clone())
            .collect::<BTreeSet<_>>();
        let referenced = self
            .edges
            .iter()
            .flat_map(|edge| [edge.parent_node_id.clone(), edge.child_node_id.clone()])
            .collect::<BTreeSet<_>>();
        if node_ids != self.node_order.iter().cloned().collect::<BTreeSet<_>>()
            || edge_ids != self.edge_order.iter().cloned().collect::<BTreeSet<_>>()
            || !referenced.is_subset(&node_ids)
            || !self
                .priority_edge_order
                .iter()
                .chain(self.branch_order.iter())
                .all(|id| edge_ids.contains(id))
            || !self.root_order.iter().all(|id| node_ids.contains(id))
            || !self.unparented_order.iter().all(|id| node_ids.contains(id))
        {
            return Err(ClonalEvolutionError::InvalidOutput(
                "graph partitions and edge references do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ClonalEvolutionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ClonalEvolutionError::InvalidOutput(
                "clonal evolution digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ClonalEvolutionRequest) -> Result<(), ClonalEvolutionError> {
    if !valid_identifier(&request.study_id)
        || request.min_shared_markers == 0
        || request.min_shared_markers > MAX_MARKERS_PER_PROFILE
        || request.min_parent_score_milli > 1_000
        || request.max_time_gap == 0
        || request.max_time_gap > MAX_TIME_GAP
        || request.max_parent_candidates == 0
        || request.max_parent_candidates > 64
        || request.min_abundance_milli > 1_000_000
    {
        return Err(ClonalEvolutionError::InvalidRequest(
            "study, marker, temporal, abundance, score, or parent-candidate bounds are invalid"
                .into(),
        ));
    }
    Ok(())
}

fn validate_profile(
    request: &ClonalEvolutionRequest,
    profile: &CloneProfile,
    seen_profiles: &mut BTreeSet<String>,
) -> Result<(), ClonalEvolutionError> {
    if !seen_profiles.insert(profile.profile_id.clone())
        || !valid_identifier(&profile.profile_id)
        || !valid_identifier(&profile.sample_lineage)
        || !valid_identifier(&profile.clone_id)
        || profile.study_id != request.study_id
        || profile.model_system != request.model_system
        || profile.abundance_milli > 1_000_000
        || profile.abundance_milli < request.min_abundance_milli
        || profile.markers.is_empty()
        || profile.markers.len() > MAX_MARKERS_PER_PROFILE
    {
        return Err(ClonalEvolutionError::InvalidProfile(
            "profile identity, study/model binding, abundance, or marker cardinality is invalid"
                .into(),
        ));
    }
    profile
        .artifact
        .validate()
        .map_err(|error| ClonalEvolutionError::InvalidProfile(error.to_string()))?;
    let mut markers = BTreeSet::new();
    for marker in &profile.markers {
        if !valid_identifier(&marker.marker_id)
            || marker.confidence_milli > 1_000
            || !markers.insert(marker.marker_id.clone())
        {
            return Err(ClonalEvolutionError::InvalidProfile(
                "markers must be unique, identifier-safe, and confidence-bounded".into(),
            ));
        }
    }
    Ok(())
}

fn build_node(profile: &CloneProfile) -> Result<ClonalNode, ClonalEvolutionError> {
    let present_marker_order = present_markers(profile).into_iter().collect::<Vec<_>>();
    let absent_marker_order = absent_markers(profile).into_iter().collect::<Vec<_>>();
    let unmeasured_marker_order = unmeasured_markers(profile).into_iter().collect::<Vec<_>>();
    let id = node_id(profile);
    let signature = ContentHash::of_value(&serde_json::json!({
        "node_id": id,
        "profile_id": profile.profile_id,
        "sample_lineage": profile.sample_lineage,
        "clone_id": profile.clone_id,
        "timepoint": profile.timepoint,
        "abundance_milli": profile.abundance_milli,
        "present_marker_order": present_marker_order,
        "absent_marker_order": absent_marker_order,
        "unmeasured_marker_order": unmeasured_marker_order,
        "artifact": profile.artifact,
    }))
    .map_err(|error| ClonalEvolutionError::Digest(error.to_string()))?;
    Ok(ClonalNode {
        node_id: id,
        profile_id: profile.profile_id.clone(),
        sample_lineage: profile.sample_lineage.clone(),
        clone_id: profile.clone_id.clone(),
        timepoint: profile.timepoint,
        abundance_milli: profile.abundance_milli,
        present_marker_order,
        absent_marker_order,
        unmeasured_marker_order,
        artifact: profile.artifact.clone(),
        signature,
    })
}

fn build_edge(
    parent: &ClonalNode,
    child: &ClonalNode,
    request: &ClonalEvolutionRequest,
) -> Option<ClonalEdge> {
    if parent.sample_lineage != child.sample_lineage
        || parent.timepoint >= child.timepoint
        || child.timepoint - parent.timepoint > request.max_time_gap
    {
        return None;
    }
    let parent_markers = parent
        .present_marker_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let child_markers = child
        .present_marker_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let shared = parent_markers
        .intersection(&child_markers)
        .cloned()
        .collect::<Vec<_>>();
    if shared.len() < request.min_shared_markers {
        return None;
    }
    let gained = child_markers
        .difference(&parent_markers)
        .cloned()
        .collect::<Vec<_>>();
    let lost = parent_markers
        .difference(&child_markers)
        .cloned()
        .collect::<Vec<_>>();
    let parent_count = parent_markers.len().max(1) as u32;
    let child_count = child_markers.len().max(1) as u32;
    let overlap_parent = (shared.len() as u32 * 1_000 / parent_count).min(1_000);
    let overlap_child = (shared.len() as u32 * 1_000 / child_count).min(1_000);
    let temporal = (request
        .max_time_gap
        .saturating_sub(child.timepoint - parent.timepoint))
    .saturating_mul(1_000)
        / request.max_time_gap;
    let score = ((overlap_parent + overlap_child) / 2 * 8 + temporal * 2) / 10;
    if score < u32::from(request.min_parent_score_milli) {
        return None;
    }
    let delta = i64::from(child.abundance_milli) - i64::from(parent.abundance_milli);
    let relation = if !gained.is_empty() && !lost.is_empty() {
        ClonalRelation::Mixed
    } else if !gained.is_empty() {
        ClonalRelation::Divergence
    } else if delta > 0 {
        ClonalRelation::Expansion
    } else if delta < 0 {
        ClonalRelation::Contraction
    } else {
        ClonalRelation::Stable
    };
    let mut uncertainty = Vec::new();
    if !parent.unmeasured_marker_order.is_empty() {
        uncertainty.push("parent-marker-unmeasured-at-source".into());
    }
    if !child.unmeasured_marker_order.is_empty() {
        uncertainty.push("child-marker-unmeasured-at-target".into());
    }
    Some(ClonalEdge {
        edge_id: edge_id(parent, child),
        parent_node_id: parent.node_id.clone(),
        child_node_id: child.node_id.clone(),
        shared_marker_order: shared,
        gained_marker_order: gained,
        lost_marker_order: lost,
        parent_score_milli: score as u16,
        abundance_delta_milli: delta,
        relation,
        uncertainty,
    })
}

/// Build a deterministic, ambiguity-preserving clonal evolution graph for preclinical glioma
/// profiles. The graph is a mechanism-discovery artifact, not a clinical phylogeny.
pub fn analyze_glioma_clonal_evolution(
    request: &ClonalEvolutionRequest,
    profiles: &[CloneProfile],
) -> Result<ClonalEvolutionGraph, ClonalEvolutionError> {
    validate_request(request)?;
    if profiles.is_empty() || profiles.len() > MAX_PROFILES {
        return Err(ClonalEvolutionError::InvalidRequest(
            "clonal evolution requires a bounded non-empty profile set".into(),
        ));
    }
    let mut seen_profiles = BTreeSet::new();
    let mut nodes = profiles
        .iter()
        .map(|profile| {
            validate_profile(request, profile, &mut seen_profiles)?;
            build_node(profile)
        })
        .collect::<Result<Vec<_>, _>>()?;
    nodes.sort_by(|left, right| {
        left.sample_lineage
            .cmp(&right.sample_lineage)
            .then_with(|| left.timepoint.cmp(&right.timepoint))
            .then_with(|| left.clone_id.cmp(&right.clone_id))
            .then_with(|| left.node_id.cmp(&right.node_id))
    });
    if nodes.len() > MAX_NODES {
        return Err(ClonalEvolutionError::InvalidRequest(
            "node bound exceeded".into(),
        ));
    }
    let mut candidate_edges = Vec::new();
    for child in &nodes {
        let mut candidates = nodes
            .iter()
            .filter_map(|parent| build_edge(parent, child, request))
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .parent_score_milli
                .cmp(&left.parent_score_milli)
                .then_with(|| left.parent_node_id.cmp(&right.parent_node_id))
        });
        if !request.allow_parallel_branches {
            candidates.truncate(1);
        } else {
            candidates.truncate(request.max_parent_candidates);
        }
        candidate_edges.extend(candidates);
    }
    candidate_edges.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    if candidate_edges.len() > MAX_EDGES {
        return Err(ClonalEvolutionError::InvalidRequest(
            "edge bound exceeded".into(),
        ));
    }
    let node_order = nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    let edge_order = candidate_edges
        .iter()
        .map(|edge| edge.edge_id.clone())
        .collect::<Vec<_>>();
    let child_ids = candidate_edges
        .iter()
        .map(|edge| edge.child_node_id.clone())
        .collect::<BTreeSet<_>>();
    let root_order = nodes
        .iter()
        .filter(|node| !child_ids.contains(&node.node_id))
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    let unparented_order = nodes
        .iter()
        .filter(|node| {
            node.timepoint
                > nodes
                    .iter()
                    .filter(|candidate| candidate.sample_lineage == node.sample_lineage)
                    .map(|candidate| candidate.timepoint)
                    .min()
                    .unwrap_or(node.timepoint)
                && !child_ids.contains(&node.node_id)
        })
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    let mut priority = candidate_edges.clone();
    priority.sort_by(|left, right| {
        right
            .parent_score_milli
            .cmp(&left.parent_score_milli)
            .then_with(|| left.edge_id.cmp(&right.edge_id))
    });
    let priority_edge_order = priority
        .iter()
        .map(|edge| edge.edge_id.clone())
        .collect::<Vec<_>>();
    let branch_order = candidate_edges
        .iter()
        .filter(|edge| {
            matches!(
                edge.relation,
                ClonalRelation::Divergence | ClonalRelation::Mixed
            )
        })
        .map(|edge| edge.edge_id.clone())
        .collect::<Vec<_>>();
    let gained_marker_order = candidate_edges
        .iter()
        .flat_map(|edge| edge.gained_marker_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let lost_marker_order = candidate_edges
        .iter()
        .flat_map(|edge| edge.lost_marker_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    for node_id in &unparented_order {
        uncertainty.insert(format!("unparented-node:{node_id}"));
    }
    for edge in &candidate_edges {
        uncertainty.extend(
            edge.uncertainty
                .iter()
                .map(|item| format!("edge:{}:{item}", edge.edge_id)),
        );
        if edge.relation == ClonalRelation::Stable {
            negative_evidence.insert(format!(
                "stable-clone-transition:{}:no-marker-gain-or-loss",
                edge.edge_id
            ));
        }
    }
    if candidate_edges.is_empty() {
        uncertainty.insert("no-parent-child-edge-met-declared-support-floor".into());
    }
    let first_timepoint_by_lineage =
        nodes
            .iter()
            .fold(BTreeMap::<String, u32>::new(), |mut map, node| {
                map.entry(node.sample_lineage.clone())
                    .and_modify(|timepoint| *timepoint = (*timepoint).min(node.timepoint))
                    .or_insert(node.timepoint);
                map
            });
    let eligible_nodes = nodes
        .iter()
        .filter(|node| node.timepoint > first_timepoint_by_lineage[&node.sample_lineage])
        .count();
    let parented_nodes = child_ids.len();
    let disposition = if candidate_edges.is_empty() {
        ClonalEvolutionDisposition::Unresolved
    } else if eligible_nodes == parented_nodes && unparented_order.is_empty() {
        ClonalEvolutionDisposition::Qualified
    } else {
        ClonalEvolutionDisposition::Partial
    };
    let mut output = ClonalEvolutionGraph {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        node_order,
        edge_order,
        priority_edge_order,
        root_order,
        unparented_order,
        branch_order,
        gained_marker_order,
        lost_marker_order,
        nodes,
        edges: candidate_edges,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-clonal-evolution"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ClonalEvolutionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn profile(
        id: &str,
        clone_id: &str,
        timepoint: u32,
        abundance_milli: u32,
        markers: &[(&str, CloneMarkerState)],
    ) -> CloneProfile {
        CloneProfile {
            profile_id: id.into(),
            study_id: "clonal-study".into(),
            sample_lineage: "lineage-a".into(),
            clone_id: clone_id.into(),
            timepoint,
            model_system: GliomaModelSystem::Organoid,
            abundance_milli,
            artifact: artifact(id),
            markers: markers
                .iter()
                .map(|(marker_id, state)| CloneMarker {
                    marker_id: (*marker_id).into(),
                    state: *state,
                    confidence_milli: 900,
                })
                .collect(),
        }
    }

    fn request() -> ClonalEvolutionRequest {
        ClonalEvolutionRequest {
            study_id: "clonal-study".into(),
            model_system: GliomaModelSystem::Organoid,
            min_shared_markers: 2,
            min_parent_score_milli: 600,
            max_time_gap: 5,
            min_abundance_milli: 1,
            allow_parallel_branches: true,
            max_parent_candidates: 2,
        }
    }

    #[test]
    fn evolution_graph_identifies_gained_marker_branch_and_replays() {
        let profiles = vec![
            profile(
                "root",
                "clone-a",
                0,
                400,
                &[
                    ("egfr", CloneMarkerState::Present),
                    ("tp53", CloneMarkerState::Present),
                ],
            ),
            profile(
                "child",
                "clone-b",
                1,
                700,
                &[
                    ("egfr", CloneMarkerState::Present),
                    ("tp53", CloneMarkerState::Present),
                    ("ecDNA", CloneMarkerState::Present),
                ],
            ),
        ];
        let first = analyze_glioma_clonal_evolution(&request(), &profiles).unwrap();
        let second = analyze_glioma_clonal_evolution(&request(), &profiles).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, ClonalEvolutionDisposition::Qualified);
        assert_eq!(first.branch_order.len(), 1);
        assert_eq!(first.gained_marker_order, vec!["ecDNA"]);
    }

    #[test]
    fn input_permutation_preserves_graph_identity() {
        let profiles = vec![
            profile(
                "root",
                "clone-a",
                0,
                400,
                &[
                    ("egfr", CloneMarkerState::Present),
                    ("tp53", CloneMarkerState::Present),
                ],
            ),
            profile(
                "child",
                "clone-b",
                1,
                700,
                &[
                    ("egfr", CloneMarkerState::Present),
                    ("tp53", CloneMarkerState::Present),
                    ("ecDNA", CloneMarkerState::Present),
                ],
            ),
        ];
        let reversed = profiles.iter().cloned().rev().collect::<Vec<_>>();
        assert_eq!(
            analyze_glioma_clonal_evolution(&request(), &profiles).unwrap(),
            analyze_glioma_clonal_evolution(&request(), &reversed).unwrap()
        );
    }

    #[test]
    fn weak_overlap_is_unresolved_and_does_not_invent_parentage() {
        let profiles = vec![
            profile(
                "root",
                "clone-a",
                0,
                400,
                &[
                    ("egfr", CloneMarkerState::Present),
                    ("tp53", CloneMarkerState::Present),
                ],
            ),
            profile(
                "child",
                "clone-b",
                1,
                700,
                &[
                    ("pten", CloneMarkerState::Present),
                    ("cdkn2a", CloneMarkerState::Present),
                ],
            ),
        ];
        let output = analyze_glioma_clonal_evolution(&request(), &profiles).unwrap();
        assert_eq!(output.disposition, ClonalEvolutionDisposition::Unresolved);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("no-parent-child-edge")));
    }

    #[test]
    fn unmeasured_marker_is_retained_as_uncertainty() {
        let profiles = vec![
            profile(
                "root",
                "clone-a",
                0,
                400,
                &[
                    ("egfr", CloneMarkerState::Present),
                    ("tp53", CloneMarkerState::Present),
                    ("pten", CloneMarkerState::Unmeasured),
                ],
            ),
            profile(
                "child",
                "clone-b",
                1,
                700,
                &[
                    ("egfr", CloneMarkerState::Present),
                    ("tp53", CloneMarkerState::Present),
                ],
            ),
        ];
        let output = analyze_glioma_clonal_evolution(&request(), &profiles).unwrap();
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("parent-marker-unmeasured")));
    }

    #[test]
    fn duplicate_profiles_are_rejected() {
        let item = profile(
            "duplicate",
            "clone-a",
            0,
            400,
            &[
                ("egfr", CloneMarkerState::Present),
                ("tp53", CloneMarkerState::Present),
            ],
        );
        assert!(matches!(
            analyze_glioma_clonal_evolution(&request(), &[item.clone(), item]),
            Err(ClonalEvolutionError::InvalidProfile(_))
        ));
    }

    #[test]
    fn human_data_is_refused_at_profile_boundary() {
        let mut item = profile(
            "human",
            "clone-a",
            0,
            400,
            &[
                ("egfr", CloneMarkerState::Present),
                ("tp53", CloneMarkerState::Present),
            ],
        );
        item.artifact.contains_human_data = true;
        assert!(matches!(
            analyze_glioma_clonal_evolution(&request(), &[item]),
            Err(ClonalEvolutionError::InvalidProfile(_))
        ));
    }
}
