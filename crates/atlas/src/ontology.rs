//! The capability ontology.
//!
//! Implements blueprint 03.09. The ontology is a versioned hierarchical vocabulary for what the
//! platform evaluates, and its stated purpose includes the caveat that matters most: it must
//! provide that vocabulary "without pretending capabilities are perfectly separable". So the
//! hierarchy carries `is_a` and the non-hierarchical relations — notably `confounds_with` — are
//! first-class, because an aggregation that treats confounded capabilities as independent
//! evidence is the failure this module exists to make detectable.
//!
//! `is_a` lives in [`CapabilityNode::parents`] and nowhere else. A capability may have several
//! parents (the hierarchy is a DAG, not a tree) because a capability such as "verify a tool
//! result against a deterministic oracle" genuinely belongs to both verification and tool use.
//! What it may not do is reach itself: [`CapabilityOntology::validate`] refuses an ontology in
//! which any capability is its own ancestor, since every ancestor and descendant query over such
//! a graph is meaningless.
//!
//! NOT implemented here: ontology reprojection. 03.09 allows historical results to be reprojected
//! onto a new version "with an explicit transformation"; no transformation language is specified,
//! so a version mismatch is refused (see [`crate::AtlasError::OntologyVersionMismatch`]) rather
//! than guessed at. Also not implemented: the evidence-driven revision procedure (factor
//! analysis, item-response residuals), which is an offline study, not a runtime contract.

use crate::error::AtlasError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

/// Identifies a capability node. Distinct from every identifier in `bioprism-ids` on purpose:
/// 03.09's invariant list forbids conflating a benchmark family, a parent task, an instance, a
/// trial and a scored result, and a capability is none of those.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CapabilityId(String);

impl CapabilityId {
    pub fn parse(value: impl Into<String>) -> Result<Self, AtlasError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(AtlasError::EmptyCapabilityId);
        }
        if value.chars().any(char::is_control) {
            return Err(AtlasError::ControlCharacterInCapabilityId(value));
        }
        Ok(CapabilityId(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<CapabilityId> for String {
    fn from(value: CapabilityId) -> Self {
        value.0
    }
}

impl TryFrom<String> for CapabilityId {
    type Error = AtlasError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        CapabilityId::parse(value)
    }
}

/// The twelve top-level families named in 03.09. The set is closed: a capability that fits none
/// of them is a signal that the ontology version needs to change, which is a reviewed act, not a
/// runtime one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityFamily {
    EvidenceAcquisition,
    ContextManagement,
    HypothesisAndPlanning,
    ToolUse,
    StateTracking,
    Memory,
    Verification,
    Recovery,
    Coordination,
    Communication,
    PrivacyAndSafety,
    DomainReasoning,
}

impl CapabilityFamily {
    pub const ALL: [CapabilityFamily; 12] = [
        CapabilityFamily::EvidenceAcquisition,
        CapabilityFamily::ContextManagement,
        CapabilityFamily::HypothesisAndPlanning,
        CapabilityFamily::ToolUse,
        CapabilityFamily::StateTracking,
        CapabilityFamily::Memory,
        CapabilityFamily::Verification,
        CapabilityFamily::Recovery,
        CapabilityFamily::Coordination,
        CapabilityFamily::Communication,
        CapabilityFamily::PrivacyAndSafety,
        CapabilityFamily::DomainReasoning,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            CapabilityFamily::EvidenceAcquisition => "evidence_acquisition",
            CapabilityFamily::ContextManagement => "context_management",
            CapabilityFamily::HypothesisAndPlanning => "hypothesis_and_planning",
            CapabilityFamily::ToolUse => "tool_use",
            CapabilityFamily::StateTracking => "state_tracking",
            CapabilityFamily::Memory => "memory",
            CapabilityFamily::Verification => "verification",
            CapabilityFamily::Recovery => "recovery",
            CapabilityFamily::Coordination => "coordination",
            CapabilityFamily::Communication => "communication",
            CapabilityFamily::PrivacyAndSafety => "privacy_and_safety",
            CapabilityFamily::DomainReasoning => "domain_reasoning",
        }
    }
}

