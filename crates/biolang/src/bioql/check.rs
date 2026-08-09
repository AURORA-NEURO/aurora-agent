//! The BioQL type checker: the part that refuses questions.
//!
//! Implements blueprint 25.21's "type checker" and, through it, the four field groups 25.21 marks
//! Required that nothing else in the platform enforces at query time: ontology expansion policy,
//! time semantics, access labels, aggregation provenance and a cost estimate.
//!
//! # What is worth refusing
//!
//! A query language over biological worlds earns its keep by the questions it will not compile. The
//! rejections here, each with the module that motivates it:
//!
//! | Refused | Because |
//! |---|---|
//! | comparing `mm3` to `cm3` | 28.00, 39.05 class 3: a conversion is an act and must be recorded |
//! | comparing a quantity to a bare number | the same rule, in the form it usually arrives |
//! | comparing points in undeclared frames | `bioprism-standards`: two silences are not a match |
//! | comparing loci across genome builds | 39.05 class 3: `chr7` is a different sequence per build |
//! | comparing terms across ontology releases | 39.05 class 4, 28.19 version drift |
//! | ordering `event_time` against `record_time` | 25.09 temporal leakage |
//! | a query with no access labels | 25.21: "Queries cannot bypass access policy" |
//! | an aggregate with no provenance | 25.21: "Aggregates retain source lineage" |
//! | a query whose scope is wider than its collection's | 43.03: evidence is valid only in its scope |
//!
//! Every comparability refusal is decided by [`bioprism_standards::comparable`], not re-derived here.
//! This crate contributes the type-level framing, the span, and the name of the blocking dimension.
//!
//! # What this is not
//!
//! **There is no execution engine, no optimiser and no planner.** A BioQL query is lexed, parsed and
//! typed here; it is never run. Nothing in this crate reads a row, opens a store, estimates a
//! cardinality or chooses an access path. The cost model is syntactic — a declared base cost times a
//! count of predicates — and is honest about that: it bounds how much a query is *permitted* to
//! cost, which is what 25.21's "cost bounds" gate asks for, and says nothing about what it would
//! actually cost.
//!
//! **There is no permission filter.** 25.21 lists one under validation and conformance. Enforcing it
//! needs a subject, a policy and a store, none of which a type checker has. What is enforced here is
//! that the query *declared* its access labels, so a filter downstream has something to filter on.
//! A declaration is not an enforcement, and calling it one would be the lie this workspace's
//! honest-labelling rule exists to prevent.
//!
//! **One clock rule is a choice, not a derivation.** Ordering two different clocks is refused;
//! equality across clocks is allowed. The blueprint does not decide this. The reasoning: a filter
//! that *orders* event time against record time selects rows by the relationship between two
//! timelines, which is how back-filled results leak into a training window, while
//! `event_time == record_time` is a data-integrity question that must stay expressible.

use crate::bioql::ast::{
    BinaryOp, Expr, ExpansionClause, ExpansionPolicy, Literal, Path, Projection, ProvenanceMode,
    Query, ScopeLiteral, UnaryOp,
};
use crate::bioql::parser::parse;
use crate::bioql::types::{BioType, CollectionDecl, QuerySchema};
use crate::clock::Clock;
use crate::error::{QueryError, TypeError};
use crate::span::Span;
use bioprism_scope::{DimensionRegistry, ScopeKey, ScopeValue};
use bioprism_standards::{comparable, Incomparability, TermBinding, Unit};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The aggregate functions BioQL defines.
///
/// A closed list, like the unit table it sits beside. `count` is separated from the rest because it
/// is the only one whose argument does not have to be measured and whose result is not in the
/// argument's unit.
pub const AGGREGATE_FUNCTIONS: &[&str] = &["count", "mean", "median", "min", "max", "sum"];

/// A query that type-checked, with everything the checker resolved.
///
/// Serializable, and therefore hashable through [`crate::canonical::Canonical`]: a result bundle
/// citing "the query that produced this score" cites this digest, not the source text, so two
/// spellings that differ only in whitespace resolve to the same query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedQuery {
    pub collection: String,
    /// Resolved field names. `select *` is expanded here, so the digest does not depend on whether
    /// the author wrote the star.
    pub projection: Vec<String>,
    pub scope: ScopeKey,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<Clock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expansion: Option<ExpansionClause>,
    pub labels: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aggregations: Vec<TypedAggregation>,
    /// Every field the query reads, with the type it resolved to.
    pub field_types: BTreeMap<String, BioType>,
    pub cost_estimate: u64,
    pub cost_limit: u64,
}

