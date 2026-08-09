//! The compiled context value that every other module in this crate talks about.
//!
//! Blueprint 39 splits the compiler across many modules, and `bioprism-obligation` already owns
//! the parts that decide *what may be dropped*: the obligation graph (39.06), the non-compressible
//! invariants (39.05), the budget controller (39.16), and the omission ledger and sufficiency
//! certificate (39.17). None of those is a *rendered projection*, and the modules this crate
//! implements — golden fixtures (39.21), staleness (39.18), ablations (39.23), peer projection
//! (39.11) — all need one thing to hold in their hands: the artifact a compile produced.
//!
//! [`CompiledContext`] is that artifact and deliberately nothing more. It is a list of selected
//! nodes with their kinds, the obligations they serve, the invariant slots they fill, an omission
//! manifest, a sufficiency status, and a token estimate. It does not contain prose. That is the
//! central design decision of this crate and it is what makes 39.21's second invariant — *"tests
//! assert semantics, not exact prose"* — enforceable rather than aspirational: a fixture cannot
//! accidentally pin wording it never stored.
//!
//! # Rendering is a digest, never a body
//!
//! A node carries [`ContextNode::rendering`], an optional digest of the text that was rendered for
//! it. It is opt-in, it is a hash rather than the text, and every comparison in [`crate::fixture`]
//! treats a rendering-only change as [`crate::fixture::DriftSeverity::Advisory`]. The failure mode
//! named in 39.21 is "brittle prose snapshot"; storing the digest lets a reviewer *see* that
//! wording moved without letting wording fail a build.
//!
//! # Every token number here is an estimate
//!
//! [`ContextNode::estimate`] is a [`TokenEstimate`] from `bioprism-obligation`, which carries the
//! [`bioprism_obligation::EstimationMethod`] that produced it. This crate contains no tokenizer and
//! never will offline, so nothing here is a measurement. `bioprism-docgraph` set the precedent that
//! an estimate must not be presentable as a count, and this crate keeps it: totals degrade to
//! [`bioprism_obligation::EstimationMethod::Mixed`] when the parts came from different rulers, and
//! [`crate::fixture::ContextDrift::EstimatorChanged`] exists because two numbers from two
//! estimators are not comparable at all.
//!
//! # Not implemented
//!
//! No compiler. Nothing here selects evidence, scores a subgraph, or renders anything; a
//! [`CompiledContext`] is a value describing a compile somebody else performed. The selection
//! function of 39.07 and the renderers of 39.04 are out of scope for this crate entirely.

use bioprism_ids::{ContentHash, CanonicalError};
use bioprism_obligation::{EstimationMethod, SufficiencyStatus, TokenEstimate};
use bioprism_section::{InfluenceClass, OmissionManifest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// What a selected node is, structurally.
///
/// The vocabulary is drawn from 39.01's two lists — what may be compressed and what may not. The
/// kinds that appear on the *must not* side ([`NodeKind::Contradiction`],
/// [`NodeKind::NegativeEvidence`], [`NodeKind::Invariant`], [`NodeKind::Uncertainty`],
/// [`NodeKind::PolicyRestriction`]) are distinguished from ordinary evidence precisely so a
/// projection or a fixture can say "this class disappeared" instead of "some node disappeared".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// A non-compressible invariant slot: identity, units, reference build, specimen lineage.
    Invariant,
    /// Ordinary supporting evidence.
    Evidence,
    /// Evidence that conflicts with other selected evidence. 39.01 forbids compressing these away.
    Contradiction,
    /// An absence, a not-measured, a below-detection, or a failed assay. Distinct from missing.
    NegativeEvidence,
    /// An uncertainty distribution or an unresolved state.
    Uncertainty,
    /// A policy, consent or residency restriction that constrains what may be done.
    PolicyRestriction,
    /// A computed view standing in for a raw artifact, governed by [`crate::summary`].
    Summary,
    /// A stable locator for data that was deliberately not inlined (39.12 raw-data handles).
    Handle,
    /// A peer's conclusion with uncertainty and a proof handle, carrying no peer-private payload.
    AttestedClaim,
}