impl fmt::Display for CapabilityFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 03.09: "Distinguish competence, reliability, efficiency, safety, and calibration."
///
/// The distinction is not decorative. A capability measured on the safety dimension is
/// noncompensatory under 43.40 — a strong competence number never buys back a failed safety
/// gate — and the atlas needs the dimension to enforce that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityDimension {
    Competence,
    Reliability,
    Efficiency,
    Safety,
    Calibration,
}

impl CapabilityDimension {
    pub fn as_str(self) -> &'static str {
        match self {
            CapabilityDimension::Competence => "competence",
            CapabilityDimension::Reliability => "reliability",
            CapabilityDimension::Efficiency => "efficiency",
            CapabilityDimension::Safety => "safety",
            CapabilityDimension::Calibration => "calibration",
        }
    }
}

/// The relation vocabulary of 03.09. `IsA` is listed for completeness of the vocabulary but is
/// rejected as a loose relation by [`CapabilityOntology::validate`]: the hierarchy has exactly one
/// representation, [`CapabilityNode::parents`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    IsA,
    Requires,
    ConfoundsWith,
    TransfersTo,
    MeasuredBy,
    ContrastsWith,
    SafetyConstraintOn,
}

impl RelationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RelationKind::IsA => "is_a",
            RelationKind::Requires => "requires",
            RelationKind::ConfoundsWith => "confounds_with",
            RelationKind::TransfersTo => "transfers_to",
            RelationKind::MeasuredBy => "measured_by",
            RelationKind::ContrastsWith => "contrasts_with",
            RelationKind::SafetyConstraintOn => "safety_constraint_on",
        }
    }

    /// Whether the relation reads the same in both directions. Confounding and contrast are
    /// symmetric; requiring, transferring and constraining are not.
    pub fn is_symmetric(self) -> bool {
        matches!(
            self,
            RelationKind::ConfoundsWith | RelationKind::ContrastsWith
        )
    }
}

impl fmt::Display for RelationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Relation {
    pub kind: RelationKind,
    pub target: CapabilityId,
}

impl Relation {
    pub fn new(kind: RelationKind, target: CapabilityId) -> Self {
        Relation { kind, target }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityNode {
    pub id: CapabilityId,
    pub title: String,
    pub family: CapabilityFamily,
    pub dimension: CapabilityDimension,
    /// `is_a` edges. Several parents are permitted; reaching yourself is not.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parents: Vec<CapabilityId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<Relation>,
}

impl CapabilityNode {
    pub fn new(
        id: CapabilityId,
        title: impl Into<String>,
        family: CapabilityFamily,
        dimension: CapabilityDimension,
    ) -> Self {
        CapabilityNode {
            id,
            title: title.into(),
            family,
            dimension,
            parents: Vec::new(),
            relations: Vec::new(),
        }
    }

    pub fn with_parent(mut self, parent: CapabilityId) -> Self {
        self.parents.push(parent);
        self
    }

    pub fn with_relation(mut self, kind: RelationKind, target: CapabilityId) -> Self {
        self.relations.push(Relation::new(kind, target));
        self
    }

    pub fn targets_of(&self, kind: RelationKind) -> impl Iterator<Item = &CapabilityId> {
        self.relations
            .iter()
            .filter(move |r| r.kind == kind)
            .map(|r| &r.target)
    }
}

/// A versioned capability hierarchy.
///
/// Graph queries walk the node map directly rather than maintaining a child index. Ontologies at
/// this scale are hundreds of nodes, and an index that can disagree with the nodes is a worse
/// problem than a linear scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityOntology {
    version: String,
    nodes: BTreeMap<CapabilityId, CapabilityNode>,
}

impl CapabilityOntology {
    pub fn new(version: impl Into<String>) -> Self {
        CapabilityOntology {
            version: version.into(),
            nodes: BTreeMap::new(),
        }
    }