impl TypedQuery {
    /// True when the query reads at least one ontology-bound field.
    pub fn uses_bound_terms(&self) -> bool {
        self.expansion.is_some()
    }

    /// The ontology expansion policy in force, when the query declared one.
    pub fn expansion_policy(&self) -> Option<ExpansionPolicy> {
        self.expansion.as_ref().map(|clause| clause.policy)
    }
}

/// An aggregate with its argument type resolved and its provenance mode attached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedAggregation {
    pub function: String,
    pub argument: String,
    pub argument_type: BioType,
    pub result_type: BioType,
    pub provenance: ProvenanceMode,
}

/// Lexes, parses and type-checks a query in one step.
pub fn compile(source: &str, schema: &QuerySchema) -> Result<TypedQuery, QueryError> {
    let query = parse(source)?;
    Ok(check(&query, schema)?)
}

/// Type-checks an already-parsed query.
pub fn check(query: &Query, schema: &QuerySchema) -> Result<TypedQuery, TypeError> {
    let collection =
        schema
            .get(&query.from.name)
            .ok_or_else(|| TypeError::UnknownCollection {
                name: query.from.name.clone(),
                span: query.from.span,
            })?;

    let labels = query
        .labels
        .as_ref()
        .ok_or(TypeError::AccessLabelsNotDeclared)?
        .labels
        .clone();
    let cost = query.cost.ok_or(TypeError::CostBoundNotDeclared)?;

    if collection.longitudinal && query.time.is_none() {
        return Err(TypeError::TimeSemanticsNotDeclared {
            collection: collection.name.clone(),
        });
    }

    let scope = resolve_scope(query, collection)?;

    let mut checker = Checker {
        collection,
        expansion: query.expand.as_ref(),
        field_types: BTreeMap::new(),
    };

    let projection = match &query.select {
        Projection::Everything { span } => {
            let names: Vec<String> = collection.field_names().map(str::to_string).collect();
            for name in &names {
                checker.resolve(&Path::new(vec![name.clone()], *span))?;
            }
            names
        }
        Projection::Fields { paths, .. } => {
            let mut names = Vec::new();
            for path in paths {
                checker.resolve(path)?;
                names.push(path.dotted());
            }
            names
        }
    };

    if let Some(filter) = &query.filter {
        let filter_type = checker.type_of(filter)?;
        if filter_type != BioType::Bool {
            return Err(TypeError::NotBoolean {
                found: filter_type.to_string(),
                span: filter.span(),
            });
        }
    }

    let mut aggregations = Vec::new();
    if let Some(clause) = &query.aggregate {
        let provenance = clause
            .provenance
            .ok_or_else(|| TypeError::AggregationWithoutProvenance {
                function: clause
                    .items
                    .first()
                    .map(|item| item.function.clone())
                    .unwrap_or_else(|| "aggregate".to_string()),
                span: clause.span,
            })?;
        for item in &clause.items {
            if !AGGREGATE_FUNCTIONS.contains(&item.function.as_str()) {
                return Err(TypeError::UnknownFunction {
                    name: item.function.clone(),
                    span: item.span,
                });
            }
            let argument_type = checker.resolve(&item.argument)?;
            let result_type = match item.function.as_str() {
                "count" => BioType::Number,
                _ => {
                    if !matches!(argument_type, BioType::Quantity { .. } | BioType::Extent { .. } | BioType::Number)
                    {
                        return Err(TypeError::AggregateOverNonMeasured {
                            function: item.function.clone(),
                            found: argument_type.to_string(),
                            span: item.span,
                        });
                    }
                    argument_type.clone()
                }
            };
            aggregations.push(TypedAggregation {
                function: item.function.clone(),
                argument: item.argument.dotted(),
                argument_type,
                result_type,
                provenance: provenance.mode,
            });
        }
    }

    let predicates = query
        .filter
        .as_ref()
        .map(Expr::predicate_count)
        .unwrap_or(0);
    let cost_estimate = estimate_cost(collection.base_cost, predicates, aggregations.len() as u64);
    if cost_estimate > cost.limit {
        return Err(TypeError::CostBoundExceeded {
            estimate: cost_estimate,
            limit: cost.limit,
        });
    }

    Ok(TypedQuery {
        collection: collection.name.clone(),
        projection,
        scope,
        clock: query.time.map(|clause| clause.clock),
        expansion: query.expand.clone(),
        labels,
        aggregations,
        field_types: checker.field_types,
        cost_estimate,
        cost_limit: cost.limit,
    })
}

