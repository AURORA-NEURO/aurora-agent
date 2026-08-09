//! The typed documentation edge vocabulary (41.03).
//!
//! 41.03's decision is that agents must know "whether a document is required, produced, related,
//! superseded, or governed by another", and its invariants are that "edge direction is
//! documented", "unknown edge types fail validation", and "requires edges are acyclic at the
//! release-contract layer".
//!
//! The point of typing an edge is that a *traversal* can act on it. So every member here answers
//! four questions mechanically, and nothing in this crate branches on an edge's name:
//!
//! | question | method | used by |
//! |---|---|---|
//! | does including one endpoint oblige the other? | [`DocEdgeType::companion`] | [`crate::bundle`] |
//! | does a change here reach further than one hop? | [`DocEdgeType::propagation`] | [`crate::impact`] |
//! | is a cycle over this type a defect? | [`DocEdgeType::acyclic`] | [`crate::lint`] |
//! | does the edge mean the same thing both ways? | [`DocEdgeType::symmetric`] | both |
//!
//! # There is no `related`
//!
//! The blueprint distribution's `machine/edge_vocabulary.json` carries a tenth member, `related`,
//! glossed "non-normative adjacency". It is not implemented here and never will be. An untyped
//! adjacency edge answers none of the four questions above: a traversal cannot decide whether to
//! follow it, a bundle cannot decide whether it creates an obligation, and an impact analysis
//! cannot decide whether to stop. `bioprism-graph` reaches the same conclusion from the other
//! direction (43.01 requires exposing alternatives rather than "inserting a guessed edge"). If a
//! relation is real, it has a type; if it does not, the honest artifact is no edge.
//!
//! # The `governs` direction is backwards, and is reproduced anyway
//!
//! The normative gloss for `governs` is "target constrains source" — an edge `A --governs--> B`
//! says *B constrains A*, which is the opposite of what the English word suggests. That is the
//! blueprint's text and `bioprism-graph` already reproduces it rather than silently correcting
//! it; correcting it in one crate and not the other would make the same JSON mean two things.
//! Every consumer here reads [`DocEdgeType::gloss`] rather than assuming a direction.
//!
//! # Four members the blueprint does not list
//!
//! `refines`, `contradicts`, `example_of` and the reversed alias `tested_by` are additions. 41.03
//! froze a vocabulary for a corpus of specifications; a corpus that also contains implementations,
//! worked examples and superseded drafts needs to say that one document sharpens another, that two
//! documents cannot both be right, and that a file is an illustration rather than a contract.
//! Each addition earns its place by having a *different* answer to at least one of the four
//! questions than any existing member — `contradicts` is the only symmetric type, `example_of` is
//! the only companion-bearing type that does not propagate impact.

use crate::error::DocGraphError;
use crate::registry::ModuleId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// What including one endpoint of an edge obliges about the other.
///
/// This is the mechanism behind the 41.09 invariant that a bundle preserves "source boundaries"
/// and the 41.07 invariant that "omitted required nodes invalidate the bundle".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionRequirement {
    /// Including the source obliges including the target. `A --requires--> B`: B is what A is
    /// written against, so A alone is not a readable context.
    TargetWithSource,
    /// Including the target obliges including the source. `A --supersedes--> B`: presenting the
    /// superseded B without its successor A is how an agent ends up implementing a withdrawn
    /// contract, so B may not appear alone.
    SourceWithTarget,
    /// Both endpoints or neither. `A --contradicts--> B`: showing one side of a live disagreement
    /// as though it were settled is a defect, not a smaller bundle.
    BothOrNeither,
    /// Neither endpoint obliges the other. The edge is navigational.
    None,
}

/// How far a change at one endpoint reaches (41.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactPropagation {
    /// Impact continues through the affected node's own dependents.
    Transitive,
    /// The immediate neighbour is affected; its neighbours are not affected *by this hop*.
    DirectOnly,
}

/// The twelve documentation edge types.
///
/// Names are the blueprint's where the blueprint has one. `depends_on` is accepted by
/// [`DocEdgeType::parse`] as an alias for [`DocEdgeType::Requires`] because it is the word most
/// callers reach for, but it is not a second member: two names for one relation would make
/// `requires`-acyclicity checkable only over the half of the corpus that used the blueprint's
/// spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocEdgeType {
    /// source depends on target contract
    Requires,
    /// source produces inputs used by target
    Provides,
    /// source belongs to target section
    PartOf,
    /// source indexes target
    Contains,
    /// source links to target
    References,
    /// target constrains source
    Governs,
    /// source realizes target
    Implements,
    /// source tests target
    Evaluates,
    /// source replaces target
    Supersedes,
    /// source sharpens target without replacing it
    Refines,
    /// source and target cannot both be correct
    Contradicts,
    /// source illustrates target and is not normative
    ExampleOf,
}

