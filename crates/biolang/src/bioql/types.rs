//! The BioQL type system and the schema it checks against.
//!
//! A BioQL type is not "number". It is "a quantity in `mm3`", "a point in a frame that nobody
//! declared", "a locus on GRCh38 read with 0-based half-open coordinates", "an instant on the
//! record clock". That is the whole idea: the questions this platform exists to refuse are refusable
//! only if the declarations that make two values incomparable are part of the type.
//!
//! The declarations themselves are `bioprism-standards` types — [`Unit`], [`FrameBinding`],
//! [`BuildBinding`], [`CoordinateConvention`], [`TermBinding`] — and comparability is decided by
//! [`bioprism_standards::comparable`]. This crate contributes the *type-level* framing and nothing
//! else. There is no second unit table here, no second frame model, and no second notion of what
//! makes two ontology bindings agree.
//!
//! # Where a type comes from
//!
//! Fields get their types from a [`QuerySchema`], which a world publishes. The schema is not
//! inferred, discovered, or defaulted: a field the schema does not declare is an
//! [`crate::error::TypeError::UnknownField`], never an untyped passthrough. A language that let an
//! undeclared field flow through would let exactly the undeclared frames and unversioned ontologies
//! the platform refuses back in through the query layer.
//!
//! # What is deliberately not modelled
//!
//! - **No nullability and no missingness.** `bioprism-bioir`'s `MissingnessClass` is the right
//!   vocabulary for that and it belongs to the measurement, not to the query type.
//! - **No numeric width.** `Integer` versus `Quantity` is a semantic distinction (a count versus a
//!   measured magnitude), not a storage one; there is no `i32` here.
//! - **No records, no lists of records, no joins.** A query reads one collection. 25.21 never
//!   describes a join, and inventing its semantics — especially its scope semantics — would be this
//!   crate writing spec.

use crate::clock::Clock;
use bioprism_scope::ScopeKey;
use bioprism_standards::{
    BuildBinding, CoordinateConvention, Extent, FrameBinding, GenomicPosition, Incomparability,
    Measurement, Observable, Position, Quantity, TermBinding, Unit,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The type of a BioQL expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BioType {
    Bool,
    Text,
    /// A bare number: a count, a ratio, an index.
    ///
    /// Deliberately *not* comparable to a [`BioType::Quantity`]. `tumor_volume > 12.5` with no unit
    /// on the threshold is the commonest way a unit error enters a pipeline, and treating a bare
    /// number as "dimensionless and therefore compatible with anything" is the silent coercion
    /// blueprint 28.00 forbids.
    Number,
    /// An instant. `clock: None` is a literal, which belongs to no clock and may be ordered
    /// against any single clock.
    Instant {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        clock: Option<Clock>,
    },
    /// A scalar magnitude in a declared unit.
    Quantity { unit: Unit },
    /// A frame-independent spatial magnitude: a diameter, an area, a volume.
    Extent { unit: Unit },
    /// A located point, which means nothing without its frame.
    Point { unit: Unit, frame: FrameBinding },
    /// A genomic locus, which means nothing without its build.
    Locus {
        build: BuildBinding,
        convention: CoordinateConvention,
    },
    Set { element: Box<BioType> },
}

impl BioType {
    pub fn quantity(unit: Unit) -> Self {
        BioType::Quantity { unit }
    }

    pub fn extent(unit: Unit) -> Self {
        BioType::Extent { unit }
    }

    pub fn point(unit: Unit, frame: FrameBinding) -> Self {
        BioType::Point { unit, frame }
    }

    pub fn locus(build: BuildBinding, convention: CoordinateConvention) -> Self {
        BioType::Locus { build, convention }
    }

    pub fn instant(clock: Clock) -> Self {
        BioType::Instant { clock: Some(clock) }
    }

    /// True when the value carries standards metadata that comparability depends on.
    pub fn is_measured(&self) -> bool {
        matches!(
            self,
            BioType::Quantity { .. } | BioType::Extent { .. } | BioType::Point { .. } | BioType::Locus { .. }
        )
    }

    /// The clock this type belongs to, when it is an instant.
    pub fn clock(&self) -> Option<Clock> {
        match self {
            BioType::Instant { clock } => *clock,
            _ => None,
        }
    }

    /// Builds the `bioprism-standards` measurement this type would produce.
    ///
    /// The magnitudes are placeholders — `1.0`, contig `"*"`, position `1` — because at type-check
    /// time there is no value, only the declarations attached to it. That is sound for exactly the
    /// checks [`bioprism_standards::comparable`] performs on these variants, all of which read the
    /// unit, frame, build and convention and none of which read the magnitude. The one check it does
    /// read a value for is contig equality, and passing the same synthetic contig on both sides is
    /// what makes the type checker stop at build and convention: a contig is a value, and refusing a
    /// query because two rows might sit on different chromosomes would refuse every query.
    pub fn as_measurement(&self, label: &str) -> Result<Measurement, Incomparability> {
        let observable = match self {
            BioType::Quantity { unit } => Observable::Scalar(Quantity::new(1.0, unit.clone())),
            BioType::Extent { unit } => {
                Observable::Extent(Extent::new(Quantity::new(1.0, unit.clone()))?)
            }
            BioType::Point { unit, frame } => Observable::Located(Position::new(
                [1.0, 1.0, 1.0],
                unit.clone(),
                frame.clone(),
            )),
            BioType::Locus { build, convention } => Observable::Locus(GenomicPosition::new(
                build.clone(),
                SYNTHETIC_CONTIG,
                1,
                *convention,
            )),
            other => {
                return Err(Incomparability::KindMismatch {
                    left: label.to_string(),
                    right: label.to_string(),
                    left_kind: other.to_string(),
                    right_kind: "a measured value".to_string(),
                })
            }
        };
        Ok(Measurement::new(label, observable))
    }
}

/// The contig used when lifting a [`BioType::Locus`] into a measurement.
///
/// Public so a reader of a diagnostic can recognise it as a placeholder rather than a real sequence
/// name. `*` is not a valid contig in any assembly, which is the point.
pub const SYNTHETIC_CONTIG: &str = "*";

impl fmt::Display for BioType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BioType::Bool => f.write_str("bool"),
            BioType::Text => f.write_str("text"),
            BioType::Number => f.write_str("bare number"),
            BioType::Instant { clock: Some(clock) } => write!(f, "instant on {clock}"),
            BioType::Instant { clock: None } => f.write_str("instant (no clock)"),
            BioType::Quantity { unit } => write!(f, "quantity in {}", unit.symbol),
            BioType::Extent { unit } => write!(f, "extent in {}", unit.symbol),
            BioType::Point { unit, frame } => match frame.declared() {
                Some(frame) => write!(f, "point in {} in frame {}", unit.symbol, frame.id),
                None => write!(f, "point in {} in an undeclared frame", unit.symbol),
            },
            BioType::Locus { build, convention } => match build.declared() {
                Some(build) => write!(f, "locus on {} ({convention})", build.label()),
                None => write!(f, "locus on an undeclared build ({convention})"),
            },
            BioType::Set { element } => write!(f, "set of {element}"),
        }
    }
}