/// The static cost model: a declared base cost, multiplied by how much work the query asks for.
///
/// Saturating rather than wrapping, because a cost estimate that overflowed to a small number would
/// let the largest queries through the bound.
pub fn estimate_cost(base_cost: u64, predicates: u64, aggregations: u64) -> u64 {
    base_cost
        .saturating_mul(predicates.saturating_add(1))
        .saturating_add(base_cost.saturating_mul(aggregations))
}

fn resolve_scope(query: &Query, collection: &CollectionDecl) -> Result<ScopeKey, TypeError> {
    let registry = DimensionRegistry::default();
    let mut scope = ScopeKey::new();
    if let Some(clause) = &query.scope {
        for binding in &clause.bindings {
            if !registry.classify(&binding.dimension).is_classified() {
                return Err(TypeError::UnclassifiedScopeDimension {
                    dimension: binding.dimension.clone(),
                    span: binding.span,
                });
            }
            let value = match &binding.value {
                ScopeLiteral::Exact { value } => ScopeValue::Exact(value.clone()),
                ScopeLiteral::OneOf { values } => ScopeValue::OneOf(values.clone()),
            };
            scope = scope.bind(binding.dimension.clone(), value);
        }
    }
    if !scope.refines(&collection.scope) {
        let dimension = collection
            .scope
            .iter()
            .find(|(dimension, coarse)| {
                !scope
                    .get(dimension)
                    .is_some_and(|fine| fine.refines(coarse))
            })
            .map(|(dimension, _)| dimension.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        return Err(TypeError::ScopeNotRefining { dimension });
    }
    Ok(scope)
}

struct Checker<'a> {
    collection: &'a CollectionDecl,
    expansion: Option<&'a ExpansionClause>,
    field_types: BTreeMap<String, BioType>,
}

impl<'a> Checker<'a> {
    /// Resolves a field, records its type, and enforces the ontology-expansion requirement.
    fn resolve(&mut self, path: &Path) -> Result<BioType, TypeError> {
        let dotted = path.dotted();
        let decl = self
            .collection
            .get(&dotted)
            .ok_or_else(|| TypeError::UnknownField {
                path: dotted.clone(),
                span: path.span,
                declared: self.collection.len(),
            })?;
        if let Some(term) = &decl.term {
            self.check_expansion(&dotted, term, path.span)?;
        }
        self.field_types.insert(dotted, decl.ty.clone());
        Ok(decl.ty.clone())
    }

