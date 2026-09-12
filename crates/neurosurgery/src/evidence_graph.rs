//! Provenance-preserving graph projections for the real glioma snapshot.
//!
//! The graph is deliberately an explicit crosswalk, not a knowledge graph or an inference
//! engine. Nodes are compact public-record metadata and edges are only relationships that can be
//! derived from stable identifiers already present in the validated bundle (study/profile and
//! study/PMID links). Isolated records, omitted nodes, and bounded scans remain visible so a
//! disconnected or incomplete corpus cannot look like a connected evidence base.

use crate::{NeurosurgeryError, RealDataRecordKind, RealDataRelation, RealGliomaBundle, Specialty};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const EVIDENCE_GRAPH_SCHEMA_VERSION: &str = "bioprism-neurosurgery-evidence-graph/0.1";
pub const MAX_EVIDENCE_GRAPH_NODES: usize = 512;
pub const MAX_EVIDENCE_GRAPH_EDGES: usize = 1024;

fn default_max_nodes() -> usize {
    128
}

fn default_max_edges() -> usize {
    256
}

/// Select a bounded portion of the explicit public-record crosswalk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceGraphQuery {
    /// Optional exact stable record id. When supplied, traversal follows explicit edges in both
    /// directions so a PMID can reach its linked study and assay profiles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_record_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_record_kind: Option<RealDataRecordKind>,
    #[serde(default = "default_max_nodes")]
    pub max_nodes: usize,
    #[serde(default = "default_max_edges")]
    pub max_edges: usize,
}

