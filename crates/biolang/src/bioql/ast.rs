//! The BioQL syntax tree.
//!
//! Blueprint 25.21 names six primary objects — entity query, lineage query, temporal query,
//! capability query, world selection, evidence obligation query — and then never distinguishes their
//! syntax. Six grammars would be six inventions. So there is one query form and the *collection* it
//! reads from is what makes it an entity query or a capability query; the schema
//! ([`crate::bioql::types::QuerySchema`]) says which collections exist and what they contain.
//!
//! The tree is the *untyped* one. Field names are unresolved strings here and a comparison between a
//! volume and a length is perfectly representable; refusing it is [`crate::bioql::check`]'s job. The
//! split matters because a parse error and a type error are different things to tell an author, and
//! collapsing them would mean a query with a genuine syntax problem gets reported as a semantic one.
//!
//! Every node carries a [`Span`], including the clause nodes, so a diagnostic can point at the
//! `aggregate` that lacked a `provenance` rather than at the query as a whole.

use crate::clock::Clock;
use crate::span::Span;
use bioprism_standards::Unit;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A whole query, exactly as written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Query {
    pub select: Projection,
    pub from: CollectionRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeClause>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Expr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expand: Option<ExpansionClause>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<TimeClause>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<LabelClause>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregate: Option<AggregateClause>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<CostClause>,
    pub span: Span,
}

/// What the query returns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "projection", rename_all = "snake_case")]
pub enum Projection {
    /// `select *`.
    ///
    /// Kept as a distinct variant rather than expanded at parse time, because expanding it needs the
    /// schema and the parser deliberately does not have one.
    Everything { span: Span },
    Fields { paths: Vec<Path>, span: Span },
}

impl Projection {
    pub fn span(&self) -> Span {
        match self {
            Projection::Everything { span } | Projection::Fields { span, .. } => *span,
        }
    }
}

/// A dotted field path, e.g. `lesion.volume`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Path {
    pub segments: Vec<String>,
    pub span: Span,
}

impl Path {
    pub fn new(segments: Vec<String>, span: Span) -> Self {
        Path { segments, span }
    }

    /// The dotted rendering used as the schema lookup key and in diagnostics.
    pub fn dotted(&self) -> String {
        self.segments.join(".")
    }
}

/// The collection being read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionRef {
    pub name: String,
    pub span: Span,
}

/// `in { site = "SITE-A", genome_build = "GRCh38" }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeClause {
    pub bindings: Vec<ScopeBinding>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeBinding {
    pub dimension: String,
    pub value: ScopeLiteral,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope_value", rename_all = "snake_case")]
pub enum ScopeLiteral {
    Exact { value: String },
    OneOf { values: BTreeSet<String> },
}

/// `expand ontology "mondo" release "2026-03-01" policy descendants`.
///
/// 25.21 requires the expansion policy to be present and versioned: "Ontology expansion is versioned
/// and visible." The release string is therefore mandatory in the grammar, not optional with a
/// default, for the same reason `bioprism-standards` rejects an unversioned ontology identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpansionClause {
    pub ontology: String,
    pub release: String,
    pub policy: ExpansionPolicy,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpansionPolicy {
    /// Match the named term only.
    Exact,
    /// Match the term and everything it subsumes.
    Descendants,
    /// Match the term and everything that subsumes it.
    Ancestors,
}

impl ExpansionPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            ExpansionPolicy::Exact => "exact",
            ExpansionPolicy::Descendants => "descendants",
            ExpansionPolicy::Ancestors => "ancestors",
        }
    }

    pub fn parse(name: &str) -> Option<ExpansionPolicy> {
        match name {
            "exact" => Some(ExpansionPolicy::Exact),
            "descendants" => Some(ExpansionPolicy::Descendants),
            "ancestors" => Some(ExpansionPolicy::Ancestors),
            _ => None,
        }
    }
}

/// `at event` — which clock the query's time semantics refer to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeClause {
    pub clock: Clock,
    pub span: Span,
}

/// `labels { "phi:deidentified" }`, possibly empty.
///
/// An empty label set is a declaration; an absent clause is not. The type checker refuses the
/// absent one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelClause {
    pub labels: BTreeSet<String>,
    pub span: Span,
}

/// `aggregate mean(volume) provenance source_lineage`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregateClause {
    pub items: Vec<Aggregation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ProvenanceClause>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aggregation {
    pub function: String,
    pub argument: Path,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceClause {
    pub mode: ProvenanceMode,
    pub span: Span,
}

/// How an aggregate keeps its source lineage. 25.21: "Aggregates retain source lineage."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceMode {
    /// Retain the full specimen and evidence lineage of every contributing row.
    SourceLineage,
    /// Retain the contributing evidence identifiers only.
    EvidenceIds,
}