impl DocEdgeType {
    pub const ALL: [DocEdgeType; 12] = [
        DocEdgeType::Requires,
        DocEdgeType::Provides,
        DocEdgeType::PartOf,
        DocEdgeType::Contains,
        DocEdgeType::References,
        DocEdgeType::Governs,
        DocEdgeType::Implements,
        DocEdgeType::Evaluates,
        DocEdgeType::Supersedes,
        DocEdgeType::Refines,
        DocEdgeType::Contradicts,
        DocEdgeType::ExampleOf,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            DocEdgeType::Requires => "requires",
            DocEdgeType::Provides => "provides",
            DocEdgeType::PartOf => "part_of",
            DocEdgeType::Contains => "contains",
            DocEdgeType::References => "references",
            DocEdgeType::Governs => "governs",
            DocEdgeType::Implements => "implements",
            DocEdgeType::Evaluates => "evaluates",
            DocEdgeType::Supersedes => "supersedes",
            DocEdgeType::Refines => "refines",
            DocEdgeType::Contradicts => "contradicts",
            DocEdgeType::ExampleOf => "example_of",
        }
    }

    /// The direction statement, shipped with the edge so a reader never infers one from an
    /// arrowhead. 41.03: "edge direction is documented".
    pub fn gloss(self) -> &'static str {
        match self {
            DocEdgeType::Requires => "source depends on target contract",
            DocEdgeType::Provides => "source produces inputs used by target",
            DocEdgeType::PartOf => "source belongs to target section",
            DocEdgeType::Contains => "source indexes target",
            DocEdgeType::References => "source links to target",
            DocEdgeType::Governs => "target constrains source",
            DocEdgeType::Implements => "source realizes target",
            DocEdgeType::Evaluates => "source tests target",
            DocEdgeType::Supersedes => "source replaces target",
            DocEdgeType::Refines => "source sharpens target without replacing it",
            DocEdgeType::Contradicts => "source and target cannot both be correct",
            DocEdgeType::ExampleOf => "source illustrates target and is not normative",
        }
    }

    /// Parse a canonical name or a documented same-direction alias.
    ///
    /// Reversed aliases (`tested_by`, `superseded_by`, `depended_on_by`) are deliberately *not*
    /// accepted here: reversing an edge is an operation on endpoints, not on a name, and a
    /// function that only sees the name cannot perform it. Use [`DocEdge::parse`].
    pub fn parse(name: &str) -> Result<Self, DocGraphError> {
        match name {
            "requires" | "depends_on" => Ok(DocEdgeType::Requires),
            "provides" => Ok(DocEdgeType::Provides),
            "part_of" => Ok(DocEdgeType::PartOf),
            "contains" => Ok(DocEdgeType::Contains),
            "references" => Ok(DocEdgeType::References),
            "governs" => Ok(DocEdgeType::Governs),
            "implements" => Ok(DocEdgeType::Implements),
            "evaluates" | "tests" => Ok(DocEdgeType::Evaluates),
            "supersedes" => Ok(DocEdgeType::Supersedes),
            "refines" => Ok(DocEdgeType::Refines),
            "contradicts" => Ok(DocEdgeType::Contradicts),
            "example_of" => Ok(DocEdgeType::ExampleOf),
            other => Err(DocGraphError::UnknownEdgeType(other.to_string())),
        }
    }

    /// A name that denotes this relation with the endpoints swapped, or `None`.
    pub fn reversed_alias(name: &str) -> Option<Self> {
        match name {
            "tested_by" => Some(DocEdgeType::Evaluates),
            "superseded_by" => Some(DocEdgeType::Supersedes),
            "depended_on_by" | "required_by" => Some(DocEdgeType::Requires),
            "refined_by" => Some(DocEdgeType::Refines),
            "example" | "illustrated_by" => Some(DocEdgeType::ExampleOf),
            _ => None,
        }
    }

    pub fn companion(self) -> CompanionRequirement {
        match self {
            DocEdgeType::Requires
            | DocEdgeType::Governs
            | DocEdgeType::Implements
            | DocEdgeType::Refines
            | DocEdgeType::ExampleOf => CompanionRequirement::TargetWithSource,
            DocEdgeType::Supersedes | DocEdgeType::Provides => {
                CompanionRequirement::SourceWithTarget
            }
            DocEdgeType::Contradicts => CompanionRequirement::BothOrNeither,
            DocEdgeType::PartOf
            | DocEdgeType::Contains
            | DocEdgeType::References
            | DocEdgeType::Evaluates => CompanionRequirement::None,
        }
    }

    /// Whether a change at one endpoint reaches past the immediate neighbour.
    ///
    /// The asymmetry that matters: `requires` and `refines` are transitive, `example_of` is not.
    /// A contract's dependents are themselves depended upon, so a change genuinely travels. An
    /// example is a leaf by construction — nothing is written against an illustration, and a
    /// document that cites the example cites it *as* an illustration. Making `example_of`
    /// transitive would let one contract edit reach every document that ever mentioned an example
    /// of it, and an impact report that names the whole corpus is the same as no report. 41.10
    /// asks for results that are "conservative", which is a floor on recall, not a licence to
    /// return everything: depth-1 impact is still emitted for every edge type, so the example is
    /// never silently dropped — only its neighbours are.
    pub fn propagation(self) -> ImpactPropagation {
        match self {
            DocEdgeType::Requires
            | DocEdgeType::Provides
            | DocEdgeType::Governs
            | DocEdgeType::Implements
            | DocEdgeType::Supersedes
            | DocEdgeType::Refines => ImpactPropagation::Transitive,
            DocEdgeType::PartOf
            | DocEdgeType::Contains
            | DocEdgeType::References
            | DocEdgeType::Evaluates
            | DocEdgeType::Contradicts
            | DocEdgeType::ExampleOf => ImpactPropagation::DirectOnly,
        }
    }

    /// Whether a cycle over edges of this type alone is a defect.
    ///
    /// 41.03 mandates it for `requires`. The same argument extends to every type whose meaning is
    /// an ordering — you cannot be part of something that is part of you, nor refine what refines
    /// you. `references` and `governs` may cycle because mutual citation and mutual constraint are
    /// ordinary. `contradicts` must be allowed to cycle because it is symmetric: a single
    /// disagreement is a two-cycle by construction, and flagging it would make every contradiction
    /// a lint error.
    pub fn acyclic(self) -> bool {
        match self {
            DocEdgeType::Requires
            | DocEdgeType::Provides
            | DocEdgeType::PartOf
            | DocEdgeType::Contains
            | DocEdgeType::Implements
            | DocEdgeType::Supersedes
            | DocEdgeType::Refines
            | DocEdgeType::ExampleOf => true,
            DocEdgeType::References | DocEdgeType::Governs | DocEdgeType::Evaluates => false,
            DocEdgeType::Contradicts => false,
        }
    }

    /// Whether the relation reads identically with the endpoints swapped.
    pub fn symmetric(self) -> bool {
        matches!(self, DocEdgeType::Contradicts)
    }

    /// Whether a default traversal follows this edge.
    ///
    /// Derived, not declared: an edge is worth following exactly when following it is what keeps
    /// the bundle honest. Anything that creates a companion obligation, and anything symmetric,
    /// is followed; pure navigation (`references`, `contains`, `part_of`, `evaluates`) is not,
    /// because those expand breadth without changing whether the bundle is correct.
    pub fn followed_by_default(self) -> bool {
        self.companion() != CompanionRequirement::None || self.symmetric()
    }
}

