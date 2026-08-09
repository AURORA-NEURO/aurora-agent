//! The normative edge vocabulary.
//!
//! Blueprint 41.03 fixes a finite set of edge types and makes three things non-negotiable: "edge
//! direction is documented", "unknown edge types fail validation", and "requires edges are acyclic
//! at the release-contract layer". The first is served by [`EdgeType::gloss`], the second by
//! [`EdgeType::parse`] returning a typed error, the third by [`crate::lint`].
//!
//! The glosses below are copied verbatim from `machine/edge_vocabulary.json` in the blueprint
//! distribution, including one that reads oddly. Every entry except `governs` describes the edge
//! source-first — `provides` is "source produces inputs used by target". `governs` is given as
//! "target constrains source", i.e. an edge `A --governs--> B` says *B constrains A*. That is
//! backwards from the English word and is recorded in this crate's blueprint-vagueness notes, but
//! it is the normative text and is reproduced rather than corrected. The graph projection emits
//! `governs` in the direction that makes the gloss true: from the decision to the obligation that
//! constrains it.
//!
//! Two members are never emitted by this crate's projections. `supersedes` needs an amendment
//! relation, which 43.09 places in the world's event structure and which a Decision Section does
//! not carry; `related` is defined as "non-normative adjacency", and 43.01's failure-mode contract
//! says that when a relation is ambiguous the system must "expose alternatives instead of
//! inserting a guessed edge". An untyped adjacency edge is a guessed edge with better manners.

use crate::error::ProjectionError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The ten edge types of `machine/edge_vocabulary.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// source indexes target
    Contains,
    /// source tests target
    Evaluates,
    /// target constrains source
    Governs,
    /// source realizes target
    Implements,
    /// source belongs to target section
    PartOf,
    /// source produces inputs used by target
    Provides,
    /// source links to target
    References,
    /// non-normative adjacency
    Related,
    /// source depends on target contract
    Requires,
    /// source replaces target
    Supersedes,
}

impl EdgeType {
    pub const ALL: [EdgeType; 10] = [
        EdgeType::Contains,
        EdgeType::Evaluates,
        EdgeType::Governs,
        EdgeType::Implements,
        EdgeType::PartOf,
        EdgeType::Provides,
        EdgeType::References,
        EdgeType::Related,
        EdgeType::Requires,
        EdgeType::Supersedes,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            EdgeType::Contains => "contains",
            EdgeType::Evaluates => "evaluates",
            EdgeType::Governs => "governs",
            EdgeType::Implements => "implements",
            EdgeType::PartOf => "part_of",
            EdgeType::Provides => "provides",
            EdgeType::References => "references",
            EdgeType::Related => "related",
            EdgeType::Requires => "requires",
            EdgeType::Supersedes => "supersedes",
        }
    }

    /// The blueprint's own wording for this edge, byte for byte.
    ///
    /// Shipped inside every graph view so a renderer states the direction the vocabulary states,
    /// instead of inferring one from the arrowhead.
    pub fn gloss(self) -> &'static str {
        match self {
            EdgeType::Contains => "source indexes target",
            EdgeType::Evaluates => "source tests target",
            EdgeType::Governs => "target constrains source",
            EdgeType::Implements => "source realizes target",
            EdgeType::PartOf => "source belongs to target section",
            EdgeType::Provides => "source produces inputs used by target",
            EdgeType::References => "source links to target",
            EdgeType::Related => "non-normative adjacency",
            EdgeType::Requires => "source depends on target contract",
            EdgeType::Supersedes => "source replaces target",
        }
    }

    /// False only for [`EdgeType::Related`], which the vocabulary itself labels non-normative.
    pub fn is_normative(self) -> bool {
        self != EdgeType::Related
    }

    /// 41.03: unknown edge types fail validation.
    pub fn parse(text: &str) -> Result<EdgeType, ProjectionError> {
        EdgeType::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == text)
            .ok_or_else(|| ProjectionError::UnknownEdgeType {
                value: text.to_string(),
            })
    }
}

impl fmt::Display for EdgeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a node stands for in a projected region.
///
/// The kinds mirror the parts of a Decision Section (43.25) rather than any biological ontology:
/// a projection of a compiled region shows the region, and the domain meaning lives in the scope
/// and provenance attached to each evidence node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// The compiled region itself: one per view, named by the query.
    Decision,
    /// An evidence capsule delivered by the section.
    Evidence,
    /// A typed factor selected into the region.
    Factor,
    /// A variable in the factor calculus.
    Variable,
    /// An obligation the compiler could not discharge.
    Obligation,
    /// The oracle that judged the region.
    Oracle,
    /// One oracle witness — a concrete, checkable disagreement.
    Conflict,
    /// A move on the refinement frontier.
    Refinement,
    /// A stable provenance handle echoed from an evidence capsule.
    Source,
}

impl NodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            NodeKind::Decision => "decision",
            NodeKind::Evidence => "evidence",
            NodeKind::Factor => "factor",
            NodeKind::Variable => "variable",
            NodeKind::Obligation => "obligation",
            NodeKind::Oracle => "oracle",
            NodeKind::Conflict => "conflict",
            NodeKind::Refinement => "refinement",
            NodeKind::Source => "source",
        }
    }
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a node is ordinary, blocked, contradicted, or named-but-not-delivered.
///
/// A typed status rather than a colour: 40.29 forbids colour as the only encoding, and a data
/// layer that never emits colour cannot violate that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    /// Present in the compiled region with nothing outstanding against it.
    Delivered,
    /// An unresolved obligation.
    Blocked,
    /// An oracle witness, or a node one points at.
    Contradicted,
    /// Named by an obligation or a refinement option but absent from the region — the hole the
    /// view must show rather than close.
    Withheld,
}

impl NodeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            NodeStatus::Delivered => "delivered",
            NodeStatus::Blocked => "blocked",
            NodeStatus::Contradicted => "contradicted",
            NodeStatus::Withheld => "withheld",
        }
    }
}