impl ProvenanceMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ProvenanceMode::SourceLineage => "source_lineage",
            ProvenanceMode::EvidenceIds => "evidence_ids",
        }
    }

    pub fn parse(name: &str) -> Option<ProvenanceMode> {
        match name {
            "source_lineage" => Some(ProvenanceMode::SourceLineage),
            "evidence_ids" => Some(ProvenanceMode::EvidenceIds),
            _ => None,
        }
    }
}

/// `cost limit 5000`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostClause {
    pub limit: u64,
    pub span: Span,
}

/// A filter expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "expr", rename_all = "snake_case")]
pub enum Expr {
    Literal {
        value: Literal,
        span: Span,
    },
    Field {
        path: Path,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    /// `{"SITE-A", "SITE-B"}`, the right operand of `in`.
    Set {
        items: Vec<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Set { span, .. } => *span,
            Expr::Field { path } => path.span,
        }
    }

    /// True when this expression is a literal, which is what suppresses the ontology-binding check.
    ///
    /// A threshold is not a measurement of a concept: `volume > 12.5 mm3` compares a quantity to a
    /// number, and demanding that the number carry an ontology binding would make every threshold
    /// unwritable. Field-against-field comparisons get the full check.
    pub fn is_literal(&self) -> bool {
        matches!(self, Expr::Literal { .. })
    }

    /// Every field path this expression reads, in source order.
    pub fn fields(&self) -> Vec<&Path> {
        let mut out = Vec::new();
        self.collect_fields(&mut out);
        out
    }

    fn collect_fields<'a>(&'a self, out: &mut Vec<&'a Path>) {
        match self {
            Expr::Field { path } => out.push(path),
            Expr::Unary { operand, .. } => operand.collect_fields(out),
            Expr::Binary { left, right, .. } => {
                left.collect_fields(out);
                right.collect_fields(out);
            }
            Expr::Set { items, .. } => {
                for item in items {
                    item.collect_fields(out);
                }
            }
            Expr::Literal { .. } => {}
        }
    }

    /// The number of comparison and membership tests, used by the static cost estimate.
    pub fn predicate_count(&self) -> u64 {
        match self {
            Expr::Binary { op, left, right, .. } => {
                let own = u64::from(op.is_predicate());
                own + left.predicate_count() + right.predicate_count()
            }
            Expr::Unary { operand, .. } => operand.predicate_count(),
            Expr::Set { items, .. } => items.iter().map(Expr::predicate_count).sum(),
            Expr::Literal { .. } | Expr::Field { .. } => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "literal", rename_all = "snake_case")]
pub enum Literal {
    /// Struct variants throughout, not newtypes: an internally tagged enum cannot serialize a
    /// newtype variant wrapping a primitive, and the canonical encoder is not negotiable.
    Bool { value: bool },
    Text { value: String },
    /// A number, with the unit suffix the parser resolved through the standards unit table.
    ///
    /// `unit: None` is a bare number, which is comparable only to another bare number or to an
    /// integer-valued field. It is *not* silently treated as dimensionless-and-therefore-compatible
    /// with anything.
    Number {
        value: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unit: Option<Unit>,
        integral: bool,
    },
    /// `instant "2026-03-01T09:00:00Z"`. A literal instant belongs to no clock.
    Instant { rfc3339: String, nanos_utc: i128 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnaryOp {
    Not,
    Negate,
}

impl UnaryOp {
    pub fn as_str(self) -> &'static str {
        match self {
            UnaryOp::Not => "not",
            UnaryOp::Negate => "-",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryOp {
    Or,
    And,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    In,
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl BinaryOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinaryOp::Or => "or",
            BinaryOp::And => "and",
            BinaryOp::Equal => "==",
            BinaryOp::NotEqual => "!=",
            BinaryOp::Less => "<",
            BinaryOp::LessEqual => "<=",
            BinaryOp::Greater => ">",
            BinaryOp::GreaterEqual => ">=",
            BinaryOp::In => "in",
            BinaryOp::Add => "+",
            BinaryOp::Subtract => "-",
            BinaryOp::Multiply => "*",
            BinaryOp::Divide => "/",
        }
    }

    /// Comparisons and membership, which is what the cost estimate counts.
    pub fn is_predicate(self) -> bool {
        matches!(
            self,
            BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual
                | BinaryOp::In
        )
    }

    /// Ordering comparisons, which are the ones subject to the clock rule.
    pub fn is_ordering(self) -> bool {
        matches!(
            self,
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual
        )
    }

    pub fn is_arithmetic(self) -> bool {
        matches!(
            self,
            BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide
        )
    }
}