impl Default for EvidenceGraphQuery {
    fn default() -> Self {
        Self {
            root_record_id: None,
            root_record_kind: None,
            max_nodes: default_max_nodes(),
            max_edges: default_max_edges(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct NodeKey {
    record_kind: RealDataRecordKind,
    record_id: String,
}

/// A public record node. It carries provenance and identity, but no sample- or patient-level
/// values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceGraphNode {
    pub record_kind: RealDataRecordKind,
    pub record_id: String,
    pub title: String,
    pub source_id: String,
    pub source_uri: String,
}

/// A directional edge whose relation was explicitly derivable from stable bundle identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceGraphEdge {
    pub from_record_kind: RealDataRecordKind,
    pub from_record_id: String,
    pub to_record_kind: RealDataRecordKind,
    pub to_record_id: String,
    pub relation: RealDataRelation,
}

/// Bounded, digest-addressed graph projection for human review and caller-owned traversal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceGraphReport {
    pub schema_version: String,
    pub bundle_digest: String,
    pub graph_digest: String,
    pub specialty: Specialty,
    pub query: EvidenceGraphQuery,
    pub nodes: Vec<EvidenceGraphNode>,
    pub edges: Vec<EvidenceGraphEdge>,
    /// Counts are for the root-selected graph before output bounds are applied.
    pub total_node_count: usize,
    pub total_edge_count: usize,
    pub omitted_node_count: usize,
    pub omitted_edge_count: usize,
    pub truncated: bool,
    pub root_count: usize,
    pub connected_component_count: usize,
    pub isolated_node_count: usize,
    pub source_count: usize,
    pub bundle_relationship_count: usize,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl EvidenceGraphReport {
    /// Validate a persisted graph projection without reopening the source snapshot.
    ///
    /// The graph is an identifier crosswalk only. This checks bounds, node/edge closure,
    /// deterministic ordering, graph-shape counters, provenance posture, and the digest; it does
    /// not infer biology, causality, study quality, or patient applicability.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != EVIDENCE_GRAPH_SCHEMA_VERSION
            || !is_sha256_hex(&self.bundle_digest)
            || !is_sha256_hex(&self.graph_digest)
            || self.specialty != Specialty::Glioma
            || self.nodes.len() > self.query.max_nodes
            || self.edges.len() > self.query.max_edges
            || self.total_node_count < self.nodes.len()
            || self.total_edge_count < self.edges.len()
            || self.omitted_node_count != self.total_node_count.saturating_sub(self.nodes.len())
            || self.omitted_edge_count != self.total_edge_count.saturating_sub(self.edges.len())
            || self.truncated != (self.omitted_node_count > 0 || self.omitted_edge_count > 0)
            || self.root_count > self.total_node_count
            || self.connected_component_count > self.total_node_count
            || self.isolated_node_count > self.total_node_count
            || self.source_count == 0
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(graph_rejected("evidence graph envelope is invalid"));
        }
        validate_query(&self.query)?;
        if self.query.root_record_kind.is_some() && self.query.root_record_id.is_none() {
            return Err(graph_rejected(
                "evidence graph root_record_kind requires root_record_id",
            ));
        }
        let mut node_keys = BTreeSet::new();
        let mut previous_key: Option<NodeKey> = None;
        for node in &self.nodes {
            let key = NodeKey {
                record_kind: node.record_kind,
                record_id: node.record_id.clone(),
            };
            if node.record_id.trim().is_empty()
                || node.title.trim().is_empty()
                || node.source_id.trim().is_empty()
                || !node.source_uri.starts_with("https://")
                || !node_keys.insert(key.clone())
                || previous_key
                    .as_ref()
                    .is_some_and(|previous| previous >= &key)
            {
                return Err(graph_rejected("evidence graph node projection is invalid"));
            }
            previous_key = Some(key);
        }
        let mut edge_keys = BTreeSet::new();
        let mut previous_edge: Option<EvidenceGraphEdge> = None;
        for edge in &self.edges {
            let from = NodeKey {
                record_kind: edge.from_record_kind,
                record_id: edge.from_record_id.clone(),
            };
            let to = NodeKey {
                record_kind: edge.to_record_kind,
                record_id: edge.to_record_id.clone(),
            };
            if edge.from_record_id.trim().is_empty()
                || edge.to_record_id.trim().is_empty()
                || !node_keys.contains(&from)
                || !node_keys.contains(&to)
                || !edge_keys.insert((
                    edge.from_record_kind,
                    edge.from_record_id.clone(),
                    edge.to_record_kind,
                    edge.to_record_id.clone(),
                    edge.relation,
                ))
                || previous_edge
                    .as_ref()
                    .is_some_and(|previous| edge_key(previous, edge) != std::cmp::Ordering::Less)
            {
                return Err(graph_rejected("evidence graph edge projection is invalid"));
            }
            previous_edge = Some(edge.clone());
        }
        if self.query.root_record_id.is_some() && self.root_count == 0 {
            return Err(graph_rejected("evidence graph root count is invalid"));
        }
        if self.graph_digest
            != digest_graph(
                &self.bundle_digest,
                &self.query,
                &self.nodes,
                &self.edges,
                self.total_node_count,
                self.total_edge_count,
            )?
        {
            return Err(graph_rejected(
                "evidence graph digest does not match its contents",
            ));
        }
        Ok(())
    }

    /// Rebuild the graph from the exact validated snapshot and persisted query bounds.
    pub fn validate_for_inputs(&self, bundle: &RealGliomaBundle) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.evidence_graph(&self.query)?;
        if &expected != self {
            return Err(graph_rejected(
                "evidence graph does not replay to the exact supplied snapshot",
            ));
        }
        Ok(())
    }
}

impl RealGliomaBundle {
    /// Build a source-linked graph without fetching, interpreting, or promoting public records.
    pub fn evidence_graph(
        &self,
        query: &EvidenceGraphQuery,
    ) -> Result<EvidenceGraphReport, NeurosurgeryError> {
        self.validate()?;
        compile_graph(self, query)
    }
}

