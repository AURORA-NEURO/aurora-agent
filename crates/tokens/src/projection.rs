//! Multi-agent context projection (39.11).
//!
//! 39.11 in full is three sentences: each specialist receives the smallest subgraph needed for its
//! role; projection policies specify visible node classes, permitted expansions, mandatory
//! contradiction edges, authority and output contracts; and *"a synthesis agent does not receive
//! hidden specialist data unless policy allows it; it may receive an attested claim with
//! uncertainty and proof handle instead"*.
//!
//! # What this module is, next to `bioprism-weave`
//!
//! `bioprism-weave` already carries a context capsule between agents — that is the transport. The
//! question here is the *token* one: what a projection costs, what it drops, and whether every drop
//! was recorded. A projection is not a message; it is a lossy view of a compiled context, and the
//! loss has to be as legible as the content.
//!
//! # A projection carries its own omission record
//!
//! This is the load-bearing design decision. The source context has an omission manifest describing
//! what the *compiler* left out. The projection has a second one describing what the *projection*
//! left out, and they are not merged: a peer that receives a projection must be able to tell
//! "nobody selected this" from "it was selected and your role does not see it", because the
//! remedies are different — one is a retrieval, the other is an authority request.
//! [`PeerProjection::accounts_for_every_drop`] is the check that keeps the second record honest, and
//! [`ProjectionError::DropNotAccountedFor`] fires when it does not.
//!
//! # A projection may not know more than its source
//!
//! [`ProjectionError::SufficiencyStrengthenedByProjection`]. Removing evidence cannot raise a
//! sufficiency claim, so a projection inherits its source's status and may only weaken it. Without
//! this, the smallest projection would be the one that looked most complete, which is the section's
//! central failure mode arriving through the side door.
//!
//! # Cost is estimated and the saving says so
//!
//! [`ProjectionCost`] holds three [`TokenEstimate`]s and refuses to produce a saving unless the
//! source and the projection totals are comparable per
//! [`crate::context::estimates_are_comparable`] — a difference between two rulers is not a saving,
//! and neither is a difference between two totals that were each already mixed across rulers.
//! [`ProjectionCost::saved_fraction`] returns `None` in those cases rather than a plausible number.
//!
//! # Not implemented
//!
//! No transport, no authority verification, no attestation. [`ProjectionPolicy::authority`] is a
//! recorded string, not a checked credential: 39.19 owns role authority and this module trusts the
//! policy it is handed. An [`crate::context::NodeKind::AttestedClaim`] node is likewise a value a
//! peer constructed, and nothing here verifies its proof handle.

use crate::context::{estimates_are_comparable, CompiledContext, ContextNode, NodeKind, Visibility};
use crate::error::ProjectionError;
use bioprism_ids::ContentHash;
use bioprism_obligation::{SufficiencyStatus, TokenEstimate};
use bioprism_section::{InfluenceClass, OmissionGroup, OmissionManifest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// What a peer role is permitted to see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionPolicy {
    pub policy_id: String,
    pub role: String,
    /// Node classes this role receives. Everything else is dropped and recorded.
    pub visible_kinds: BTreeSet<NodeKind>,
    /// Expansion operations the peer may request against a dropped node. A projection that drops
    /// something and offers no way to ask for it has removed the evidence from the conversation.
    #[serde(default)]
    pub permitted_expansions: BTreeSet<String>,
    /// Whether contradiction nodes must survive the projection whatever the visible-kind set says.
    /// 39.11's "mandatory contradiction edges".
    #[serde(default)]
    pub contradictions_mandatory: bool,
    /// Peer roles whose private state this role may receive directly. Empty in the ordinary case:
    /// a synthesis agent receives attested claims instead.
    #[serde(default)]
    pub may_receive_private_from: BTreeSet<String>,
    /// Recorded, not verified. 39.19 owns authority.
    pub authority: String,
    /// The shape the peer is expected to return.
    pub output_contract: String,
}

