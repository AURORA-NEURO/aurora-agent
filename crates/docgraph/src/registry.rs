//! The module registry and the graph it induces (41.01, 41.02, 41.04).
//!
//! 41.02 asks for "one machine-readable row for every Markdown contract, root guide, tool, and
//! generated registry", with unique ids, existing paths, and hashes that match bytes. 41.01 adds
//! the edges and requires that "every node resolves to a file and H1". This module is those two
//! together: [`ModuleNode`] is the row, [`DocGraph`] is the rows plus the typed edges.
//!
//! # A graph may be broken and still exist
//!
//! [`DocGraph::insert_edge`] does not check that its endpoints are registered. That is deliberate
//! and is the difference between this crate and a validating parser: 41.11 asks for a lint
//! *report* over a corpus, and a graph type that refuses to hold a dangling edge can only ever
//! report zero of them. Construction accepts; [`crate::lint`] judges; [`crate::bundle`] refuses
//! to compile when the damage would reach the delivered context.
//!
//! # Hashes are optional and are never invented
//!
//! [`ModuleNode::hash`] is `Option<ContentHash>`. When a node is built from bytes the hash is
//! computed from those bytes ([`ModuleNode::with_hashed_body`]); when a node is declared by hand
//! it has no hash and says so. There is no "hash of the fields we happen to have", because 41.02
//! requires that "hashes match bytes" and a hash over a reconstruction matches nothing.
//!
//! # Not implemented
//!
//! No word counts (a second cost number that no consumer here reads), no `status: draft|final`
//! workflow state beyond [`NodeStatus`], and no ownership metadata. Each is a registry column the
//! blueprint lists that nothing in this crate would branch on, and an unread column is a column
//! that goes stale silently.

use crate::error::DocGraphError;
use crate::tokens::{ProfileLevel, TokenCost};
use crate::vocabulary::{DocEdge, DocEdgeType};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A stable module identifier: `41.03`, `repo/docs/ARCHITECTURE.md`, `crate/bioprism-fiber`.
///
/// Validated rather than aliased to `String` so that "the id of a module" and "some text" are not
/// the same type at a call site that takes several strings in a row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ModuleId(String);

impl ModuleId {
    pub fn parse(value: impl Into<String>) -> Result<Self, DocGraphError> {
        let value = value.into();
        if value.is_empty() {
            return Err(DocGraphError::MalformedModuleId(value, "empty"));
        }
        if value.chars().any(char::is_whitespace) {
            return Err(DocGraphError::MalformedModuleId(
                value,
                "contains whitespace",
            ));
        }
        if value.chars().any(char::is_control) {
            return Err(DocGraphError::MalformedModuleId(
                value,
                "contains a control character",
            ));
        }
        Ok(ModuleId(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<ModuleId> for String {
    fn from(value: ModuleId) -> Self {
        value.0
    }
}

impl TryFrom<String> for ModuleId {
    type Error = DocGraphError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ModuleId::parse(value)
    }
}

/// What kind of thing a module is, in the one sense a reader must not get wrong.
///
/// 41.12's first invariant is that "agents never imply implementation from specification". That
/// is only checkable if the registry says which a node is, so this enum exists to be read by
/// [`crate::protocol`] and by nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    /// A normative contract. States what must be true; says nothing about what is built.
    Specification,
    /// Documentation of what exists in the repository.
    Implementation,
    /// Orientation for a human or an agent. Not normative.
    Guide,
    /// A tool's own documentation.
    Tool,
    /// Emitted by a program from other nodes. Editing it by hand is a defect.
    Generated,
    /// Superseded, retained for history. Must never be delivered without its successor.
    Withdrawn,
}

impl NodeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            NodeStatus::Specification => "specification",
            NodeStatus::Implementation => "implementation",
            NodeStatus::Guide => "guide",
            NodeStatus::Tool => "tool",
            NodeStatus::Generated => "generated",
            NodeStatus::Withdrawn => "withdrawn",
        }
    }

    /// Whether an orphan node of this status is worth reporting (41.11: "orphan normative modules
    /// are reviewed"). A guide with no incoming edges is a front door; a specification with no
    /// incoming edges is a contract nobody builds against.
    pub fn orphan_is_a_defect(self) -> bool {
        matches!(
            self,
            NodeStatus::Specification | NodeStatus::Implementation | NodeStatus::Generated
        )
    }
}