fn compile_graph(
    bundle: &RealGliomaBundle,
    query: &EvidenceGraphQuery,
) -> Result<EvidenceGraphReport, NeurosurgeryError> {
    validate_query(query)?;
    let (node_map, mut all_edges) = collect_graph(bundle)?;
    let mut all_keys = node_map.keys().cloned().collect::<Vec<_>>();
    all_keys.sort();

    let roots = if let Some(root_id) = query.root_record_id.as_deref() {
        let roots = all_keys
            .iter()
            .filter(|key| {
                key.record_id == root_id
                    && query
                        .root_record_kind
                        .is_none_or(|kind| kind == key.record_kind)
            })
            .cloned()
            .collect::<Vec<_>>();
        if roots.is_empty() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "evidence graph root record {root_id:?} is not present in the validated bundle"
                ),
            });
        }
        roots
    } else {
        if query.root_record_kind.is_some() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "evidence graph root_record_kind requires root_record_id".to_string(),
            });
        }
        Vec::new()
    };

    all_edges.sort_by(edge_key);
    let selected_keys = if roots.is_empty() {
        all_keys.into_iter().collect::<BTreeSet<_>>()
    } else {
        let mut adjacency: BTreeMap<NodeKey, Vec<NodeKey>> = BTreeMap::new();
        for edge in &all_edges {
            let from = NodeKey {
                record_kind: edge.from_record_kind,
                record_id: edge.from_record_id.clone(),
            };
            let to = NodeKey {
                record_kind: edge.to_record_kind,
                record_id: edge.to_record_id.clone(),
            };
            adjacency.entry(from.clone()).or_default().push(to.clone());
            adjacency.entry(to).or_default().push(from);
        }
        for neighbors in adjacency.values_mut() {
            neighbors.sort();
            neighbors.dedup();
        }
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::from(roots.clone());
        while let Some(key) = queue.pop_front() {
            if !visited.insert(key.clone()) {
                continue;
            }
            if let Some(neighbors) = adjacency.get(&key) {
                queue.extend(neighbors.iter().cloned());
            }
        }
        visited
    };

    let selected_edges = all_edges
        .into_iter()
        .filter(|edge| {
            selected_keys.contains(&NodeKey {
                record_kind: edge.from_record_kind,
                record_id: edge.from_record_id.clone(),
            }) && selected_keys.contains(&NodeKey {
                record_kind: edge.to_record_kind,
                record_id: edge.to_record_id.clone(),
            })
        })
        .collect::<Vec<_>>();
    let total_node_count = selected_keys.len();
    let total_edge_count = selected_edges.len();
    let (connected_component_count, isolated_node_count) =
        graph_shape(&selected_keys, &selected_edges);

    let emitted_keys = selected_keys
        .iter()
        .take(query.max_nodes)
        .cloned()
        .collect::<BTreeSet<_>>();
    let nodes = emitted_keys
        .iter()
        .filter_map(|key| node_map.get(key).cloned())
        .collect::<Vec<_>>();
    let mut edges = selected_edges
        .iter()
        .filter(|edge| {
            emitted_keys.contains(&NodeKey {
                record_kind: edge.from_record_kind,
                record_id: edge.from_record_id.clone(),
            }) && emitted_keys.contains(&NodeKey {
                record_kind: edge.to_record_kind,
                record_id: edge.to_record_id.clone(),
            })
        })
        .take(query.max_edges)
        .cloned()
        .collect::<Vec<_>>();
    edges.sort_by(edge_key);
    let omitted_node_count = total_node_count.saturating_sub(nodes.len());
    let omitted_edge_count = total_edge_count.saturating_sub(edges.len());
    let bundle_digest = bundle.summary()?.bundle_digest;
    let graph_digest = digest_graph(
        &bundle_digest,
        query,
        &nodes,
        &edges,
        total_node_count,
        total_edge_count,
    )?;
    let report = EvidenceGraphReport {
        schema_version: EVIDENCE_GRAPH_SCHEMA_VERSION.to_string(),
        bundle_digest,
        graph_digest,
        specialty: Specialty::Glioma,
        query: query.clone(),
        nodes,
        edges,
        total_node_count,
        total_edge_count,
        omitted_node_count,
        omitted_edge_count,
        truncated: omitted_node_count > 0 || omitted_edge_count > 0,
        root_count: roots.len(),
        connected_component_count,
        isolated_node_count,
        source_count: bundle.sources.len(),
        bundle_relationship_count: bundle.portal_molecular_profiles.len()
            + bundle
                .portal_studies
                .iter()
                .filter(|study| {
                    study.pmid.as_deref().is_some_and(|pmid| {
                        bundle.literature.iter().any(|article| article.pmid == pmid)
                    })
                })
                .count(),
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: vec![
            "nodes are public-record metadata and contain no patient-level or sample-level values".to_string(),
            "edges are explicit stable-id crosswalks only; graph adjacency is not biological or clinical causation".to_string(),
            "isolated records and omitted bounded nodes remain unmeasured connectivity, not proof of irrelevance".to_string(),
            "the graph never evaluates study quality, applicability, eligibility, diagnosis, prognosis, treatment, or procedure".to_string(),
            "the graph never fetches URLs, invokes a model, opens credentials, or writes durable state".to_string(),
        ],
    };
    report.validate_integrity()?;
    Ok(report)
}

