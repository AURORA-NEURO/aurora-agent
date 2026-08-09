//! The trace explorer's view model: three reasons a node is not on the screen.
//!
//! Implements the checkable part of blueprint 10.14 (Trace and Fork Explorer). Most of 10.14 is a
//! user interface — a timeline, a branch tree, deep links, threaded annotations, a cost waterfall —
//! and `bioprism-hubapi` is right that renderings are nobody's module here. Two parts are not
//! renderings, and one of those is already built:
//!
//! * **First divergence** between two branches is a computation, and `bioprism-trace`'s
//!   `divergence::first_divergence` already does it, on content digests, so that one insertion
//!   reports as one insertion. Not rebuilt.
//! * **Elision** is the part left over, and it is a predicate over a projection.
//!
//! # Why elision needs three states
//!
//! 10.14's scale section: "Progressively load events and artifacts, summarize repetitive spans, and
//! retain exact access to raw evidence. **Never summarize away an effect or grader input by
//! default.**" Its decision lens adds: "Oracle-only content requires reviewer permission."
//!
//! So a node can be off the screen for three different reasons, and a viewer that renders all three
//! as absence is lying in three different ways:
//!
//! | [`Elision`] | What a reviewer should conclude |
//! |---|---|
//! | [`Elision::Collapsed`] | it is here, folded, and the digest will fetch it exactly |
//! | [`Elision::NotLoaded`] | it exists and this view has not fetched it |
//! | [`Elision::Withheld`] | it exists, you are not cleared for it, and here is the reason |
//!
//! [`Elision::retrievable`] separates the first from the second and third. This is the same
//! distinction as [`crate::state`]'s three-valued comparison and [`crate::conform`]'s untested
//! providers, in display currency: an empty region of a view means nothing until you know which of
//! the three produced it.
//!
//! # The refusal
//!
//! [`summarize`] will not collapse a node whose kind is [`NodeKind::Effect`] or
//! [`NodeKind::GraderInput`]. Not "will warn", not "will collapse but keep a flag" — it returns
//! [`SweepError::Malformed`] naming the node. The default in 10.14's sentence is the load-bearing
//! word, and the honest way to implement a default that must not be overridden by convenience is to
//! give the convenient path no way to express the override.
//!
//! # What is not implemented
//!
//! No rendering, no layout, no diffing, no annotations, no deep links, no fork launching. The fork
//! workflow of 10.14 (choose boundary, override architecture, declare budget, preview effects,
//! launch suffixes) is `bioprism-prism` and `bioprism-runtime`'s work and is not duplicated. There
//! is no first-divergence function here for the reason given above.

use serde::{Deserialize, Serialize};

use bioprism_ids::ContentHash;

use crate::error::{require_nonempty, SweepError};
use crate::state::Visibility;

/// What a trace node is. The two protected kinds are the reason the enum exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// An ordinary step: a message, a thought, a retrieval.
    Step,
    /// Something the run did to the world. Never collapsed.
    Effect,
    /// Something an evaluator read in order to reach a verdict. Never collapsed.
    GraderInput,
    /// Repetitive scaffolding — the kind of span 10.14 says to summarise.
    Repetitive,
}

impl NodeKind {
    /// Whether 10.14 forbids summarising this kind away.
    pub fn protected_from_summary(self) -> bool {
        matches!(self, NodeKind::Effect | NodeKind::GraderInput)
    }
}

/// One node of a trace, before projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceNode {
    pub id: String,
    pub kind: NodeKind,
    pub visibility: Visibility,
    pub digest: ContentHash,
}

impl TraceNode {
    pub fn new(
        id: impl Into<String>,
        kind: NodeKind,
        visibility: Visibility,
        digest: ContentHash,
    ) -> Result<Self, SweepError> {
        let id = id.into();
        require_nonempty(&id, "TraceNode", "id")?;
        Ok(TraceNode { id, kind, visibility, digest })
    }
}

/// Why a node is not shown in full.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "elision")]
pub enum Elision {
    /// Shown in full.
    Present,
    /// Folded for display. The digest keeps the raw evidence addressable, and `count` says how many
    /// nodes went into the fold.
    Collapsed { digest: ContentHash, count: usize },
    /// Not fetched by this view. The digest is known, so it can be fetched — but it has not been,
    /// and the view must not imply it has.
    NotLoaded { digest: ContentHash },
    /// The reviewer is not cleared for it. The reason is required: a withheld node with no reason
    /// is indistinguishable from a bug.
    Withheld { reason: String },
}