impl NodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            NodeKind::Invariant => "invariant",
            NodeKind::Evidence => "evidence",
            NodeKind::Contradiction => "contradiction",
            NodeKind::NegativeEvidence => "negative_evidence",
            NodeKind::Uncertainty => "uncertainty",
            NodeKind::PolicyRestriction => "policy_restriction",
            NodeKind::Summary => "summary",
            NodeKind::Handle => "handle",
            NodeKind::AttestedClaim => "attested_claim",
        }
    }

    /// Whether 39.01 lists this class under "what must not be compressed away".
    ///
    /// This is a statement about the blueprint, not about any particular decision: a specific
    /// contradiction may turn out to be irrelevant, but dropping it is a decision that has to be
    /// argued in an omission ledger rather than made silently by a packer.
    pub fn is_protected_class(self) -> bool {
        matches!(
            self,
            NodeKind::Invariant
                | NodeKind::Contradiction
                | NodeKind::NegativeEvidence
                | NodeKind::Uncertainty
                | NodeKind::PolicyRestriction
        )
    }
}

/// Who may see a node, before any role projection is applied.
///
/// 39.11 says a synthesis agent "does not receive hidden specialist data unless policy allows it".
/// [`Visibility::PeerPrivate`] is how a node says so about itself, and [`Visibility::Holdout`] is
/// how an evaluator's hidden truth says so — the latter is never legally projected to anyone, which
/// is why it is a separate variant rather than a strict private.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "visibility", rename_all = "snake_case")]
pub enum Visibility {
    /// Any role whose policy admits the node's kind may see it.
    #[default]
    Shared,
    /// Visible only to the named owning role, and to others only as an [`NodeKind::AttestedClaim`].
    PeerPrivate { owner_role: String },
    /// Evaluator-held truth. Not projectable to an evaluated agent under any policy.
    Holdout,
}

impl Visibility {
    pub fn is_holdout(&self) -> bool {
        matches!(self, Visibility::Holdout)
    }
}

/// One selected unit of a compiled context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextNode {
    pub node_id: String,
    pub kind: NodeKind,
    /// The obligation this node was selected to discharge, when it serves one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obligation: Option<String>,
    /// The invariant slot this node fills, for [`NodeKind::Invariant`] nodes. A slot name rather
    /// than a value, so a fixture can assert "the reference build slot is filled" without pinning
    /// which build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invariant_slot: Option<String>,
    /// A stable locator back into the source. 39.13 and 39.14 both require one on every derived
    /// view and every claim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    #[serde(default)]
    pub visibility: Visibility,
    pub estimate: TokenEstimate,
    /// Digest of the rendered text, when the compiler chose to record one. Never the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rendering: Option<String>,
}

impl ContextNode {
    /// A shared evidence node with a declared token estimate.
    pub fn new(node_id: impl Into<String>, kind: NodeKind, estimate: TokenEstimate) -> Self {
        ContextNode {
            node_id: node_id.into(),
            kind,
            obligation: None,
            invariant_slot: None,
            locator: None,
            visibility: Visibility::Shared,
            estimate,
            rendering: None,
        }
    }

    pub fn serving(mut self, obligation: impl Into<String>) -> Self {
        self.obligation = Some(obligation.into());
        self
    }

    pub fn filling_slot(mut self, slot: impl Into<String>) -> Self {
        self.invariant_slot = Some(slot.into());
        self
    }

    pub fn at(mut self, locator: impl Into<String>) -> Self {
        self.locator = Some(locator.into());
        self
    }

    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Records the digest of rendered text. Takes the text, keeps only the hash.
    pub fn rendered_as(mut self, text: &str) -> Self {
        self.rendering = Some(ContentHash::of_bytes(text.as_bytes()).as_str().to_string());
        self
    }
}