fn validate_query(query: &EvidenceGraphQuery) -> Result<(), NeurosurgeryError> {
    if query.max_nodes == 0 || query.max_nodes > MAX_EVIDENCE_GRAPH_NODES {
        return Err(NeurosurgeryError::TooMany {
            field: "evidence_graph.max_nodes",
            found: query.max_nodes,
            max: MAX_EVIDENCE_GRAPH_NODES,
        });
    }
    if query.max_edges == 0 || query.max_edges > MAX_EVIDENCE_GRAPH_EDGES {
        return Err(NeurosurgeryError::TooMany {
            field: "evidence_graph.max_edges",
            found: query.max_edges,
            max: MAX_EVIDENCE_GRAPH_EDGES,
        });
    }
    if let Some(root_id) = query.root_record_id.as_deref() {
        if root_id.is_empty() || root_id.len() > 512 || root_id.chars().any(char::is_control) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "evidence graph root_record_id is empty, too long, or contains a control character".to_string(),
            });
        }
    }
    Ok(())
}

fn collect_graph(
    bundle: &RealGliomaBundle,
) -> Result<(BTreeMap<NodeKey, EvidenceGraphNode>, Vec<EvidenceGraphEdge>), NeurosurgeryError> {
    let source_uri = |source_id: &str| {
        bundle
            .sources
            .iter()
            .find(|source| source.source_id == source_id)
            .map(|source| source.uri.clone())
            .ok_or_else(|| NeurosurgeryError::RealDataRejected {
                reason: format!("record references missing provenance source {source_id:?}"),
            })
    };
    let mut nodes = BTreeMap::new();
    let mut add_node = |record_kind: RealDataRecordKind,
                        record_id: String,
                        title: String,
                        source_id: String|
     -> Result<(), NeurosurgeryError> {
        let key = NodeKey {
            record_kind,
            record_id: record_id.clone(),
        };
        let source_uri = source_uri(&source_id)?;
        nodes.insert(
            key,
            EvidenceGraphNode {
                record_kind,
                record_id,
                title,
                source_uri,
                source_id,
            },
        );
        Ok(())
    };
    for record in &bundle.clinical_trials {
        add_node(
            RealDataRecordKind::ClinicalTrial,
            record.nct_id.clone(),
            record.title.clone(),
            record.source_id.clone(),
        )?;
    }
    for record in &bundle.genomic_projects {
        add_node(
            RealDataRecordKind::GenomicProject,
            record.project_id.clone(),
            record.name.clone(),
            record.source_id.clone(),
        )?;
    }
    for record in &bundle.portal_studies {
        add_node(
            RealDataRecordKind::PortalStudy,
            record.study_id.clone(),
            record.name.clone(),
            record.source_id.clone(),
        )?;
    }
    for record in &bundle.portal_molecular_profiles {
        add_node(
            RealDataRecordKind::PortalMolecularProfile,
            record.profile_id.clone(),
            record.name.clone(),
            record.source_id.clone(),
        )?;
    }
    for record in &bundle.references {
        add_node(
            RealDataRecordKind::GuidelineReference,
            record.reference_id.clone(),
            record.title.clone(),
            record.source_id.clone(),
        )?;
    }
    for record in &bundle.literature {
        add_node(
            RealDataRecordKind::LiteratureArticle,
            record.pmid.clone(),
            record.title.clone(),
            record.source_id.clone(),
        )?;
    }

    let mut edges = Vec::new();
    let has_literature = |pmid: &str| bundle.literature.iter().any(|article| article.pmid == pmid);
    for study in &bundle.portal_studies {
        for profile in bundle
            .portal_molecular_profiles
            .iter()
            .filter(|profile| profile.study_id == study.study_id)
        {
            edges.push(edge(
                RealDataRecordKind::PortalStudy,
                study.study_id.clone(),
                RealDataRecordKind::PortalMolecularProfile,
                profile.profile_id.clone(),
                RealDataRelation::HasProfile,
            ));
            edges.push(edge(
                RealDataRecordKind::PortalMolecularProfile,
                profile.profile_id.clone(),
                RealDataRecordKind::PortalStudy,
                study.study_id.clone(),
                RealDataRelation::ProfileOfStudy,
            ));
        }
        if let Some(pmid) = study.pmid.as_deref().filter(|pmid| has_literature(pmid)) {
            edges.push(edge(
                RealDataRecordKind::PortalStudy,
                study.study_id.clone(),
                RealDataRecordKind::LiteratureArticle,
                pmid.to_string(),
                RealDataRelation::PublishedAs,
            ));
            edges.push(edge(
                RealDataRecordKind::LiteratureArticle,
                pmid.to_string(),
                RealDataRecordKind::PortalStudy,
                study.study_id.clone(),
                RealDataRelation::DescribesStudy,
            ));
        }
    }
    edges.sort_by(edge_key);
    edges.dedup();
    Ok((nodes, edges))
}