impl ProjectionPolicy {
    pub fn new(
        policy_id: impl Into<String>,
        role: impl Into<String>,
        authority: impl Into<String>,
        output_contract: impl Into<String>,
    ) -> Self {
        ProjectionPolicy {
            policy_id: policy_id.into(),
            role: role.into(),
            visible_kinds: BTreeSet::new(),
            permitted_expansions: BTreeSet::new(),
            contradictions_mandatory: false,
            may_receive_private_from: BTreeSet::new(),
            authority: authority.into(),
            output_contract: output_contract.into(),
        }
    }

    pub fn showing<I: IntoIterator<Item = NodeKind>>(mut self, kinds: I) -> Self {
        self.visible_kinds.extend(kinds);
        self
    }

    pub fn allowing_expansion(mut self, expansion: impl Into<String>) -> Self {
        self.permitted_expansions.insert(expansion.into());
        self
    }

    pub fn with_mandatory_contradictions(mut self) -> Self {
        self.contradictions_mandatory = true;
        self
    }

    pub fn receiving_private_from(mut self, role: impl Into<String>) -> Self {
        self.may_receive_private_from.insert(role.into());
        self
    }

    fn admits_kind(&self, kind: NodeKind) -> bool {
        self.visible_kinds.contains(&kind)
            || (self.contradictions_mandatory && kind == NodeKind::Contradiction)
    }
}

/// Why a node did not survive a projection.
///
/// A separate vocabulary from the compiler's omission reasons on purpose. "Your role does not see
/// this class" and "nobody selected this" call for different responses from the peer, and a single
/// merged reason string would make them look like the same event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "projection_omission", rename_all = "snake_case")]
pub enum ProjectionOmission {
    /// The node's class is not in the policy's visible set.
    KindNotVisible { kind: NodeKind },
    /// The node belongs to another specialist and this policy grants no access to that peer.
    /// The peer may still receive an attested claim about it.
    PeerPrivate { owner_role: String },
    /// The node is evaluator holdout state. Never projectable, and reported so the peer knows a
    /// gap exists rather than believing it has everything.
    Holdout,
}

impl ProjectionOmission {
    pub fn reason_key(&self) -> String {
        match self {
            ProjectionOmission::KindNotVisible { kind } => {
                format!("kind_not_visible:{}", kind.as_str())
            }
            ProjectionOmission::PeerPrivate { owner_role } => {
                format!("peer_private:{owner_role}")
            }
            ProjectionOmission::Holdout => "holdout".to_string(),
        }
    }

    /// The influence class this omission implies before any argument is supplied.
    ///
    /// None of them is [`InfluenceClass::Zero`]. Dropping a node because the peer's role does not
    /// see its class says nothing about whether it mattered, and the honest default is that nobody
    /// checked. This mirrors `bioprism-obligation`'s treatment of budget-driven omissions.
    pub fn default_influence(&self) -> InfluenceClass {
        match self {
            ProjectionOmission::KindNotVisible { .. } => InfluenceClass::Unknown,
            ProjectionOmission::PeerPrivate { .. } | ProjectionOmission::Holdout => {
                InfluenceClass::InaccessibleByPolicy
            }
        }
    }
}

/// One dropped node with the reason it was dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DroppedNode {
    pub node_id: String,
    pub kind: NodeKind,
    pub omission: ProjectionOmission,
    /// The estimated tokens the peer did not receive.
    pub estimate: TokenEstimate,
    /// How the peer could ask for it, when the policy permits an expansion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expansion: Option<String>,
}

/// What a projection cost, in estimates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionCost {
    pub source: TokenEstimate,
    pub projected: TokenEstimate,
    pub dropped: TokenEstimate,
}

impl ProjectionCost {
    /// The fraction of the source's estimated tokens that did not survive.
    ///
    /// `None` whenever the two totals are not comparable per
    /// [`crate::context::estimates_are_comparable`] — different estimators, or a total already
    /// mixed across estimators. `None` also when the source is empty, since a fraction of nothing
    /// is not zero, it is undefined.
    pub fn saved_fraction(&self) -> Option<f64> {
        if !estimates_are_comparable(&self.source, &self.projected) || self.source.tokens == 0 {
            return None;
        }
        Some(self.dropped.tokens as f64 / self.source.tokens as f64)
    }