/// The artifact a compile produced.
///
/// Ordering of [`CompiledContext::nodes`] is the compiler's presentation order and is preserved,
/// because 39.20's first invariant is that identical inputs produce identical plans and an order
/// that drifted would break that without changing any content. Lookups are by id, so nothing here
/// depends on the order being meaningful.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledContext {
    pub compiler_version: String,
    pub decision_ref: String,
    pub role_ref: String,
    /// The context policy in force. 39.20's comparison mode and 39.23's ablations both turn on
    /// this being an explicit, comparable field rather than an implicit configuration.
    pub policy_id: String,
    pub nodes: Vec<ContextNode>,
    /// What was left out, grouped by structural reason. Reuses `bioprism-section`'s manifest so
    /// the influence vocabulary has exactly one definition in the workspace.
    #[serde(default)]
    pub omissions: OmissionManifest,
    pub sufficiency: SufficiencyStatus,
}

impl CompiledContext {
    pub fn new(
        compiler_version: impl Into<String>,
        decision_ref: impl Into<String>,
        role_ref: impl Into<String>,
        policy_id: impl Into<String>,
        sufficiency: SufficiencyStatus,
    ) -> Self {
        CompiledContext {
            compiler_version: compiler_version.into(),
            decision_ref: decision_ref.into(),
            role_ref: role_ref.into(),
            policy_id: policy_id.into(),
            nodes: Vec::new(),
            omissions: OmissionManifest::default(),
            sufficiency,
        }
    }

    pub fn with_node(mut self, node: ContextNode) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn with_omissions(mut self, omissions: OmissionManifest) -> Self {
        self.omissions = omissions;
        self
    }

    pub fn node(&self, node_id: &str) -> Option<&ContextNode> {
        self.nodes.iter().find(|node| node.node_id == node_id)
    }

    pub fn node_ids(&self) -> BTreeSet<String> {
        self.nodes.iter().map(|node| node.node_id.clone()).collect()
    }

    /// Obligations that at least one selected node claims to serve.
    pub fn covered_obligations(&self) -> BTreeSet<String> {
        self.nodes
            .iter()
            .filter_map(|node| node.obligation.clone())
            .collect()
    }

    /// Invariant slots that at least one selected node fills.
    pub fn filled_invariant_slots(&self) -> BTreeSet<String> {
        self.nodes
            .iter()
            .filter_map(|node| node.invariant_slot.clone())
            .collect()
    }

    pub fn nodes_of_kind(&self, kind: NodeKind) -> impl Iterator<Item = &ContextNode> {
        self.nodes.iter().filter(move |node| node.kind == kind)
    }

    pub fn count_of_kind(&self, kind: NodeKind) -> usize {
        self.nodes_of_kind(kind).count()
    }

    /// Total estimated token cost, carrying the weakest estimator among the parts.
    ///
    /// Delegates to [`TokenEstimate::sum`], which degrades to
    /// [`EstimationMethod::Mixed`] when the parts disagree. A total assembled from two estimators
    /// is nobody's number and the method says so.
    pub fn total_estimate(&self) -> TokenEstimate {
        TokenEstimate::sum(self.nodes.iter().map(|node| &node.estimate))
    }

    /// The single estimation method behind every node, or `None` when they disagree.
    ///
    /// Callers comparing two contexts must check this: a token difference between contexts
    /// estimated by different rules measures the rules, not the contexts.
    pub fn common_estimator(&self) -> Option<EstimationMethod> {
        let mut seen: Option<EstimationMethod> = None;
        for node in &self.nodes {
            match &seen {
                None => seen = Some(node.estimate.method.clone()),
                Some(existing) if existing == &node.estimate.method => {}
                Some(_) => return None,
            }
        }
        seen
    }