/// The ten protected classes of blueprint 39.05.
///
/// 39.05: "A context compiler may replace verbose representations of these classes with stable
/// codes and graph references, but it may not omit the semantics." A node tagged with any of
/// these is non-omittable from any bundle that reaches it — not because a route said so, but
/// because the corpus said so. That distinction matters: a route author can forget an invariant,
/// and the compiler must not depend on them remembering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectedClass {
    /// 1. subject, lesion, region, specimen, aliquot, assay, artifact, model-system identity
    Identity,
    /// 2. valid, collection, treatment, record, release, decision time
    Temporal,
    /// 3. units, coordinate system, genome build, orientation, scale
    UnitsAndReference,
    /// 4. platform, protocol, reagent, preprocessing, model, ontology, classifier version
    ProvenanceVersion,
    /// 5. cohort inclusion/exclusion, duplicate structure, split unit, censoring rule
    CohortStructure,
    /// 6. uncertainty, quality, detection limit, missingness class, failure state
    UncertaintyAndQuality,
    /// 7. contradictory and negative evidence
    ContradictoryEvidence,
    /// 8. privacy, consent, data residency, role visibility
    AccessAndConsent,
    /// 9. observation vs interpretation vs hypothesis vs causal claim vs recommendation
    ClaimKind,
    /// 10. counterfactual support boundary
    CounterfactualBoundary,
}

impl ProtectedClass {
    pub const ALL: [ProtectedClass; 10] = [
        ProtectedClass::Identity,
        ProtectedClass::Temporal,
        ProtectedClass::UnitsAndReference,
        ProtectedClass::ProvenanceVersion,
        ProtectedClass::CohortStructure,
        ProtectedClass::UncertaintyAndQuality,
        ProtectedClass::ContradictoryEvidence,
        ProtectedClass::AccessAndConsent,
        ProtectedClass::ClaimKind,
        ProtectedClass::CounterfactualBoundary,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ProtectedClass::Identity => "identity",
            ProtectedClass::Temporal => "temporal",
            ProtectedClass::UnitsAndReference => "units_and_reference",
            ProtectedClass::ProvenanceVersion => "provenance_version",
            ProtectedClass::CohortStructure => "cohort_structure",
            ProtectedClass::UncertaintyAndQuality => "uncertainty_and_quality",
            ProtectedClass::ContradictoryEvidence => "contradictory_evidence",
            ProtectedClass::AccessAndConsent => "access_and_consent",
            ProtectedClass::ClaimKind => "claim_kind",
            ProtectedClass::CounterfactualBoundary => "counterfactual_boundary",
        }
    }
}

/// The 41.04 compact card: identity, one-sentence decision, protected invariants, inputs,
/// outputs, neighbours, path.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCard {
    /// One sentence. 41.04 sizes the whole card at 60–180 tokens, which is a sentence's worth of
    /// decision plus its invariants.
    pub decision: String,
    /// Restated, never compressed away. 41.04: "mandatory invariants are not compressed away".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protected_invariants: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub neighbors: Vec<ModuleId>,
}