    fn term_of(&self, path: &Path) -> Option<&'a TermBinding> {
        self.collection
            .get(&path.dotted())
            .and_then(|decl| decl.term.as_ref())
    }

    /// 25.21: "Ontology expansion is versioned and visible."
    ///
    /// Two failures, not one. A bound field with no expansion policy is under-specified — whether
    /// `== "glioma"` matches a subtype is undecided and the answer changes the result set. A bound
    /// field whose release disagrees with the query's declared release is worse: the query is
    /// expanding against a hierarchy the data was not coded in.
    fn check_expansion(
        &self,
        field: &str,
        term: &TermBinding,
        span: Span,
    ) -> Result<(), TypeError> {
        let ontology = term
            .resolve()
            .map(|id| id.prefix.clone())
            .unwrap_or_else(|_| term.local_term().to_string());
        let Some(expansion) = self.expansion else {
            return Err(TypeError::OntologyExpansionNotDeclared {
                field: field.to_string(),
                ontology,
                span,
            });
        };
        let Ok(id) = term.resolve() else {
            return Ok(());
        };
        if id.prefix.eq_ignore_ascii_case(&expansion.ontology) && id.release != expansion.release {
            return Err(TypeError::incomparable(
                format!("field {field}"),
                format!("expansion policy for {}", expansion.ontology),
                Incomparability::OntologyVersionDrift {
                    curie: id.curie(),
                    left_release: id.release.clone(),
                    right_release: expansion.release.clone(),
                },
                span,
            ));
        }
        Ok(())
    }

    fn type_of(&mut self, expr: &Expr) -> Result<BioType, TypeError> {
        match expr {
            Expr::Literal { value, .. } => Ok(literal_type(value)),
            Expr::Field { path } => self.resolve(path),
            Expr::Unary { op, operand, span } => {
                let inner = self.type_of(operand)?;
                match op {
                    UnaryOp::Not => {
                        if inner == BioType::Bool {
                            Ok(BioType::Bool)
                        } else {
                            Err(TypeError::NotBoolean {
                                found: inner.to_string(),
                                span: *span,
                            })
                        }
                    }
                    UnaryOp::Negate => match inner {
                        BioType::Number | BioType::Quantity { .. } | BioType::Extent { .. } => {
                            Ok(inner)
                        }
                        other => Err(TypeError::NotMeasured {
                            operator: "-".to_string(),
                            left: other.to_string(),
                            right: other.to_string(),
                            span: *span,
                        }),
                    },
                }
            }
            Expr::Set { items, span } => {
                let mut element: Option<BioType> = None;
                for item in items {
                    let item_type = self.type_of(item)?;
                    match &element {
                        None => element = Some(item_type),
                        Some(first) if *first == item_type => {}
                        Some(first) => {
                            return Err(TypeError::HeterogeneousSet {
                                first: first.to_string(),
                                other: item_type.to_string(),
                                span: *span,
                            })
                        }
                    }
                }
                Ok(BioType::Set {
                    element: Box::new(element.unwrap_or(BioType::Text)),
                })
            }
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => self.binary(*op, left, right, *span),
        }
    }

    fn binary(
        &mut self,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        span: Span,
    ) -> Result<BioType, TypeError> {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            for side in [left, right] {
                let ty = self.type_of(side)?;
                if ty != BioType::Bool {
                    return Err(TypeError::NotBoolean {
                        found: ty.to_string(),
                        span: side.span(),
                    });
                }
            }
            return Ok(BioType::Bool);
        }

        let left_type = self.type_of(left)?;
        let right_type = self.type_of(right)?;

        if op == BinaryOp::In {
            let BioType::Set { element } = &right_type else {
                return Err(TypeError::incomparable(
                    describe(left),
                    describe(right),
                    Incomparability::KindMismatch {
                        left: describe(left),
                        right: describe(right),
                        left_kind: left_type.to_string(),
                        right_kind: right_type.to_string(),
                    },
                    span,
                ));
            };
            self.compare(op, left, right, &left_type, element.as_ref(), span)?;
            return Ok(BioType::Bool);
        }

        if op.is_arithmetic() {
            return self.arithmetic(op, left, right, &left_type, &right_type, span);
        }

        self.compare(op, left, right, &left_type, &right_type, span)?;
        Ok(BioType::Bool)
    }

    /// The comparison rule, which is where [`bioprism_standards::comparable`] does the deciding.
    fn compare(
        &mut self,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        left_type: &BioType,
        right_type: &BioType,
        span: Span,
    ) -> Result<(), TypeError> {
        if let (Some(a), Some(b)) = (left_type.clock(), right_type.clock()) {
            if a != b && op.is_ordering() {
                return Err(TypeError::ClockMismatch {
                    left: a,
                    right: b,
                    span,
                });
            }
        }

        match (left_type.is_measured(), right_type.is_measured()) {
            (true, true) => {
                let left_label = describe(left);
                let right_label = describe(right);
                let mut left_measurement = left_type
                    .as_measurement(&left_label)
                    .map_err(|reason| {
                        TypeError::incomparable(&left_label, &right_label, reason, span)
                    })?;
                let mut right_measurement = right_type
                    .as_measurement(&right_label)
                    .map_err(|reason| {
                        TypeError::incomparable(&left_label, &right_label, reason, span)
                    })?;
                if !left.is_literal() && !right.is_literal() {
                    if let Expr::Field { path } = left {
                        left_measurement.subject_term = self.term_of(path).cloned();
                    }
                    if let Expr::Field { path } = right {
                        right_measurement.subject_term = self.term_of(path).cloned();
                    }
                }
                comparable(&left_measurement, &right_measurement).map_err(|reason| {
                    TypeError::incomparable(&left_label, &right_label, reason, span)
                })
            }
            (false, false) => {
                if compatible_unmeasured(left_type, right_type) {
                    Ok(())
                } else {
                    Err(TypeError::incomparable(
                        describe(left),
                        describe(right),
                        Incomparability::KindMismatch {
                            left: describe(left),
                            right: describe(right),
                            left_kind: left_type.to_string(),
                            right_kind: right_type.to_string(),
                        },
                        span,
                    ))
                }
            }
            _ => Err(TypeError::incomparable(
                describe(left),
                describe(right),
                Incomparability::KindMismatch {
                    left: describe(left),
                    right: describe(right),
                    left_kind: left_type.to_string(),
                    right_kind: right_type.to_string(),
                },
                span,
            )),
        }
    }

    fn arithmetic(
        &mut self,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        left_type: &BioType,
        right_type: &BioType,
        span: Span,
    ) -> Result<BioType, TypeError> {
        use BioType::*;
        match (op, left_type, right_type) {
            (_, Number, Number) => Ok(Number),
            (BinaryOp::Add | BinaryOp::Subtract, _, _) if left_type.is_measured() && right_type.is_measured() => {
                self.compare(op, left, right, left_type, right_type, span)?;
                Ok(left_type.clone())
            }
            (BinaryOp::Multiply | BinaryOp::Divide, Quantity { unit }, Number)
            | (BinaryOp::Multiply, Number, Quantity { unit }) => Ok(Quantity {
                unit: unit.clone(),
            }),
            (BinaryOp::Multiply | BinaryOp::Divide, Quantity { unit: a }, Quantity { unit: b }) => {
                compose(op, a, b, span).map(|unit| Quantity { unit })
            }
            _ => Err(TypeError::NotMeasured {
                operator: op.as_str().to_string(),
                left: left_type.to_string(),
                right: right_type.to_string(),
                span,
            }),
        }
    }
}