    /// True only when both totals came from a real tokenizer. Never true in this workspace.
    pub fn is_measured(&self) -> bool {
        self.source.method.is_measured() && self.projected.method.is_measured()
    }
}

/// What a peer actually receives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeerProjection {
    pub policy_id: String,
    pub role: String,
    /// The semantic digest of the context this was projected from. 39.11's "agents exchange stable
    /// references", reduced to the one reference that matters for reproducing the projection.
    pub source_digest: String,
    pub nodes: Vec<ContextNode>,
    pub dropped: Vec<DroppedNode>,
    /// The projection's own omission record, grouped by reason. Separate from the source context's.
    pub omissions: OmissionManifest,
    /// Never stronger than the source's.
    pub sufficiency: SufficiencyStatus,
    pub cost: ProjectionCost,
    pub permitted_expansions: BTreeSet<String>,
    pub output_contract: String,
}

impl PeerProjection {
    /// Every dropped node is described by exactly one omission group entry.
    ///
    /// The check that keeps the second omission record honest: a projection that dropped forty
    /// nodes and recorded thirty has thirty-nine explanations too few, and a peer reading the
    /// record would conclude it had a complete picture minus thirty things.
    pub fn accounts_for_every_drop(&self) -> bool {
        self.omissions.total_omitted() == self.dropped.len()
    }

    pub fn node_ids(&self) -> BTreeSet<String> {
        self.nodes.iter().map(|node| node.node_id.clone()).collect()
    }

    pub fn dropped_ids(&self) -> BTreeSet<String> {
        self.dropped
            .iter()
            .map(|node| node.node_id.clone())
            .collect()
    }

    /// Dropped nodes the peer has no way to ask for.
    ///
    /// Not an error — holdout state is legitimately unaskable — but worth surfacing, because a
    /// projection whose every drop is unrecoverable has not compressed the context, it has censored
    /// it.
    pub fn unrecoverable_drops(&self) -> Vec<&DroppedNode> {
        self.dropped
            .iter()
            .filter(|node| node.expansion.is_none())
            .collect()
    }

    /// Content hash of what the peer received, for the stable-reference exchange of 39.11.
    pub fn digest(&self) -> Result<String, ProjectionError> {
        let value = serde_json::to_value(self)
            .map_err(|error| ProjectionError::NotAddressable(self.role.clone(), error.to_string()))?;
        ContentHash::of_value(&value)
            .map(|hash| hash.as_str().to_string())
            .map_err(|error| ProjectionError::NotAddressable(self.role.clone(), error.to_string()))
    }
}