impl ContextCard {
    /// The card as an agent would read it. Deterministic; this text is what gets costed.
    pub fn render(&self, node: &ModuleNode) -> String {
        let mut out = String::new();
        out.push_str(&format!("[{}] {} — {}\n", node.id, node.title, node.path));
        out.push_str(&format!("decision: {}\n", self.decision));
        if !self.protected_invariants.is_empty() {
            out.push_str("invariants:\n");
            for invariant in &self.protected_invariants {
                out.push_str(&format!("  - {invariant}\n"));
            }
        }
        if !self.inputs.is_empty() {
            out.push_str(&format!("inputs: {}\n", self.inputs.join(", ")));
        }
        if !self.outputs.is_empty() {
            out.push_str(&format!("outputs: {}\n", self.outputs.join(", ")));
        }
        if !self.neighbors.is_empty() {
            let ids: Vec<&str> = self.neighbors.iter().map(ModuleId::as_str).collect();
            out.push_str(&format!("neighbors: {}\n", ids.join(", ")));
        }
        out
    }
}

/// The texts a node can be loaded at. Absent levels are `None`, never an empty string standing in
/// for one: "this module has no deep reference" and "this module's deep reference is empty" are
/// different registry states.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep: Option<String>,
}

/// One registry row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleNode {
    pub id: ModuleId,
    pub path: String,
    /// The H1. 41.01: "every node resolves to a file and H1".
    pub title: String,
    pub cluster: String,
    pub status: NodeStatus,
    /// The level this node's *own* front matter declares. A route may request less.
    pub declared_profile: ProfileLevel,
    pub card: ContextCard,
    #[serde(default)]
    pub body: ModuleBody,
    /// Policy labels a traversal may be denied on (41.07).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_labels: Vec<String>,
    /// 39.05 classes this module carries the semantics of. Non-empty means non-omittable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protected_classes: Vec<ProtectedClass>,
    /// Content hash of the source bytes, when the node was built from bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<ContentHash>,
}

impl ModuleNode {
    pub fn new(
        id: ModuleId,
        path: impl Into<String>,
        title: impl Into<String>,
        status: NodeStatus,
    ) -> Self {
        ModuleNode {
            id,
            path: path.into(),
            title: title.into(),
            cluster: String::new(),
            status,
            declared_profile: ProfileLevel::Brief,
            card: ContextCard::default(),
            body: ModuleBody::default(),
            policy_labels: Vec::new(),
            protected_classes: Vec::new(),
            hash: None,
        }
    }

    pub fn with_cluster(mut self, cluster: impl Into<String>) -> Self {
        self.cluster = cluster.into();
        self
    }

    pub fn with_profile(mut self, profile: ProfileLevel) -> Self {
        self.declared_profile = profile;
        self
    }

    pub fn with_card(mut self, card: ContextCard) -> Self {
        self.card = card;
        self
    }

    pub fn with_policy_label(mut self, label: impl Into<String>) -> Self {
        self.policy_labels.push(label.into());
        self
    }

    pub fn with_protected(mut self, class: ProtectedClass) -> Self {
        if !self.protected_classes.contains(&class) {
            self.protected_classes.push(class);
            self.protected_classes.sort();
        }
        self
    }

    /// Attach the contract text and hash the exact bytes it was read from.
    ///
    /// The hash covers the source, not the parsed node, so it can be compared against a file on
    /// disk without re-running this crate's parser.
    pub fn with_hashed_body(mut self, source: &str) -> Self {
        self.body.contract = Some(source.to_string());
        self.hash = Some(ContentHash::of_bytes(source.as_bytes()));
        self
    }

    pub fn with_brief(mut self, brief: impl Into<String>) -> Self {
        self.body.brief = Some(brief.into());
        self
    }

    /// The text this node renders to at a level, or `None` when the level has no text.
    ///
    /// [`ProfileLevel::Handle`] is always available because identity is always known;
    /// [`ProfileLevel::Card`] is always available because the card is rendered from registry
    /// fields. The rest depend on what was loaded.
    pub fn text_at(&self, level: ProfileLevel) -> Option<String> {
        match level {
            ProfileLevel::Handle => Some(format!("[{}] {} — {}", self.id, self.title, self.path)),
            ProfileLevel::Card => Some(self.card.render(self)),
            ProfileLevel::Brief => self.body.brief.clone(),
            ProfileLevel::Contract => self.body.contract.clone(),
            ProfileLevel::DeepReference => self
                .body
                .deep
                .clone()
                .or_else(|| self.body.contract.clone()),
        }
    }