impl Elision {
    /// Whether the exact evidence can still be reached from this view.
    ///
    /// True for `Present` and `Collapsed`. False for `NotLoaded` — the digest is there, but a view
    /// that has not loaded a node cannot claim the reviewer has access to it through this view —
    /// and false for `Withheld`, where access is the thing being denied.
    pub fn retrievable(&self) -> bool {
        matches!(self, Elision::Present | Elision::Collapsed { .. })
    }

    pub fn is_present(&self) -> bool {
        matches!(self, Elision::Present)
    }
}

/// A node as it appears in a reviewer's view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewNode {
    pub id: String,
    pub kind: NodeKind,
    pub elision: Elision,
}

/// A reviewer's clearance for oracle-only content. 10.14: "Oracle-only content requires reviewer
/// permission."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clearance {
    /// Sees agent-visible and accessible-but-unread content only.
    Reviewer,
    /// Additionally cleared for oracle-only content.
    OracleCleared,
}

/// A projection of a trace for one reviewer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceView {
    clearance: Clearance,
    nodes: Vec<ViewNode>,
}

impl TraceView {
    pub fn nodes(&self) -> &[ViewNode] {
        &self.nodes
    }

    pub fn clearance(&self) -> Clearance {
        self.clearance
    }

    pub fn node(&self, id: &str) -> Option<&ViewNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Every node the view is hiding, whatever the reason. Counts withheld and unloaded nodes, so a
    /// reviewer can see that the view is partial without knowing what is in the gaps.
    pub fn elided(&self) -> usize {
        self.nodes.iter().filter(|n| !n.elision.is_present()).count()
    }

    /// Nodes hidden because the reviewer is not cleared.
    pub fn withheld(&self) -> Vec<&ViewNode> {
        self.nodes
            .iter()
            .filter(|n| matches!(n.elision, Elision::Withheld { .. }))
            .collect()
    }
}

/// Project a trace for a reviewer with a given clearance.
///
/// Oracle-only nodes become [`Elision::Withheld`] rather than disappearing. A view that dropped
/// them would let a reviewer conclude the run made eleven decisions when it made twelve, which is a
/// worse failure than showing them a locked door.
pub fn project(nodes: &[TraceNode], clearance: Clearance) -> TraceView {
    let projected = nodes
        .iter()
        .map(|node| {
            let elision = match (node.visibility, clearance) {
                (Visibility::OracleOnly, Clearance::Reviewer) => Elision::Withheld {
                    reason: "oracle-only content requires reviewer permission".to_string(),
                },
                _ => Elision::Present,
            };
            ViewNode { id: node.id.clone(), kind: node.kind, elision }
        })
        .collect();
    TraceView { clearance, nodes: projected }
}

/// Mark a node as not yet fetched by this view.
///
/// Progressive loading, per 10.14's scale section. A node already withheld stays withheld —
/// pretending an unauthorised node is merely unloaded would leak the fact that it is loadable.
pub fn defer(view: &mut TraceView, id: &str, digest: ContentHash) {
    if let Some(node) = view.nodes.iter_mut().find(|n| n.id == id) {
        if node.elision.is_present() {
            node.elision = Elision::NotLoaded { digest };
        }
    }
}