    /// Builds and validates in one step. Deserialization deliberately cannot do this — a
    /// `CapabilityOntology` read from JSON is unvalidated until [`CapabilityOntology::validate`]
    /// runs, which is why [`crate::AtlasBuilder::build`] always calls it.
    pub fn from_nodes(
        version: impl Into<String>,
        nodes: impl IntoIterator<Item = CapabilityNode>,
    ) -> Result<Self, AtlasError> {
        let mut ontology = CapabilityOntology::new(version);
        for node in nodes {
            ontology.insert(node)?;
        }
        ontology.validate()?;
        Ok(ontology)
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn insert(&mut self, node: CapabilityNode) -> Result<(), AtlasError> {
        if self.nodes.contains_key(&node.id) {
            return Err(AtlasError::DuplicateCapability {
                capability: node.id.to_string(),
            });
        }
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn contains(&self, id: &CapabilityId) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &CapabilityId> {
        self.nodes.keys()
    }

    pub fn nodes(&self) -> impl Iterator<Item = &CapabilityNode> {
        self.nodes.values()
    }

    pub fn node(&self, id: &CapabilityId) -> Result<&CapabilityNode, AtlasError> {
        self.nodes
            .get(id)
            .ok_or_else(|| AtlasError::UnknownCapability {
                capability: id.to_string(),
                ontology_version: self.version.clone(),
            })
    }

    /// Refuses every structural defect that would make a later query silently wrong.
    ///
    /// Order matters: dangling parents are reported before cycles, because a dangling parent
    /// makes the cycle search operate on a graph that is not the declared one.
    pub fn validate(&self) -> Result<(), AtlasError> {
        for node in self.nodes.values() {
            for parent in &node.parents {
                if parent == &node.id {
                    return Err(AtlasError::CyclicIsA {
                        capability: node.id.to_string(),
                        cycle: vec![node.id.to_string(), node.id.to_string()],
                    });
                }
                if !self.nodes.contains_key(parent) {
                    return Err(AtlasError::UnknownParent {
                        capability: node.id.to_string(),
                        parent: parent.to_string(),
                    });
                }
            }
            for relation in &node.relations {
                if relation.kind == RelationKind::IsA {
                    return Err(AtlasError::IsAOutsideHierarchy {
                        capability: node.id.to_string(),
                    });
                }
                if relation.target == node.id {
                    return Err(AtlasError::SelfRelation {
                        capability: node.id.to_string(),
                        relation: relation.kind.as_str(),
                    });
                }
                if !self.nodes.contains_key(&relation.target) {
                    return Err(AtlasError::UnknownRelationTarget {
                        capability: node.id.to_string(),
                        relation: relation.kind.as_str(),
                        target: relation.target.to_string(),
                    });
                }
            }
        }
        if let Some(cycle) = self.find_is_a_cycle() {
            let capability = cycle.first().cloned().unwrap_or_default();
            return Err(AtlasError::CyclicIsA { capability, cycle });
        }
        Ok(())
    }

    /// Every capability reachable by following `is_a` upward.
    ///
    /// The visited set means this terminates even on a cyclic ontology, and on a cyclic one the
    /// result contains the query itself — which is exactly the modelling error
    /// [`CapabilityOntology::is_own_ancestor`] reports.
    pub fn ancestors(&self, id: &CapabilityId) -> Result<BTreeSet<CapabilityId>, AtlasError> {
        let node = self.node(id)?;
        let mut seen = BTreeSet::new();
        let mut queue: VecDeque<CapabilityId> = node.parents.iter().cloned().collect();
        while let Some(next) = queue.pop_front() {
            if !seen.insert(next.clone()) {
                continue;
            }
            if let Some(parent) = self.nodes.get(&next) {
                queue.extend(parent.parents.iter().cloned());
            }
        }
        Ok(seen)
    }

    pub fn children(&self, id: &CapabilityId) -> Result<Vec<CapabilityId>, AtlasError> {
        self.node(id)?;
        Ok(self
            .nodes
            .values()
            .filter(|n| n.parents.contains(id))
            .map(|n| n.id.clone())
            .collect())
    }

    pub fn descendants(&self, id: &CapabilityId) -> Result<BTreeSet<CapabilityId>, AtlasError> {
        self.node(id)?;
        let mut seen = BTreeSet::new();
        let mut queue: VecDeque<CapabilityId> = self.children(id)?.into_iter().collect();
        while let Some(next) = queue.pop_front() {
            if !seen.insert(next.clone()) {
                continue;
            }
            queue.extend(self.children(&next)?);
        }
        Ok(seen)
    }

    /// The capability together with everything below it. This is the unit a claim about a
    /// non-leaf capability is really about.
    pub fn subtree(&self, id: &CapabilityId) -> Result<BTreeSet<CapabilityId>, AtlasError> {
        let mut set = self.descendants(id)?;
        set.insert(id.clone());
        Ok(set)
    }

    /// The modelling error named in 03.09's remit: a capability that reaches itself by `is_a`.
    pub fn is_own_ancestor(&self, id: &CapabilityId) -> Result<bool, AtlasError> {
        Ok(self.ancestors(id)?.contains(id))
    }

    pub fn roots(&self) -> impl Iterator<Item = &CapabilityNode> {
        self.nodes.values().filter(|n| n.parents.is_empty())
    }

    /// Capabilities the ontology marks as not separable from `id`, in both directions.
    ///
    /// 03.09: "Every cell may measure primary and secondary capabilities ... but uncertainty
    /// reflects overlap." The atlas cannot compute that overlap, but it can refuse to pretend it
    /// is absent.
    pub fn confounded_with(&self, id: &CapabilityId) -> Result<BTreeSet<CapabilityId>, AtlasError> {
        let node = self.node(id)?;
        let mut set: BTreeSet<CapabilityId> = node
            .targets_of(RelationKind::ConfoundsWith)
            .cloned()
            .collect();
        for other in self.nodes.values() {
            if other
                .targets_of(RelationKind::ConfoundsWith)
                .any(|t| t == id)
            {
                set.insert(other.id.clone());
            }
        }
        set.remove(id);
        Ok(set)
    }

    /// Capabilities that declare a `safety_constraint_on` edge into `id` or into any of its
    /// ancestors. A constraint on a parent binds every capability underneath it.
    pub fn safety_constraints_on(
        &self,
        id: &CapabilityId,
    ) -> Result<BTreeSet<CapabilityId>, AtlasError> {
        let mut guarded = self.ancestors(id)?;
        guarded.insert(id.clone());
        let mut set = BTreeSet::new();
        for node in self.nodes.values() {
            if node
                .targets_of(RelationKind::SafetyConstraintOn)
                .any(|t| guarded.contains(t))
            {
                set.insert(node.id.clone());
            }
        }
        set.remove(id);
        Ok(set)
    }

    fn find_is_a_cycle(&self) -> Option<Vec<String>> {
        const WHITE: u8 = 0;
        const GREY: u8 = 1;
        const BLACK: u8 = 2;

        let ids: Vec<&CapabilityId> = self.nodes.keys().collect();
        let index: BTreeMap<&CapabilityId, usize> =
            ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let adjacency: Vec<Vec<usize>> = ids
            .iter()
            .map(|id| {
                self.nodes[*id]
                    .parents
                    .iter()
                    .filter_map(|p| index.get(p).copied())
                    .collect()
            })
            .collect();

        let mut colour = vec![WHITE; ids.len()];
        for start in 0..ids.len() {
            if colour[start] != WHITE {
                continue;
            }
            colour[start] = GREY;
            let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
            while let Some(&(node, next)) = stack.last() {
                if next >= adjacency[node].len() {
                    colour[node] = BLACK;
                    stack.pop();
                    continue;
                }
                if let Some(top) = stack.last_mut() {
                    top.1 += 1;
                }
                let parent = adjacency[node][next];
                match colour[parent] {
                    WHITE => {
                        colour[parent] = GREY;
                        stack.push((parent, 0));
                    }
                    GREY => {
                        let at = stack.iter().position(|&(n, _)| n == parent).unwrap_or(0);
                        let mut cycle: Vec<String> = stack[at..]
                            .iter()
                            .map(|&(n, _)| ids[n].to_string())
                            .collect();
                        cycle.push(ids[parent].to_string());
                        return Some(cycle);
                    }
                    _ => {}
                }
            }
        }
        None
    }
}