/// A field, its type, and what concept it measures.
///
/// The ontology binding lives here rather than inside [`BioType`] because it is a statement about
/// the *field*, not about the shape of its values: two `mm3` quantities are the same type whether
/// or not one of them is bound to a MONDO term.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDecl {
    pub ty: BioType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term: Option<TermBinding>,
}

impl FieldDecl {
    pub fn new(ty: BioType) -> Self {
        FieldDecl { ty, term: None }
    }

    /// Binds the field to an ontology term. Fields bound this way force the query to declare an
    /// expansion policy, because whether `== "glioma"` matches a subtype depends on it.
    pub fn of(mut self, term: TermBinding) -> Self {
        self.term = Some(term);
        self
    }
}

/// One queryable collection: what it contains, where it is valid, and what it costs to read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionDecl {
    pub name: String,
    /// The scope inside which this collection's rows are valid. A query's `in` clause must refine it.
    pub scope: ScopeKey,
    /// True when rows are ordered in time, which forces the query to say which clock it means.
    pub longitudinal: bool,
    /// The declared cost of one scan, in whatever unit the deployment counts in.
    ///
    /// A *declared* number, not a measured or estimated one. This crate reads no statistics and has
    /// no cardinality estimator; 25.21 asks for a cost estimate to exist and be bounded, and a
    /// declared base cost with a syntactic multiplier is the honest version of that.
    pub base_cost: u64,
    fields: BTreeMap<String, FieldDecl>,
}

impl CollectionDecl {
    pub fn new(name: impl Into<String>) -> Self {
        CollectionDecl {
            name: name.into(),
            scope: ScopeKey::new(),
            longitudinal: false,
            base_cost: 1,
            fields: BTreeMap::new(),
        }
    }

    pub fn within(mut self, scope: ScopeKey) -> Self {
        self.scope = scope;
        self
    }

    pub fn longitudinal(mut self) -> Self {
        self.longitudinal = true;
        self
    }

    pub fn costing(mut self, base_cost: u64) -> Self {
        self.base_cost = base_cost;
        self
    }

    pub fn field(mut self, path: impl Into<String>, decl: FieldDecl) -> Self {
        self.fields.insert(path.into(), decl);
        self
    }

    pub fn declare(self, path: impl Into<String>, ty: BioType) -> Self {
        self.field(path, FieldDecl::new(ty))
    }

    pub fn get(&self, path: &str) -> Option<&FieldDecl> {
        self.fields.get(path)
    }

    pub fn field_names(&self) -> impl Iterator<Item = &str> {
        self.fields.keys().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// Every collection a world offers to BioQL.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QuerySchema {
    collections: BTreeMap<String, CollectionDecl>,
}

impl QuerySchema {
    pub fn new() -> Self {
        QuerySchema {
            collections: BTreeMap::new(),
        }
    }

    pub fn with(mut self, collection: CollectionDecl) -> Self {
        self.collections.insert(collection.name.clone(), collection);
        self
    }

    pub fn get(&self, name: &str) -> Option<&CollectionDecl> {
        self.collections.get(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.collections.keys().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.collections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.collections.is_empty()
    }
}
