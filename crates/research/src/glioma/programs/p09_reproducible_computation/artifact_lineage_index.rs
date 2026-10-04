//! Semantic-loss-aware computation-artifact lineage indexing for preclinical glioma research.
//!
//! A content hash proves identity, not scientific derivation. This feature joins local artifact
//! metadata to declared task edges, verifies edge digests, walks every input branch to an allowed
//! root, and reports missing, unauthorized, cyclic, tampered, and semantically lossy paths. The
//! traversal is deterministic and metadata-only: protected payloads stay in the institution's
//! store and a complete path never becomes a biological conclusion by itself.

use super::execution::ComputationExecution;
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F07";
pub const OUTPUT_SCHEMA: &str = "GliomaArtifactLineageIndex1@1";
pub const MAX_NODES: usize = 4_096;
pub const MAX_EDGES: usize = 8_192;
pub const MAX_PATH_DEPTH: usize = 512;
pub const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactLineageRelation {
    ExactDerivation,
    Transformation,
    Sampling,
    SemanticLoss,
    ExternalReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactLineageEdgeIntegrity {
    Valid,
    Tampered,
    Unauthorized,
    UnknownArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactLineageNodeStatus {
    Root,
    Complete,
    SemanticLoss,
    Missing,
    Unauthorized,
    Tampered,
    Cycle,
    Orphan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactLineageDisposition {
    Ready,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactLineageNodeSpec {
    pub artifact: LocalArtifactRef,
    pub is_root: bool,
    pub access_granted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactLineageEdge {
    pub edge_id: String,
    pub parent_artifact_id: String,
    pub child_artifact_id: String,
    pub source_task_id: String,
    pub relation: ArtifactLineageRelation,
    pub semantic_loss_milli: u16,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactLineageRequest {
    pub objective: String,
    pub replay_identity: ContentHash,
    pub execution: ComputationExecution,
    pub nodes: Vec<ArtifactLineageNodeSpec>,
    pub edges: Vec<ArtifactLineageEdge>,
    pub output_artifact_ids: Vec<String>,
    pub max_path_depth: usize,
    pub allow_semantic_loss: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactLineageEdgeRecord {
    pub edge_id: String,
    pub parent_artifact_id: String,
    pub child_artifact_id: String,
    pub source_task_id: String,
    pub relation: ArtifactLineageRelation,
    pub semantic_loss_milli: u16,
    pub integrity: ArtifactLineageEdgeIntegrity,
    pub issue: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactLineageNodeRecord {
    pub artifact_id: String,
    pub output_schema: String,
    pub status: ArtifactLineageNodeStatus,
    pub incoming_edge_order: Vec<String>,
    pub outgoing_edge_order: Vec<String>,
    pub issue_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactLineagePathProof {
    pub output_artifact_id: String,
    pub node_order: Vec<String>,
    pub edge_order: Vec<String>,
    pub relation_order: Vec<ArtifactLineageRelation>,
    pub semantic_loss_milli: u16,
    pub complete: bool,
    pub status: ArtifactLineageNodeStatus,
    pub gap_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactLineageIndex {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub replay_identity: ContentHash,
    pub root_order: Vec<String>,
    pub output_order: Vec<String>,
    pub nodes: Vec<ArtifactLineageNodeRecord>,
    pub edges: Vec<ArtifactLineageEdgeRecord>,
    pub proofs: Vec<ArtifactLineagePathProof>,
    pub orphan_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub disposition: ArtifactLineageDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactLineageIndexError {
    #[error("artifact-lineage request is invalid: {0}")]
    InvalidRequest(String),
    #[error("artifact-lineage output is invalid: {0}")]
    InvalidOutput(String),
    #[error("artifact-lineage digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_TEXT_BYTES
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn edge_digest(edge: &ArtifactLineageEdge) -> ContentHash {
    ContentHash::of_value(&serde_json::json!({
        "edge_id": edge.edge_id,
        "parent_artifact_id": edge.parent_artifact_id,
        "child_artifact_id": edge.child_artifact_id,
        "source_task_id": edge.source_task_id,
        "relation": edge.relation,
        "semantic_loss_milli": edge.semantic_loss_milli,
    }))
    .expect("artifact lineage edge digest is serializable")
}

fn digest_input(output: &ArtifactLineageIndex) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "replay_identity": output.replay_identity,
        "root_order": output.root_order,
        "output_order": output.output_order,
        "nodes": output.nodes,
        "edges": output.edges,
        "proofs": output.proofs,
        "orphan_order": output.orphan_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "next_action": output.next_action,
        "disposition": output.disposition,
    })
}

impl ArtifactLineageIndex {
    pub fn validate(&self) -> Result<(), ArtifactLineageIndexError> {
        let node_ids = self
            .nodes
            .iter()
            .map(|node| node.artifact_id.clone())
            .collect::<BTreeSet<_>>();
        let edge_ids = self
            .edges
            .iter()
            .map(|edge| edge.edge_id.clone())
            .collect::<BTreeSet<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || self.replay_identity.as_str().len() != 64
            || self.nodes.is_empty()
            || self
                .nodes
                .windows(2)
                .any(|pair| pair[0].artifact_id >= pair[1].artifact_id)
            || self
                .edges
                .windows(2)
                .any(|pair| pair[0].edge_id >= pair[1].edge_id)
            || !canonical(&self.root_order)
            || !canonical(&self.output_order)
            || !canonical(&self.orphan_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || !safe_text(&self.next_action)
            || self.nodes.iter().any(|node| {
                !safe_text(&node.artifact_id)
                    || !safe_text(&node.output_schema)
                    || !canonical(&node.incoming_edge_order)
                    || !canonical(&node.outgoing_edge_order)
                    || !canonical(&node.issue_order)
            })
            || self.edges.iter().any(|edge| {
                !safe_text(&edge.edge_id)
                    || !safe_text(&edge.parent_artifact_id)
                    || !safe_text(&edge.child_artifact_id)
                    || !safe_text(&edge.source_task_id)
                    || edge.semantic_loss_milli > 1_000
                    || edge.issue.as_ref().is_some_and(|issue| !safe_text(issue))
            })
            || self
                .proofs
                .windows(2)
                .any(|pair| pair[0].output_artifact_id >= pair[1].output_artifact_id)
            || self.proofs.iter().any(|proof| {
                !safe_text(&proof.output_artifact_id)
                    || !canonical(&proof.node_order)
                    || !canonical(&proof.edge_order)
                    || !canonical(&proof.gap_order)
                    || proof.semantic_loss_milli > 1_000
            })
        {
            return Err(ArtifactLineageIndexError::InvalidOutput(
                "identity, stable ordering, text bounds, and lineage invariants are invalid".into(),
            ));
        }
        if self.root_order.iter().any(|id| !node_ids.contains(id))
            || self.orphan_order.iter().any(|id| !node_ids.contains(id))
            || self.proofs.iter().any(|proof| {
                !self.output_order.contains(&proof.output_artifact_id)
                    || proof.node_order.iter().any(|id| {
                        !node_ids.contains(id)
                            && !proof.gap_order.contains(&format!("missing-artifact:{id}"))
                    })
                    || proof.edge_order.iter().any(|id| !edge_ids.contains(id))
            })
        {
            return Err(ArtifactLineageIndexError::InvalidOutput(
                "lineage references do not reconcile with declared node and edge partitions".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ArtifactLineageIndexError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ArtifactLineageIndexError::InvalidOutput(
                "artifact-lineage index digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ArtifactLineageRequest) -> Result<(), ArtifactLineageIndexError> {
    if !safe_text(&request.objective)
        || request.replay_identity.as_str().len() != 64
        || request.execution.objective != request.objective
        || request.execution.replay_identity != request.replay_identity
        || request.nodes.is_empty()
        || request.nodes.len() > MAX_NODES
        || request.edges.len() > MAX_EDGES
        || request.output_artifact_ids.is_empty()
        || request.max_path_depth == 0
        || request.max_path_depth > MAX_PATH_DEPTH
    {
        return Err(ArtifactLineageIndexError::InvalidRequest(
            "objective/replay binding, bounded nodes/edges, outputs, and path depth are required"
                .into(),
        ));
    }
    request
        .execution
        .validate()
        .map_err(|error| ArtifactLineageIndexError::InvalidRequest(error.to_string()))?;
    let mut node_ids = BTreeSet::new();
    for node in &request.nodes {
        if !node_ids.insert(node.artifact.artifact_id.clone())
            || !safe_text(&node.artifact.artifact_id)
            || !safe_text(&node.artifact.content_type)
        {
            return Err(ArtifactLineageIndexError::InvalidRequest(
                "artifact nodes must have unique bounded IDs and schemas".into(),
            ));
        }
        node.artifact
            .validate()
            .map_err(|error| ArtifactLineageIndexError::InvalidRequest(error.to_string()))?;
    }
    let task_ids = request
        .execution
        .task_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut edge_ids = BTreeSet::new();
    for edge in &request.edges {
        if !edge_ids.insert(edge.edge_id.clone())
            || !safe_text(&edge.edge_id)
            || !safe_text(&edge.parent_artifact_id)
            || !safe_text(&edge.child_artifact_id)
            || !safe_text(&edge.source_task_id)
            || edge.parent_artifact_id == edge.child_artifact_id
            || !node_ids.contains(&edge.parent_artifact_id)
            || !node_ids.contains(&edge.child_artifact_id)
            || !task_ids.contains(&edge.source_task_id)
            || edge.semantic_loss_milli > 1_000
            || (edge.relation == ArtifactLineageRelation::ExactDerivation
                && edge.semantic_loss_milli != 0)
            || (edge.relation == ArtifactLineageRelation::SemanticLoss
                && edge.semantic_loss_milli == 0)
        {
            return Err(ArtifactLineageIndexError::InvalidRequest(format!(
                "edge {} has invalid node, task, relation, loss, or identity fields",
                edge.edge_id
            )));
        }
    }
    let mut output_ids = BTreeSet::new();
    for output in &request.output_artifact_ids {
        if !safe_text(output) || !output_ids.insert(output.clone()) {
            return Err(ArtifactLineageIndexError::InvalidRequest(
                "output artifact IDs must be unique and bounded".into(),
            ));
        }
    }
    Ok(())
}

#[derive(Default)]
struct TraceSummary {
    nodes: BTreeSet<String>,
    edges: BTreeSet<String>,
    relations: BTreeSet<ArtifactLineageRelation>,
    gaps: BTreeSet<String>,
    statuses: BTreeSet<ArtifactLineageNodeStatus>,
    semantic_loss_milli: u16,
}

fn trace_artifact(
    artifact_id: &str,
    depth: usize,
    max_depth: usize,
    node_map: &BTreeMap<String, &ArtifactLineageNodeSpec>,
    incoming: &BTreeMap<String, Vec<&ArtifactLineageEdgeRecord>>,
    visiting: &mut BTreeSet<String>,
) -> TraceSummary {
    let mut summary = TraceSummary::default();
    summary.nodes.insert(artifact_id.to_string());
    let Some(node) = node_map.get(artifact_id) else {
        summary
            .gaps
            .insert(format!("missing-artifact:{artifact_id}"));
        summary.statuses.insert(ArtifactLineageNodeStatus::Missing);
        return summary;
    };
    if !node.access_granted {
        summary
            .gaps
            .insert(format!("unauthorized-artifact:{artifact_id}"));
        summary
            .statuses
            .insert(ArtifactLineageNodeStatus::Unauthorized);
        return summary;
    }
    if node.is_root {
        summary.statuses.insert(ArtifactLineageNodeStatus::Root);
        return summary;
    }
    if depth >= max_depth {
        summary
            .gaps
            .insert(format!("path-depth-exceeded:{artifact_id}"));
        summary.statuses.insert(ArtifactLineageNodeStatus::Cycle);
        return summary;
    }
    if !visiting.insert(artifact_id.to_string()) {
        summary.gaps.insert(format!("cycle-at:{artifact_id}"));
        summary.statuses.insert(ArtifactLineageNodeStatus::Cycle);
        return summary;
    }
    let mut valid_incoming = false;
    if let Some(edges) = incoming.get(artifact_id) {
        for edge in edges {
            if edge.integrity != ArtifactLineageEdgeIntegrity::Valid {
                summary
                    .gaps
                    .insert(edge.issue.clone().unwrap_or_else(|| edge.edge_id.clone()));
                summary.statuses.insert(match edge.integrity {
                    ArtifactLineageEdgeIntegrity::Tampered => ArtifactLineageNodeStatus::Tampered,
                    ArtifactLineageEdgeIntegrity::Unauthorized => {
                        ArtifactLineageNodeStatus::Unauthorized
                    }
                    ArtifactLineageEdgeIntegrity::UnknownArtifact => {
                        ArtifactLineageNodeStatus::Missing
                    }
                    ArtifactLineageEdgeIntegrity::Valid => ArtifactLineageNodeStatus::Complete,
                });
                continue;
            }
            valid_incoming = true;
            summary.edges.insert(edge.edge_id.clone());
            summary.relations.insert(edge.relation);
            summary.semantic_loss_milli = summary.semantic_loss_milli.max(edge.semantic_loss_milli);
            let parent = trace_artifact(
                &edge.parent_artifact_id,
                depth + 1,
                max_depth,
                node_map,
                incoming,
                visiting,
            );
            summary.nodes.extend(parent.nodes);
            summary.edges.extend(parent.edges);
            summary.relations.extend(parent.relations);
            summary.gaps.extend(parent.gaps);
            summary.statuses.extend(parent.statuses);
            summary.semantic_loss_milli =
                summary.semantic_loss_milli.max(parent.semantic_loss_milli);
        }
    }
    visiting.remove(artifact_id);
    if !valid_incoming {
        summary
            .gaps
            .insert(format!("no-lineage-parent:{artifact_id}"));
        summary.statuses.insert(ArtifactLineageNodeStatus::Missing);
    }
    if summary.statuses.is_empty() {
        summary.statuses.insert(ArtifactLineageNodeStatus::Complete);
    }
    summary
}

fn proof_status(summary: &TraceSummary) -> ArtifactLineageNodeStatus {
    for status in [
        ArtifactLineageNodeStatus::Unauthorized,
        ArtifactLineageNodeStatus::Tampered,
        ArtifactLineageNodeStatus::Cycle,
        ArtifactLineageNodeStatus::Missing,
    ] {
        if summary.statuses.contains(&status) {
            return status;
        }
    }
    if summary.semantic_loss_milli > 0
        || summary
            .relations
            .contains(&ArtifactLineageRelation::SemanticLoss)
    {
        ArtifactLineageNodeStatus::SemanticLoss
    } else {
        ArtifactLineageNodeStatus::Complete
    }
}

/// Build a deterministic, access-aware artifact lineage index for a local computation run.
pub fn index_glioma_artifact_lineage(
    request: &ArtifactLineageRequest,
) -> Result<ArtifactLineageIndex, ArtifactLineageIndexError> {
    validate_request(request)?;
    let node_map = request
        .nodes
        .iter()
        .map(|node| (node.artifact.artifact_id.clone(), node))
        .collect::<BTreeMap<_, _>>();
    let mut edge_records = request
        .edges
        .iter()
        .map(|edge| {
            let integrity = if !node_map.contains_key(&edge.parent_artifact_id)
                || !node_map.contains_key(&edge.child_artifact_id)
            {
                ArtifactLineageEdgeIntegrity::UnknownArtifact
            } else if !node_map[&edge.parent_artifact_id].access_granted
                || !node_map[&edge.child_artifact_id].access_granted
            {
                ArtifactLineageEdgeIntegrity::Unauthorized
            } else if edge_digest(edge) != edge.digest {
                ArtifactLineageEdgeIntegrity::Tampered
            } else {
                ArtifactLineageEdgeIntegrity::Valid
            };
            let issue = match integrity {
                ArtifactLineageEdgeIntegrity::Valid => None,
                ArtifactLineageEdgeIntegrity::Tampered => {
                    Some(format!("tampered-edge-digest:{}", edge.edge_id))
                }
                ArtifactLineageEdgeIntegrity::Unauthorized => {
                    Some(format!("unauthorized-lineage-edge:{}", edge.edge_id))
                }
                ArtifactLineageEdgeIntegrity::UnknownArtifact => {
                    Some(format!("unknown-artifact-edge:{}", edge.edge_id))
                }
            };
            ArtifactLineageEdgeRecord {
                edge_id: edge.edge_id.clone(),
                parent_artifact_id: edge.parent_artifact_id.clone(),
                child_artifact_id: edge.child_artifact_id.clone(),
                source_task_id: edge.source_task_id.clone(),
                relation: edge.relation,
                semantic_loss_milli: edge.semantic_loss_milli,
                integrity,
                issue,
            }
        })
        .collect::<Vec<_>>();
    edge_records.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    let edge_map = edge_records
        .iter()
        .map(|edge| (edge.edge_id.clone(), edge))
        .collect::<BTreeMap<_, _>>();
    let mut incoming = BTreeMap::<String, Vec<&ArtifactLineageEdgeRecord>>::new();
    for edge in edge_map.values() {
        incoming
            .entry(edge.child_artifact_id.clone())
            .or_default()
            .push(edge);
    }
    for edges in incoming.values_mut() {
        edges.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    }
    let root_order = node_map
        .values()
        .filter(|node| node.is_root)
        .map(|node| node.artifact.artifact_id.clone())
        .collect::<Vec<_>>();
    let mut output_order = request.output_artifact_ids.clone();
    output_order.sort();
    let mut proofs = Vec::new();
    let mut reachable = BTreeSet::new();
    for output_artifact_id in &output_order {
        let mut visiting = BTreeSet::new();
        let summary = trace_artifact(
            output_artifact_id,
            0,
            request.max_path_depth,
            &node_map,
            &incoming,
            &mut visiting,
        );
        reachable.extend(summary.nodes.iter().cloned());
        let status = proof_status(&summary);
        let mut node_order = summary.nodes.into_iter().collect::<Vec<_>>();
        let mut edge_order = summary.edges.into_iter().collect::<Vec<_>>();
        let mut relation_order = summary.relations.into_iter().collect::<Vec<_>>();
        node_order.sort();
        edge_order.sort();
        relation_order.sort();
        proofs.push(ArtifactLineagePathProof {
            output_artifact_id: output_artifact_id.clone(),
            node_order,
            edge_order,
            relation_order,
            semantic_loss_milli: summary.semantic_loss_milli,
            complete: matches!(
                status,
                ArtifactLineageNodeStatus::Complete | ArtifactLineageNodeStatus::SemanticLoss
            ),
            status,
            gap_order: summary.gaps.into_iter().collect(),
        });
    }
    let orphan_order = node_map
        .keys()
        .filter(|artifact_id| !reachable.contains(*artifact_id))
        .cloned()
        .collect::<Vec<_>>();
    let mut nodes = node_map
        .values()
        .map(|node| {
            let mut incoming_edge_order = incoming
                .get(&node.artifact.artifact_id)
                .into_iter()
                .flatten()
                .map(|edge| edge.edge_id.clone())
                .collect::<Vec<_>>();
            let mut outgoing_edge_order = edge_records
                .iter()
                .filter(|edge| edge.parent_artifact_id == node.artifact.artifact_id)
                .map(|edge| edge.edge_id.clone())
                .collect::<Vec<_>>();
            incoming_edge_order.sort();
            outgoing_edge_order.sort();
            let proof_status = proofs
                .iter()
                .find(|proof| proof.output_artifact_id == node.artifact.artifact_id)
                .map(|proof| proof.status);
            let status = if orphan_order.contains(&node.artifact.artifact_id) {
                ArtifactLineageNodeStatus::Orphan
            } else {
                proof_status.unwrap_or({
                    if !node.access_granted {
                        ArtifactLineageNodeStatus::Unauthorized
                    } else if node.is_root {
                        ArtifactLineageNodeStatus::Root
                    } else {
                        ArtifactLineageNodeStatus::Complete
                    }
                })
            };
            let mut issue_order = Vec::new();
            for edge in &edge_records {
                if (edge.parent_artifact_id == node.artifact.artifact_id
                    || edge.child_artifact_id == node.artifact.artifact_id)
                    && edge.issue.is_some()
                {
                    issue_order.push(edge.issue.clone().expect("checked issue"));
                }
            }
            issue_order.sort();
            issue_order.dedup();
            ArtifactLineageNodeRecord {
                artifact_id: node.artifact.artifact_id.clone(),
                output_schema: node.artifact.content_type.clone(),
                status,
                incoming_edge_order,
                outgoing_edge_order,
                issue_order,
            }
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    let mut negative_evidence = proofs
        .iter()
        .filter(|proof| !proof.complete || proof.status != ArtifactLineageNodeStatus::Complete)
        .flat_map(|proof| {
            proof
                .gap_order
                .iter()
                .map(move |gap| format!("{}: {}", proof.output_artifact_id, gap))
        })
        .collect::<Vec<_>>();
    negative_evidence.extend(edge_records.iter().filter_map(|edge| edge.issue.clone()));
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = proofs
        .iter()
        .filter(|proof| proof.status == ArtifactLineageNodeStatus::SemanticLoss)
        .map(|proof| {
            format!(
                "{} includes semantic loss of {} milli-units",
                proof.output_artifact_id, proof.semantic_loss_milli
            )
        })
        .collect::<Vec<_>>();
    if !orphan_order.is_empty() {
        uncertainty.push(format!("orphan-artifacts:{}", orphan_order.join(",")));
    }
    uncertainty.sort();
    uncertainty.dedup();
    let has_block = proofs.iter().any(|proof| {
        matches!(
            proof.status,
            ArtifactLineageNodeStatus::Missing
                | ArtifactLineageNodeStatus::Unauthorized
                | ArtifactLineageNodeStatus::Tampered
                | ArtifactLineageNodeStatus::Cycle
        )
    });
    let has_semantic_loss = proofs
        .iter()
        .any(|proof| proof.status == ArtifactLineageNodeStatus::SemanticLoss);
    let disposition = if has_block || (has_semantic_loss && !request.allow_semantic_loss) {
        ArtifactLineageDisposition::Blocked
    } else if has_semantic_loss || !orphan_order.is_empty() {
        ArtifactLineageDisposition::Partial
    } else if proofs.is_empty() {
        ArtifactLineageDisposition::Unresolved
    } else {
        ArtifactLineageDisposition::Ready
    };
    let next_action = match disposition {
        ArtifactLineageDisposition::Ready => {
            "route the complete metadata lineage to the research-object builder or interpretation gate".into()
        }
        ArtifactLineageDisposition::Partial => {
            "review semantic loss and orphan artifacts before pooling or publication".into()
        }
        ArtifactLineageDisposition::Blocked => {
            "repair missing, unauthorized, tampered, or cyclic lineage before downstream use".into()
        }
        ArtifactLineageDisposition::Unresolved => {
            "declare at least one output artifact and a bounded lineage root".into()
        }
    };
    let mut output = ArtifactLineageIndex {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        replay_identity: request.replay_identity.clone(),
        root_order,
        output_order,
        nodes,
        edges: edge_records,
        proofs,
        orphan_order,
        negative_evidence,
        uncertainty,
        next_action,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-artifact-lineage-index"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ArtifactLineageIndexError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::execution::{
        ComputationExecutionDisposition, ComputationExecutionStopReason, ComputationOperation,
        ComputationTask, ComputationTaskDisposition, ComputationTaskResult,
    };
    use super::*;
    use crate::glioma_engine::GliomaModelSystem;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn artifact(id: &str, schema: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: hash(id),
            content_type: schema.into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn execution() -> ComputationExecution {
        let task = ComputationTask {
            task_id: "integrate".into(),
            operation: ComputationOperation::Integrate,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            input_artifact_ids: vec!["raw".into()],
            output_schema: "GliomaIntegrated1@1".into(),
            estimated_cost_units: 1,
            estimated_duration_ticks: 1,
            deterministic: true,
        };
        let result = ComputationTaskResult {
            task_id: "integrate".into(),
            output_schema: "GliomaIntegrated1@1".into(),
            disposition: ComputationTaskDisposition::Completed,
            attempt_count: 1,
            artifact: Some(artifact("result", "GliomaIntegrated1@1")),
            cache_hit: false,
            note: "local result".into(),
        };
        let mut output = ComputationExecution {
            feature_id: super::super::execution::FEATURE_ID.into(),
            output_schema: super::super::execution::OUTPUT_SCHEMA.into(),
            objective: "integrate glioma modalities".into(),
            model_system: GliomaModelSystem::Organoid,
            replay_identity: hash("lineage-replay"),
            task_order: vec!["integrate".into()],
            task_results: vec![result],
            completed_order: vec!["integrate".into()],
            cached_order: Vec::new(),
            negative_order: Vec::new(),
            partial_order: Vec::new(),
            failed_order: Vec::new(),
            skipped_order: Vec::new(),
            budget_used_units: 1,
            duration_used_ticks: 1,
            cache_hit_count: 0,
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            disposition: ComputationExecutionDisposition::Completed,
            stop_reason: ComputationExecutionStopReason::Completed,
            digest: hash("unsealed"),
        };
        output.digest =
            ContentHash::of_value(&super::super::execution::digest_input(&output)).expect("digest");
        output.validate().expect("execution");
        let _ = task;
        output
    }

    fn edge(
        edge_id: &str,
        parent: &str,
        child: &str,
        relation: ArtifactLineageRelation,
        loss: u16,
    ) -> ArtifactLineageEdge {
        let mut edge = ArtifactLineageEdge {
            edge_id: edge_id.into(),
            parent_artifact_id: parent.into(),
            child_artifact_id: child.into(),
            source_task_id: "integrate".into(),
            relation,
            semantic_loss_milli: loss,
            digest: hash("unsealed-edge"),
        };
        edge.digest = edge_digest(&edge);
        edge
    }

    fn request(
        edges: Vec<ArtifactLineageEdge>,
        allow_semantic_loss: bool,
    ) -> ArtifactLineageRequest {
        let execution = execution();
        ArtifactLineageRequest {
            objective: execution.objective.clone(),
            replay_identity: execution.replay_identity.clone(),
            execution,
            nodes: vec![
                ArtifactLineageNodeSpec {
                    artifact: artifact("raw", "GliomaRaw1@1"),
                    is_root: true,
                    access_granted: true,
                },
                ArtifactLineageNodeSpec {
                    artifact: artifact("result", "GliomaIntegrated1@1"),
                    is_root: false,
                    access_granted: true,
                },
            ],
            edges,
            output_artifact_ids: vec!["result".into()],
            max_path_depth: 16,
            allow_semantic_loss,
        }
    }

    #[test]
    fn exact_lineage_is_ready_and_replay_stable() {
        let request = request(
            vec![edge(
                "raw-to-result",
                "raw",
                "result",
                ArtifactLineageRelation::ExactDerivation,
                0,
            )],
            false,
        );
        let reverse = {
            let mut reversed = request.clone();
            reversed.nodes.reverse();
            reversed.edges.reverse();
            reversed
        };
        let left = index_glioma_artifact_lineage(&request).expect("left");
        let right = index_glioma_artifact_lineage(&reverse).expect("right");
        assert_eq!(left.disposition, ArtifactLineageDisposition::Ready);
        assert!(left.proofs[0].complete);
        assert_eq!(left.digest, right.digest);
    }

    #[test]
    fn missing_parent_remains_explicit() {
        let mut request = request(Vec::new(), false);
        request.nodes[1].is_root = false;
        let output = index_glioma_artifact_lineage(&request).expect("index");
        assert_eq!(output.disposition, ArtifactLineageDisposition::Blocked);
        assert_eq!(output.proofs[0].status, ArtifactLineageNodeStatus::Missing);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("no-lineage-parent")));
    }

    #[test]
    fn semantic_loss_requires_declared_policy() {
        let blocked = index_glioma_artifact_lineage(&request(
            vec![edge(
                "sampled-result",
                "raw",
                "result",
                ArtifactLineageRelation::Sampling,
                300,
            )],
            false,
        ))
        .expect("blocked index");
        let allowed = index_glioma_artifact_lineage(&request(
            vec![edge(
                "sampled-result",
                "raw",
                "result",
                ArtifactLineageRelation::Sampling,
                300,
            )],
            true,
        ))
        .expect("allowed index");
        assert_eq!(blocked.disposition, ArtifactLineageDisposition::Blocked);
        assert_eq!(allowed.disposition, ArtifactLineageDisposition::Partial);
        assert_eq!(
            allowed.proofs[0].status,
            ArtifactLineageNodeStatus::SemanticLoss
        );
    }

    #[test]
    fn tampered_edge_is_detected_without_trusting_hash_identity() {
        let mut tampered = edge(
            "raw-to-result",
            "raw",
            "result",
            ArtifactLineageRelation::ExactDerivation,
            0,
        );
        tampered.digest = hash("tampered-edge");
        let output = index_glioma_artifact_lineage(&request(vec![tampered], false)).expect("index");
        assert_eq!(
            output.edges[0].integrity,
            ArtifactLineageEdgeIntegrity::Tampered
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("tampered-edge-digest")));
    }
}