/// Collapse a run of repetitive nodes into one folded node.
///
/// Refuses if any of them is an effect or a grader input. The collapsed node keeps a digest over
/// the ids it swallowed, so the fold is addressable rather than merely counted.
pub fn summarize(
    view: &mut TraceView,
    ids: &[&str],
    into: &str,
) -> Result<(), SweepError> {
    if ids.is_empty() {
        return Err(SweepError::malformed("summarize", "nothing to collapse"));
    }
    for id in ids {
        let node = view
            .nodes
            .iter()
            .find(|n| n.id == *id)
            .ok_or_else(|| SweepError::malformed("summarize", format!("no node {id}")))?;
        if node.kind.protected_from_summary() {
            return Err(SweepError::malformed(
                "summarize",
                format!("{id} is a {:?} and is never summarised away (10.14)", node.kind),
            ));
        }
    }
    let joined = ids.join("\u{1f}");
    let digest = ContentHash::of_bytes(joined.as_bytes());
    let count = ids.len();
    view.nodes.retain(|n| !ids.contains(&n.id.as_str()));
    view.nodes.push(ViewNode {
        id: into.to_string(),
        kind: NodeKind::Repetitive,
        elision: Elision::Collapsed { digest, count },
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, kind: NodeKind, visibility: Visibility) -> TraceNode {
        TraceNode::new(id, kind, visibility, ContentHash::of_bytes(id.as_bytes())).unwrap()
    }

    fn trace() -> Vec<TraceNode> {
        vec![
            node("n1", NodeKind::Repetitive, Visibility::AgentVisible),
            node("n2", NodeKind::Repetitive, Visibility::AgentVisible),
            node("n3", NodeKind::Effect, Visibility::AgentVisible),
            node("n4", NodeKind::GraderInput, Visibility::AgentVisible),
            node("n5", NodeKind::Step, Visibility::OracleOnly),
        ]
    }

    #[test]
    fn effects_and_grader_inputs_are_protected_and_ordinary_steps_are_not() {
        assert!(NodeKind::Effect.protected_from_summary());
        assert!(NodeKind::GraderInput.protected_from_summary());
        assert!(!NodeKind::Repetitive.protected_from_summary());
        assert!(!NodeKind::Step.protected_from_summary());
    }

    #[test]
    fn an_uncleared_reviewer_sees_oracle_nodes_as_withheld_not_as_absent() {
        let view = project(&trace(), Clearance::Reviewer);
        assert_eq!(view.nodes().len(), 5);
        assert_eq!(view.withheld().len(), 1);
        assert_eq!(view.withheld()[0].id, "n5");
        match &view.node("n5").unwrap().elision {
            Elision::Withheld { reason } => assert!(reason.contains("reviewer permission")),
            other => panic!("expected Withheld, got {other:?}"),
        }
    }

    #[test]
    fn a_cleared_reviewer_sees_the_oracle_node_in_full() {
        let view = project(&trace(), Clearance::OracleCleared);
        assert!(view.withheld().is_empty());
        assert!(view.node("n5").unwrap().elision.is_present());
        assert_eq!(view.clearance(), Clearance::OracleCleared);
    }

    #[test]
    fn summarizing_refuses_to_collapse_an_effect() {
        let mut view = project(&trace(), Clearance::Reviewer);
        let err = summarize(&mut view, &["n1", "n3"], "fold").unwrap_err();
        assert!(format!("{err}").contains("Effect"));
        assert_eq!(view.nodes().len(), 5);
    }

    #[test]
    fn summarizing_refuses_to_collapse_a_grader_input() {
        let mut view = project(&trace(), Clearance::Reviewer);
        assert!(summarize(&mut view, &["n4"], "fold").is_err());
    }

    #[test]
    fn summarizing_repetitive_nodes_folds_them_into_an_addressable_node() {
        let mut view = project(&trace(), Clearance::Reviewer);
        summarize(&mut view, &["n1", "n2"], "fold-1").unwrap();
        assert_eq!(view.nodes().len(), 4);
        match &view.node("fold-1").unwrap().elision {
            Elision::Collapsed { count, digest } => {
                assert_eq!(*count, 2);
                assert_eq!(digest, &ContentHash::of_bytes("n1\u{1f}n2".as_bytes()));
            }
            other => panic!("expected Collapsed, got {other:?}"),
        }
    }

    #[test]
    fn a_collapsed_node_stays_retrievable_and_an_unloaded_one_does_not() {
        let mut view = project(&trace(), Clearance::Reviewer);
        summarize(&mut view, &["n1", "n2"], "fold-1").unwrap();
        defer(&mut view, "n3", ContentHash::of_bytes(b"n3"));
        assert!(view.node("fold-1").unwrap().elision.retrievable());
        assert!(!view.node("n3").unwrap().elision.retrievable());
        assert!(!view.node("n5").unwrap().elision.retrievable());
    }

    #[test]
    fn deferring_a_withheld_node_leaves_it_withheld() {
        let mut view = project(&trace(), Clearance::Reviewer);
        defer(&mut view, "n5", ContentHash::of_bytes(b"n5"));
        assert!(matches!(
            view.node("n5").unwrap().elision,
            Elision::Withheld { .. }
        ));
    }

    #[test]
    fn the_view_reports_how_much_it_is_hiding_without_saying_what() {
        let mut view = project(&trace(), Clearance::Reviewer);
        assert_eq!(view.elided(), 1);
        defer(&mut view, "n3", ContentHash::of_bytes(b"n3"));
        assert_eq!(view.elided(), 2);
        summarize(&mut view, &["n1", "n2"], "fold-1").unwrap();
        assert_eq!(view.elided(), 3);
    }

    #[test]
    fn summarizing_an_unknown_node_or_nothing_at_all_is_an_error() {
        let mut view = project(&trace(), Clearance::Reviewer);
        assert!(summarize(&mut view, &[], "fold").is_err());
        assert!(summarize(&mut view, &["nope"], "fold").is_err());
    }

    #[test]
    fn a_trace_node_needs_an_id() {
        assert!(TraceNode::new(
            "  ",
            NodeKind::Step,
            Visibility::AgentVisible,
            ContentHash::of_bytes(b"x")
        )
        .is_err());
    }
}