fn compose(op: BinaryOp, left: &Unit, right: &Unit, span: Span) -> Result<Unit, TypeError> {
    let composed = match op {
        BinaryOp::Multiply => left.checked_mul(right),
        _ => left.checked_div(right),
    };
    composed.map_err(|error| TypeError::UnitComposition {
        operator: op.as_str().to_string(),
        left: left.symbol.clone(),
        right: right.symbol.clone(),
        detail: error.to_string(),
        span,
    })
}

fn literal_type(value: &Literal) -> BioType {
    match value {
        Literal::Bool { .. } => BioType::Bool,
        Literal::Text { .. } => BioType::Text,
        Literal::Number { unit: Some(unit), .. } => BioType::Quantity { unit: unit.clone() },
        Literal::Number { unit: None, .. } => BioType::Number,
        Literal::Instant { .. } => BioType::Instant { clock: None },
    }
}

/// Whether two non-measured types may be compared.
///
/// Instants are the interesting case: a clocked instant and a literal instant are comparable, and
/// two clocked instants are comparable as far as *this* function is concerned — the clock rule is
/// applied separately, because it depends on the operator.
fn compatible_unmeasured(left: &BioType, right: &BioType) -> bool {
    match (left, right) {
        (BioType::Instant { .. }, BioType::Instant { .. }) => true,
        (BioType::Set { element: a }, BioType::Set { element: b }) => compatible_unmeasured(a, b),
        _ => left == right,
    }
}

/// How an expression is named in a diagnostic.
///
/// Field paths render as themselves; literals render as their value, with the unit, so the message
/// reads `tumor_volume and 12.5 cm3 are not comparable` rather than naming two anonymous operands.
fn describe(expr: &Expr) -> String {
    match expr {
        Expr::Field { path } => path.dotted(),
        Expr::Literal { value, .. } => match value {
            Literal::Bool { value } => value.to_string(),
            Literal::Text { value } => format!("{value:?}"),
            Literal::Number {
                value,
                unit: Some(unit),
                ..
            } => format!("{value} {}", unit.symbol),
            Literal::Number { value, .. } => value.to_string(),
            Literal::Instant { rfc3339, .. } => rfc3339.clone(),
        },
        Expr::Unary { op, operand, .. } => format!("{} {}", op.as_str(), describe(operand)),
        Expr::Binary {
            op, left, right, ..
        } => format!("({} {} {})", describe(left), op.as_str(), describe(right)),
        Expr::Set { items, .. } => {
            let rendered: Vec<String> = items.iter().map(describe).collect();
            format!("{{{}}}", rendered.join(", "))
        }
    }
}