/// Project a compiled context to one peer role under one policy.
///
/// Refuses rather than silently redacting in the two cases where silence would be a security
/// failure — holdout state and unauthorised peer-private state reaching the visible set — and
/// records everything else in the projection's own omission manifest.
pub fn project(
    context: &CompiledContext,
    policy: &ProjectionPolicy,
) -> Result<PeerProjection, ProjectionError> {
    let source_digest = context
        .semantic_digest()
        .map_err(|error| ProjectionError::NotAddressable(policy.role.clone(), error.to_string()))?;

    let mut kept: Vec<ContextNode> = Vec::new();
    let mut dropped: Vec<DroppedNode> = Vec::new();

    for node in &context.nodes {
        let omission = match &node.visibility {
            Visibility::Holdout => {
                if policy.admits_kind(node.kind) && policy.contradictions_mandatory
                    && node.kind == NodeKind::Contradiction
                {
                    return Err(ProjectionError::HoldoutWouldBeProjected {
                        role: policy.role.clone(),
                        node: node.node_id.clone(),
                    });
                }
                Some(ProjectionOmission::Holdout)
            }
            Visibility::PeerPrivate { owner_role }
                if owner_role != &policy.role
                    && !policy.may_receive_private_from.contains(owner_role) =>
            {
                Some(ProjectionOmission::PeerPrivate {
                    owner_role: owner_role.clone(),
                })
            }
            _ if !policy.admits_kind(node.kind) => {
                if policy.contradictions_mandatory && node.kind == NodeKind::Contradiction {
                    return Err(ProjectionError::MandatoryContradictionDropped {
                        role: policy.role.clone(),
                        node: node.node_id.clone(),
                    });
                }
                Some(ProjectionOmission::KindNotVisible { kind: node.kind })
            }
            _ => None,
        };

        match omission {
            None => kept.push(node.clone()),
            Some(omission) => {
                let expansion = expansion_for(policy, node);
                dropped.push(DroppedNode {
                    node_id: node.node_id.clone(),
                    kind: node.kind,
                    omission,
                    estimate: node.estimate.clone(),
                    expansion,
                });
            }
        }
    }

    if policy.contradictions_mandatory {
        if let Some(node) = dropped
            .iter()
            .find(|node| node.kind == NodeKind::Contradiction)
        {
            return Err(ProjectionError::MandatoryContradictionDropped {
                role: policy.role.clone(),
                node: node.node_id.clone(),
            });
        }
    }

    if let Some(node) = kept.iter().find(|node| node.visibility.is_holdout()) {
        return Err(ProjectionError::HoldoutWouldBeProjected {
            role: policy.role.clone(),
            node: node.node_id.clone(),
        });
    }

    let omissions = group_omissions(&dropped);
    let projected = TokenEstimate::sum(kept.iter().map(|node| &node.estimate));
    let dropped_estimate = TokenEstimate::sum(dropped.iter().map(|node| &node.estimate));
    let cost = ProjectionCost {
        source: context.total_estimate(),
        projected,
        dropped: dropped_estimate,
    };

    let sufficiency = weaken(context.sufficiency, &dropped);
    let projection = PeerProjection {
        policy_id: policy.policy_id.clone(),
        role: policy.role.clone(),
        source_digest,
        nodes: kept,
        dropped,
        omissions,
        sufficiency,
        cost,
        permitted_expansions: policy.permitted_expansions.clone(),
        output_contract: policy.output_contract.clone(),
    };

    if !projection.accounts_for_every_drop() {
        return Err(ProjectionError::DropNotAccountedFor {
            role: policy.role.clone(),
            dropped: projection.dropped.len(),
            accounted: projection.omissions.total_omitted(),
        });
    }

    Ok(projection)
}

/// Check that a projection assembled elsewhere obeys the rules this module enforces.
///
/// The transport is somebody else's — `bioprism-weave` carries capsules — so a projection can
/// arrive without having gone through [`project`]. This is the receiving end's check.
pub fn validate_projection(
    projection: &PeerProjection,
    source: &CompiledContext,
) -> Result<(), ProjectionError> {
    if !projection.accounts_for_every_drop() {
        return Err(ProjectionError::DropNotAccountedFor {
            role: projection.role.clone(),
            dropped: projection.dropped.len(),
            accounted: projection.omissions.total_omitted(),
        });
    }
    if let Some(node) = projection.nodes.iter().find(|node| node.visibility.is_holdout()) {
        return Err(ProjectionError::HoldoutWouldBeProjected {
            role: projection.role.clone(),
            node: node.node_id.clone(),
        });
    }
    if rank(projection.sufficiency) > rank(source.sufficiency) {
        return Err(ProjectionError::SufficiencyStrengthenedByProjection {
            role: projection.role.clone(),
            source_status: source.sufficiency.as_str().to_string(),
            projected_status: projection.sufficiency.as_str().to_string(),
        });
    }
    Ok(())
}

fn rank(status: SufficiencyStatus) -> u8 {
    match status {
        SufficiencyStatus::Sufficient => 3,
        SufficiencyStatus::Insufficient => 2,
        SufficiencyStatus::Unknown => 1,
        SufficiencyStatus::Failed => 0,
    }
}