    /// Omission groups that block a sufficiency claim, by structural reason.
    pub fn blocking_omission_reasons(&self) -> BTreeMap<String, InfluenceClass> {
        self.omissions
            .blocking_groups()
            .map(|group| (group.reason.clone(), group.influence))
            .collect()
    }

    /// Nodes an evaluated agent must never receive, whatever the role policy says.
    pub fn holdout_nodes(&self) -> BTreeSet<String> {
        self.nodes
            .iter()
            .filter(|node| node.visibility.is_holdout())
            .map(|node| node.node_id.clone())
            .collect()
    }

    /// The semantic content hash: what a fixture compares and what 39.20 requires an API to return.
    ///
    /// Deliberately excludes [`ContextNode::rendering`]. Two compiles that selected the same nodes
    /// for the same obligations with the same omissions have the same semantic digest even if the
    /// wording moved, which is 39.21's "assert semantics, not exact prose" expressed as a hash.
    pub fn semantic_digest(&self) -> Result<String, CanonicalError> {
        let nodes: Vec<Value> = self
            .nodes
            .iter()
            .map(|node| {
                json!({
                    "node_id": node.node_id,
                    "kind": node.kind.as_str(),
                    "obligation": node.obligation,
                    "invariant_slot": node.invariant_slot,
                    "locator": node.locator,
                    "tokens": node.estimate.tokens,
                    "estimator": node.estimate.method.label(),
                })
            })
            .collect();
        let value = json!({
            "compiler_version": self.compiler_version,
            "decision_ref": self.decision_ref,
            "role_ref": self.role_ref,
            "policy_id": self.policy_id,
            "nodes": nodes,
            "omissions": serde_json::to_value(&self.omissions).unwrap_or(Value::Null),
            "sufficiency": self.sufficiency.as_str(),
        });
        ContentHash::of_value(&value).map(|hash| hash.as_str().to_string())
    }

    /// A digest that *does* include rendered text, for a caller that wants to detect wording drift.
    ///
    /// Separate from [`CompiledContext::semantic_digest`] so the two questions cannot be confused.
    /// Nothing in this crate fails a comparison on this hash alone.
    pub fn rendering_digest(&self) -> Result<String, CanonicalError> {
        let renders: Vec<Value> = self
            .nodes
            .iter()
            .map(|node| json!({ "node_id": node.node_id, "rendering": node.rendering }))
            .collect();
        ContentHash::of_value(&json!(renders)).map(|hash| hash.as_str().to_string())
    }
}