impl fmt::Display for DocEdgeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A typed, directed edge between two registry modules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocEdge {
    pub from: ModuleId,
    pub to: ModuleId,
    pub kind: DocEdgeType,
    /// For [`DocEdgeType::Contradicts`] only: the module that settles the disagreement. `None`
    /// means the contradiction is live, which [`crate::lint`] reports and which forces both
    /// endpoints into any bundle containing either.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<ModuleId>,
    /// Set when the edge was written with a reversed alias and stored canonically. Kept so a
    /// round-trip can reproduce the author's spelling without the graph carrying two directions
    /// for one fact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_from: Option<String>,
}

impl DocEdge {
    pub fn new(from: ModuleId, to: ModuleId, kind: DocEdgeType) -> Self {
        DocEdge {
            from,
            to,
            kind,
            resolved_by: None,
            normalized_from: None,
        }
    }

    /// Parse an edge whose type may be written in either direction.
    ///
    /// `A tested_by B` and `B evaluates A` are the same fact. Storing both spellings would mean
    /// the acyclicity and companion rules had to be written twice, so a reversed alias swaps the
    /// endpoints and records the original name in [`DocEdge::normalized_from`].
    pub fn parse(from: ModuleId, name: &str, to: ModuleId) -> Result<Self, DocGraphError> {
        match DocEdgeType::parse(name) {
            Ok(kind) => Ok(DocEdge::new(from, to, kind)),
            Err(unknown) => match DocEdgeType::reversed_alias(name) {
                Some(kind) => Ok(DocEdge {
                    from: to,
                    to: from,
                    kind,
                    resolved_by: None,
                    normalized_from: Some(name.to_string()),
                }),
                None => Err(unknown),
            },
        }
    }

    pub fn resolved_by(mut self, resolver: ModuleId) -> Self {
        self.resolved_by = Some(resolver);
        self
    }

    /// Sort key giving every listing in this crate a total, content-derived order.
    pub fn order_key(&self) -> (&str, &str, &str) {
        (self.from.as_str(), self.kind.as_str(), self.to.as_str())
    }
}