/// Removing evidence can only lower a sufficiency claim.
///
/// A projection that dropped anything with unknown influence cannot still be sufficient, because
/// sufficiency was established over a set that no longer holds. Everything else is inherited.
fn weaken(source: SufficiencyStatus, dropped: &[DroppedNode]) -> SufficiencyStatus {
    if dropped.is_empty() {
        return source;
    }
    match source {
        SufficiencyStatus::Sufficient => SufficiencyStatus::Unknown,
        other => other,
    }
}

fn expansion_for(policy: &ProjectionPolicy, node: &ContextNode) -> Option<String> {
    if node.visibility.is_holdout() {
        return None;
    }
    let by_kind = format!("expand:{}", node.kind.as_str());
    if policy.permitted_expansions.contains(&by_kind) {
        return Some(by_kind);
    }
    if policy.permitted_expansions.contains("expand:any") {
        return Some("expand:any".to_string());
    }
    None
}

fn group_omissions(dropped: &[DroppedNode]) -> OmissionManifest {
    let mut by_reason: BTreeMap<String, (InfluenceClass, usize, Vec<String>)> = BTreeMap::new();
    for node in dropped {
        let entry = by_reason
            .entry(node.omission.reason_key())
            .or_insert_with(|| (node.omission.default_influence(), 0, Vec::new()));
        entry.1 += 1;
        if entry.2.len() < 3 {
            entry.2.push(node.node_id.clone());
        }
    }
    let mut manifest = OmissionManifest::default();
    for (reason, (influence, count, examples)) in by_reason {
        manifest.push(OmissionGroup {
            reason,
            influence,
            count,
            bound: None,
            examples,
        });
    }
    manifest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn est(tokens: usize) -> TokenEstimate {
        TokenEstimate::declared(tokens)
    }

    fn source() -> CompiledContext {
        CompiledContext::new(
            "compiler/1.0",
            "decision/board",
            "shared",
            "policy/full",
            SufficiencyStatus::Sufficient,
        )
        .with_node(
            ContextNode::new("n/build", NodeKind::Invariant, est(6)).filling_slot("reference_build"),
        )
        .with_node(ContextNode::new("n/expr", NodeKind::Evidence, est(120)).serving("o/expression"))
        .with_node(ContextNode::new("n/conflict", NodeKind::Contradiction, est(40)))
        .with_node(
            ContextNode::new("n/imaging-raw", NodeKind::Evidence, est(900)).with_visibility(
                Visibility::PeerPrivate {
                    owner_role: "imaging".to_string(),
                },
            ),
        )
        .with_node(ContextNode::new("n/attested", NodeKind::AttestedClaim, est(20)))
    }

    fn synthesis_policy() -> ProjectionPolicy {
        ProjectionPolicy::new("policy/synthesis", "synthesis", "board", "typed_synthesis")
            .showing([
                NodeKind::Invariant,
                NodeKind::Evidence,
                NodeKind::AttestedClaim,
                NodeKind::Contradiction,
            ])
            .allowing_expansion("expand:evidence")
    }

    #[test]
    fn a_synthesis_agent_does_not_receive_another_specialists_private_state() {
        let projection = project(&source(), &synthesis_policy()).expect("projects");
        assert!(!projection.node_ids().contains("n/imaging-raw"));
        assert!(projection.dropped_ids().contains("n/imaging-raw"));
    }

    #[test]
    fn it_receives_the_attested_claim_instead_of_the_private_payload() {
        let projection = project(&source(), &synthesis_policy()).expect("projects");
        assert!(projection.node_ids().contains("n/attested"));
    }

    #[test]
    fn a_policy_that_grants_access_to_a_peers_state_receives_it() {
        let policy = synthesis_policy().receiving_private_from("imaging");
        let projection = project(&source(), &policy).expect("projects");
        assert!(projection.node_ids().contains("n/imaging-raw"));
    }

    #[test]
    fn every_dropped_node_is_accounted_for_in_the_projections_own_omission_record() {
        let projection = project(&source(), &synthesis_policy()).expect("projects");
        assert!(projection.accounts_for_every_drop());
        assert_eq!(
            projection.omissions.total_omitted(),
            projection.dropped.len()
        );
    }

    #[test]
    fn a_projection_omission_record_is_separate_from_the_compilers_omission_manifest() {
        let mut compiled = source();
        compiled.omissions.push(OmissionGroup {
            reason: "never_selected".to_string(),
            influence: InfluenceClass::Zero,
            count: 7,
            bound: None,
            examples: vec![],
        });
        let projection = project(&compiled, &synthesis_policy()).expect("projects");
        assert!(projection
            .omissions
            .groups
            .iter()
            .all(|group| group.reason != "never_selected"));
        assert_eq!(compiled.omissions.total_omitted(), 7);
    }

    #[test]
    fn a_node_dropped_because_the_role_cannot_see_its_class_is_unknown_influence_not_zero() {
        let policy = ProjectionPolicy::new("policy/narrow", "stats", "board", "estimate")
            .showing([NodeKind::Evidence]);
        let projection = project(&source(), &policy).expect("projects");
        let group = projection
            .omissions
            .groups
            .iter()
            .find(|group| group.reason.starts_with("kind_not_visible"))
            .expect("records the class drop");
        assert_eq!(group.influence, InfluenceClass::Unknown);
        assert!(!projection.omissions.supports_sufficiency_claim());
    }

    #[test]
    fn a_policy_declaring_contradictions_mandatory_refuses_to_project_without_them() {
        let policy = ProjectionPolicy::new("policy/stats", "stats", "board", "estimate")
            .showing([NodeKind::Evidence])
            .with_mandatory_contradictions();
        let mut hidden = source();
        hidden.nodes[2].visibility = Visibility::PeerPrivate {
            owner_role: "molecular".to_string(),
        };
        assert!(matches!(
            project(&hidden, &policy),
            Err(ProjectionError::MandatoryContradictionDropped { node, .. })
                if node == "n/conflict"
        ));
    }

    #[test]
    fn holdout_state_is_never_projected_whatever_the_policy_says() {
        let leaky = source().with_node(
            ContextNode::new("n/answer", NodeKind::Evidence, est(5))
                .with_visibility(Visibility::Holdout),
        );
        let permissive = synthesis_policy().receiving_private_from("imaging");
        let projection = project(&leaky, &permissive).expect("projects");
        assert!(!projection.node_ids().contains("n/answer"));
        assert!(projection
            .dropped
            .iter()
            .any(|node| node.omission == ProjectionOmission::Holdout));
    }

    #[test]
    fn a_holdout_node_is_dropped_with_no_expansion_offered() {
        let leaky = source().with_node(
            ContextNode::new("n/answer", NodeKind::Evidence, est(5))
                .with_visibility(Visibility::Holdout),
        );
        let policy = synthesis_policy().allowing_expansion("expand:any");
        let projection = project(&leaky, &policy).expect("projects");
        let holdout = projection
            .dropped
            .iter()
            .find(|node| node.node_id == "n/answer")
            .expect("dropped");
        assert_eq!(holdout.expansion, None);
    }

    #[test]
    fn a_projection_that_drops_anything_cannot_still_claim_sufficiency() {
        let projection = project(&source(), &synthesis_policy()).expect("projects");
        assert_eq!(projection.sufficiency, SufficiencyStatus::Unknown);
    }

    #[test]
    fn a_projection_that_drops_nothing_inherits_its_sources_sufficiency() {
        let policy = synthesis_policy().receiving_private_from("imaging");
        let complete = CompiledContext::new(
            "compiler/1.0",
            "d",
            "shared",
            "policy/full",
            SufficiencyStatus::Sufficient,
        )
        .with_node(ContextNode::new("n/a", NodeKind::Evidence, est(10)));
        let projection = project(&complete, &policy).expect("projects");
        assert!(projection.dropped.is_empty());
        assert_eq!(projection.sufficiency, SufficiencyStatus::Sufficient);
    }

    #[test]
    fn a_projection_arriving_with_a_stronger_claim_than_its_source_is_refused_on_receipt() {
        let compiled = source();
        let mut projection = project(&compiled, &synthesis_policy()).expect("projects");
        projection.sufficiency = SufficiencyStatus::Sufficient;
        let mut weaker_source = compiled.clone();
        weaker_source.sufficiency = SufficiencyStatus::Unknown;
        assert!(matches!(
            validate_projection(&projection, &weaker_source),
            Err(ProjectionError::SufficiencyStrengthenedByProjection { .. })
        ));
    }

    #[test]
    fn a_projection_with_an_unrecorded_drop_is_refused_on_receipt() {
        let compiled = source();
        let mut projection = project(&compiled, &synthesis_policy()).expect("projects");
        projection.dropped.push(DroppedNode {
            node_id: "n/silent".to_string(),
            kind: NodeKind::Evidence,
            omission: ProjectionOmission::KindNotVisible {
                kind: NodeKind::Evidence,
            },
            estimate: est(50),
            expansion: None,
        });
        assert!(matches!(
            validate_projection(&projection, &compiled),
            Err(ProjectionError::DropNotAccountedFor {
                dropped: 2,
                accounted: 1,
                ..
            })
        ));
    }

    #[test]
    fn the_cost_of_a_projection_reports_source_kept_and_dropped_estimates_separately() {
        let projection = project(&source(), &synthesis_policy()).expect("projects");
        assert_eq!(projection.cost.source.tokens, 6 + 120 + 40 + 900 + 20);
        assert_eq!(projection.cost.projected.tokens, 6 + 120 + 40 + 20);
        assert_eq!(projection.cost.dropped.tokens, 900);
    }

    #[test]
    fn a_projection_saving_is_never_reported_as_a_measurement() {
        let projection = project(&source(), &synthesis_policy()).expect("projects");
        assert!(!projection.cost.is_measured());
        assert!(projection.cost.saved_fraction().is_some());
    }

    #[test]
    fn a_saving_over_a_total_mixed_across_estimators_is_not_reported() {
        let mut mixed = source();
        mixed.nodes[0].estimate = TokenEstimate::from_provider(6, "cl100k");
        let projection = project(&mixed, &synthesis_policy()).expect("projects");
        assert_eq!(projection.cost.saved_fraction(), None);
    }

    #[test]
    fn a_drop_with_no_permitted_expansion_is_reported_as_unrecoverable() {
        let policy = ProjectionPolicy::new("policy/bare", "stats", "board", "estimate")
            .showing([NodeKind::Evidence]);
        let projection = project(&source(), &policy).expect("projects");
        assert!(!projection.unrecoverable_drops().is_empty());
    }

    #[test]
    fn a_permitted_expansion_travels_with_the_dropped_node_so_the_peer_can_ask() {
        let policy = ProjectionPolicy::new("policy/expandable", "stats", "board", "estimate")
            .showing([NodeKind::Invariant])
            .allowing_expansion("expand:evidence");
        let projection = project(&source(), &policy).expect("projects");
        let dropped = projection
            .dropped
            .iter()
            .find(|node| node.node_id == "n/expr")
            .expect("dropped");
        assert_eq!(dropped.expansion.as_deref(), Some("expand:evidence"));
    }

    #[test]
    fn a_projection_names_the_source_it_was_projected_from() {
        let compiled = source();
        let projection = project(&compiled, &synthesis_policy()).expect("projects");
        assert_eq!(
            projection.source_digest,
            compiled.semantic_digest().expect("digests")
        );
    }

    #[test]
    fn projecting_the_same_context_twice_produces_the_same_digest() {
        let first = project(&source(), &synthesis_policy()).expect("projects");
        let second = project(&source(), &synthesis_policy()).expect("projects");
        assert_eq!(first.digest().expect("digests"), second.digest().expect("digests"));
    }

    #[test]
    fn a_peer_projection_survives_a_json_round_trip() {
        let projection = project(&source(), &synthesis_policy()).expect("projects");
        let text = serde_json::to_string(&projection).expect("serialises");
        let back: PeerProjection = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, projection);
    }
}