/// Whether two token estimates may be subtracted, divided, or otherwise compared.
///
/// Two conditions, and the second is the one that is easy to miss. The estimates must come from the
/// same method — a difference between two rulers is not a difference in cost — **and** that method
/// must not be [`EstimationMethod::Mixed`]. A mixed total is already an aggregate over two
/// estimators, so subtracting one mixed total from another compounds the problem rather than
/// cancelling it, however equal the two `Mixed` labels happen to look.
///
/// Every cost comparison in this crate goes through here: [`crate::projection::ProjectionCost`],
/// [`crate::ablation::ContrastReport`] and [`crate::compiler::PolicyComparison`] all refuse to
/// report a number rather than reporting one nobody can interpret.
pub fn estimates_are_comparable(left: &TokenEstimate, right: &TokenEstimate) -> bool {
    left.method == right.method && !matches!(left.method, EstimationMethod::Mixed { .. })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_section::OmissionGroup;

    fn est(tokens: usize) -> TokenEstimate {
        TokenEstimate::declared(tokens)
    }

    fn ctx() -> CompiledContext {
        CompiledContext::new("c/1", "d/1", "molecular", "policy/a", SufficiencyStatus::Unknown)
            .with_node(
                ContextNode::new("n/build", NodeKind::Invariant, est(4)).filling_slot("reference_build"),
            )
            .with_node(ContextNode::new("n/e1", NodeKind::Evidence, est(40)).serving("o/variant"))
    }

    #[test]
    fn a_semantic_digest_ignores_wording_so_a_rewrite_is_not_a_content_change() {
        let plain = ctx();
        let mut reworded = ctx();
        reworded.nodes[1] = reworded.nodes[1].clone().rendered_as("entirely different prose");
        assert_eq!(
            plain.semantic_digest().expect("digests"),
            reworded.semantic_digest().expect("digests")
        );
        assert_ne!(
            plain.rendering_digest().expect("digests"),
            reworded.rendering_digest().expect("digests")
        );
    }

    #[test]
    fn changing_which_obligation_a_node_serves_changes_the_semantic_digest() {
        let before = ctx();
        let mut after = ctx();
        after.nodes[1].obligation = Some("o/other".to_string());
        assert_ne!(
            before.semantic_digest().expect("digests"),
            after.semantic_digest().expect("digests")
        );
    }

    #[test]
    fn a_rendering_is_stored_as_a_digest_and_never_as_the_text_itself() {
        let node = ContextNode::new("n/x", NodeKind::Evidence, est(3)).rendered_as("SECRET PROSE");
        let rendering = node.rendering.clone().expect("recorded");
        assert!(!rendering.contains("SECRET"));
        assert_eq!(rendering.len(), 64);
    }

    #[test]
    fn a_total_over_two_estimators_reports_a_mixed_method_rather_than_either_one() {
        let mixed = CompiledContext::new("c/1", "d/1", "r", "p", SufficiencyStatus::Unknown)
            .with_node(ContextNode::new("a", NodeKind::Evidence, TokenEstimate::of_text("aaaa")))
            .with_node(ContextNode::new("b", NodeKind::Evidence, TokenEstimate::declared(9)));
        assert!(matches!(
            mixed.total_estimate().method,
            EstimationMethod::Mixed { .. }
        ));
        assert!(mixed.common_estimator().is_none());
    }

    #[test]
    fn no_token_number_in_a_compiled_context_claims_to_be_a_measurement() {
        assert!(!ctx().total_estimate().method.is_measured());
    }

    #[test]
    fn protected_classes_are_the_ones_the_thesis_forbids_compressing_away() {
        for kind in [
            NodeKind::Invariant,
            NodeKind::Contradiction,
            NodeKind::NegativeEvidence,
            NodeKind::Uncertainty,
            NodeKind::PolicyRestriction,
        ] {
            assert!(kind.is_protected_class(), "{} must be protected", kind.as_str());
        }
        for kind in [NodeKind::Evidence, NodeKind::Summary, NodeKind::Handle] {
            assert!(!kind.is_protected_class());
        }
    }

    #[test]
    fn blocking_omission_reasons_name_the_group_rather_than_only_counting_it() {
        let mut manifest = OmissionManifest::default();
        manifest.push(OmissionGroup {
            reason: "dropped_for_budget".to_string(),
            influence: InfluenceClass::Unknown,
            count: 12,
            bound: None,
            examples: vec![],
        });
        let context = ctx().with_omissions(manifest);
        let blocking = context.blocking_omission_reasons();
        assert_eq!(
            blocking.get("dropped_for_budget"),
            Some(&InfluenceClass::Unknown)
        );
    }

    #[test]
    fn a_holdout_node_is_identifiable_without_reading_its_payload() {
        let context = ctx().with_node(
            ContextNode::new("n/truth", NodeKind::Evidence, est(1))
                .with_visibility(Visibility::Holdout),
        );
        assert_eq!(
            context.holdout_nodes().into_iter().collect::<Vec<_>>(),
            vec!["n/truth".to_string()]
        );
    }

    #[test]
    fn a_compiled_context_survives_a_json_round_trip() {
        let context = ctx();
        let text = serde_json::to_string(&context).expect("serialises");
        let back: CompiledContext = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, context);
    }
}