fn edge(
    from_record_kind: RealDataRecordKind,
    from_record_id: String,
    to_record_kind: RealDataRecordKind,
    to_record_id: String,
    relation: RealDataRelation,
) -> EvidenceGraphEdge {
    EvidenceGraphEdge {
        from_record_kind,
        from_record_id,
        to_record_kind,
        to_record_id,
        relation,
    }
}

fn edge_key(edge: &EvidenceGraphEdge, other: &EvidenceGraphEdge) -> std::cmp::Ordering {
    edge.from_record_kind
        .cmp(&other.from_record_kind)
        .then_with(|| edge.from_record_id.cmp(&other.from_record_id))
        .then_with(|| edge.to_record_kind.cmp(&other.to_record_kind))
        .then_with(|| edge.to_record_id.cmp(&other.to_record_id))
        .then_with(|| edge.relation.cmp(&other.relation))
}

fn graph_shape(keys: &BTreeSet<NodeKey>, edges: &[EvidenceGraphEdge]) -> (usize, usize) {
    let mut adjacency: BTreeMap<NodeKey, Vec<NodeKey>> =
        keys.iter().cloned().map(|key| (key, Vec::new())).collect();
    for edge in edges {
        let from = NodeKey {
            record_kind: edge.from_record_kind,
            record_id: edge.from_record_id.clone(),
        };
        let to = NodeKey {
            record_kind: edge.to_record_kind,
            record_id: edge.to_record_id.clone(),
        };
        adjacency.entry(from.clone()).or_default().push(to.clone());
        adjacency.entry(to).or_default().push(from);
    }
    let isolated = adjacency
        .values()
        .filter(|neighbors| neighbors.is_empty())
        .count();
    let mut visited = BTreeSet::new();
    let mut components = 0;
    for key in adjacency.keys() {
        if visited.contains(key) {
            continue;
        }
        components += 1;
        let mut queue = VecDeque::from([key.clone()]);
        while let Some(next) = queue.pop_front() {
            if !visited.insert(next.clone()) {
                continue;
            }
            if let Some(neighbors) = adjacency.get(&next) {
                queue.extend(neighbors.iter().cloned());
            }
        }
    }
    (components, isolated)
}

fn digest_graph(
    bundle_digest: &str,
    query: &EvidenceGraphQuery,
    nodes: &[EvidenceGraphNode],
    edges: &[EvidenceGraphEdge],
    total_node_count: usize,
    total_edge_count: usize,
) -> Result<String, NeurosurgeryError> {
    let payload = (
        bundle_digest,
        query,
        nodes,
        edges,
        total_node_count,
        total_edge_count,
    );
    let bytes = serde_json::to_vec(&payload)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn graph_rejected(reason: &str) -> NeurosurgeryError {
    NeurosurgeryError::RealDataRejected {
        reason: reason.to_string(),
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}