    /// Cost at a level. A level with no text costs nothing, which is honest: the bundle will not
    /// contain it either.
    pub fn cost_at(&self, level: ProfileLevel) -> TokenCost {
        match self.text_at(level) {
            Some(text) => TokenCost::estimate(&text),
            None => TokenCost::zero_estimate(),
        }
    }

    /// The highest level this node can actually be delivered at.
    pub fn best_available_level(&self) -> ProfileLevel {
        if self.body.deep.is_some() {
            ProfileLevel::DeepReference
        } else if self.body.contract.is_some() {
            ProfileLevel::Contract
        } else if self.body.brief.is_some() {
            ProfileLevel::Brief
        } else {
            ProfileLevel::Card
        }
    }

    pub fn is_non_omittable(&self) -> bool {
        !self.protected_classes.is_empty()
    }
}

/// Registry plus typed edges.
///
/// Nodes are keyed by id in a `BTreeMap` and edges are kept sorted, so every listing this crate
/// produces has one order regardless of insertion sequence. Determinism is not a nicety here:
/// 41.16 requires that a clean extraction "reproduce the same selected module set from the same
/// registry version", and a bundle whose contents depend on `HashMap` iteration cannot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocGraph {
    nodes: BTreeMap<ModuleId, ModuleNode>,
    edges: Vec<DocEdge>,
}

impl DocGraph {
    pub fn new() -> Self {
        DocGraph::default()
    }

    /// 41.02: "module IDs are unique". Re-registering is an error, not an overwrite, because an
    /// overwrite loses the row that was there.
    pub fn insert_node(&mut self, node: ModuleNode) -> Result<(), DocGraphError> {
        if self.nodes.contains_key(&node.id) {
            return Err(DocGraphError::DuplicateModuleId(node.id));
        }
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    /// Accepts dangling and self edges. See the module docs: the linter must be able to find them.
    pub fn insert_edge(&mut self, edge: DocEdge) {
        let key = edge.order_key();
        let position = self
            .edges
            .binary_search_by(|existing| existing.order_key().cmp(&key))
            .unwrap_or_else(|slot| slot);
        self.edges.insert(position, edge);
    }

    pub fn node(&self, id: &ModuleId) -> Option<&ModuleNode> {
        self.nodes.get(id)
    }

    pub fn contains(&self, id: &ModuleId) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &ModuleNode> {
        self.nodes.values()
    }

    pub fn node_ids(&self) -> impl Iterator<Item = &ModuleId> {
        self.nodes.keys()
    }

    pub fn edges(&self) -> &[DocEdge] {
        &self.edges
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn in_edges<'a>(&'a self, id: &'a ModuleId) -> impl Iterator<Item = &'a DocEdge> + 'a {
        self.edges.iter().filter(move |edge| &edge.to == id)
    }

    /// Edges of one type, in stored order.
    pub fn edges_of<'a>(&'a self, kind: DocEdgeType) -> impl Iterator<Item = &'a DocEdge> + 'a {
        self.edges.iter().filter(move |edge| edge.kind == kind)
    }

    /// Successors of a superseded module: sources of incoming `supersedes` edges.
    pub fn successors_of<'a>(&'a self, id: &'a ModuleId) -> impl Iterator<Item = &'a ModuleId> + 'a {
        self.in_edges(id)
            .filter(|edge| edge.kind == DocEdgeType::Supersedes)
            .map(|edge| &edge.from)
    }

    /// Every module id an edge names but the registry does not hold.
    pub fn dangling_endpoints(&self) -> BTreeSet<ModuleId> {
        let mut missing = BTreeSet::new();
        for edge in &self.edges {
            if !self.nodes.contains_key(&edge.from) {
                missing.insert(edge.from.clone());
            }
            if !self.nodes.contains_key(&edge.to) {
                missing.insert(edge.to.clone());
            }
        }
        missing
    }
}
