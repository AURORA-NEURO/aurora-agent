//! Typed failures for the standards layer.
//!
//! Blueprint 28.00 forbids silent coercion, and 39.05 lists units, coordinate system,
//! genome/reference build, orientation and scale as a protected class that may be re-encoded
//! but never omitted. Both requirements collapse to the same engineering rule: when a piece of
//! standards metadata is missing or contradictory, the crate must produce a value that *names
//! the problem*, not a plausible number.
//!
//! [`Incomparability`] is the load-bearing type here. It is the reason a comparison was refused,
//! carried as data so that a report can say "these two volumes differ in genome build" rather
//! than "comparison failed".

use crate::frame::CoordinateSpace;
use crate::unit::Dimension;
use bioprism_scope::ScopeClass;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Failures raised while constructing units or doing arithmetic on quantities.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum UnitError {
    /// The symbol is not in the closed built-in table.
    ///
    /// There is deliberately no fallback that invents a unit from an unrecognised string: a
    /// typo'd `"mL "` must not become a fresh unit that happens to compare equal to nothing.
    #[error("unknown unit symbol {symbol:?}; this crate ships a closed table, not a UCUM parser")]
    UnknownUnit { symbol: String },

    /// The two operands measure different physical dimensions.
    ///
    /// This is the millimetres-plus-millilitres case: no conversion factor exists, so the
    /// operation is rejected outright rather than being offered a scale factor.
    #[error("{operation} across dimensions: {left} is {left_dimension} but {right} is {right_dimension}")]
    DimensionMismatch {
        operation: String,
        left: String,
        right: String,
        left_dimension: Dimension,
        right_dimension: Dimension,
    },

    /// Same dimension, different unit: a conversion exists but was not requested.
    ///
    /// Mirrors `bioprism-bioir`'s `LineageError::UnitMismatch`. Arithmetic never converts
    /// implicitly, because an implicit conversion is a fact about the data that leaves no trace.
    #[error("{operation} across units {left} and {right}; convert explicitly to record it")]
    UnitMismatch {
        operation: String,
        left: String,
        right: String,
    },

    /// Equal dimensions but incompatible quantity kinds.
    ///
    /// `mg/kg` and `%` are both dimensionless. Dimensional analysis alone would happily rescale
    /// a body-weight dose into a percentage, which is why commensurability is a separate check.
    #[error("{left} and {right} share a dimension but are not commensurable: {reason}")]
    NotCommensurable {
        left: String,
        right: String,
        reason: String,
    },

    /// A composed dimension exponent left the `i8` range.
    #[error("dimension exponent overflow composing {left} with {right}")]
    ExponentOverflow { left: String, right: String },

    /// Ratio units are not composable operands.
    ///
    /// Multiplying `mg/kg` by `kg` is meaningful to a pharmacologist but the built-in table
    /// carries no evidence of *which* mass the denominator refers to, so the composition is
    /// refused instead of guessing that the operand supplies it.
    #[error("unit {symbol} is a ratio and may not be composed by multiplication or division")]
    NonCompositional { symbol: String },
}

/// Failures raised while constructing coordinate frames and orientations.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum FrameError {
    /// Two of the three axis letters name the same anatomical axis.
    #[error("orientation {code:?} is degenerate: axis {axis} is named twice")]
    DegenerateOrientation { code: String, axis: String },

    /// A letter outside `RLAPSI`.
    #[error("{letter:?} is not an anatomical axis direction (expected one of R L A P S I)")]
    UnknownAxisLetter { letter: String },

    /// An orientation code that is not exactly three letters.
    #[error("orientation code {code:?} has {length} letters; anatomical orientation needs 3")]
    WrongCodeLength { code: String, length: usize },
}

/// Failures raised while declaring or applying reference-build transports.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum ReferenceError {
    /// A coordinate arrived with no build attached.
    #[error("{context} has no declared reference build; a coordinate without a build is not a location")]
    BuildNotDeclared { context: String },

    /// The transport was declared for a different source build than the coordinate carries.
    #[error("transport lifts from {expected} but the coordinate is on {found}")]
    TransportSourceMismatch { expected: String, found: String },

    /// A cross-build transport that claims to have lost nothing.
    ///
    /// 43.05 treats an empty loss ledger on a transport as a modelling defect. Lifting between
    /// assemblies moves, splits and drops regions; a ledger asserting otherwise is a claim no
    /// liftover tool can support.
    #[error("transport {from} -> {to} declares no loss; cross-build lifting is never free")]
    UndeclaredLoss { from: String, to: String },

    /// An unmapped lift with no stated reason.
    #[error("lift of {context} failed without a stated reason; 'unmapped' must say why")]
    UnmappedWithoutReason { context: String },
}

/// Failures raised while binding local terminology to external ontologies.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum OntologyError {
    #[error("{value:?} is not a well-formed CURIE: {reason}")]
    MalformedCurie { value: String, reason: String },

    /// An identifier without a release string.
    ///
    /// 28.19 names version drift as a characteristic failure mode. An unversioned ontology
    /// identifier cannot be checked against drift later, so it is rejected at construction.
    #[error("identifier {curie} carries no ontology release; unversioned mappings drift silently")]
    MissingRelease { curie: String },

    /// The same local term was bound twice to different targets.
    #[error("local term {local_term:?} already has a different binding in this catalog")]
    ConflictingBinding { local_term: String },

    /// Resolution was attempted on a binding that does not identify a single term.
    #[error("local term {local_term:?} does not resolve to one identifier: {reason}")]
    NotResolvable { local_term: String, reason: String },

    /// A binding with no local term.
    ///
    /// 28.19 requires historical labels to be mapped "without erasing provenance", and 25.03
    /// requires local terminology to survive binding. A binding whose local term is empty has
    /// already erased the thing it was supposed to preserve.
    #[error("a binding to {target} carries no local term; the local term is what must survive")]
    EmptyLocalTerm { target: String },
}

/// Why two measurements may not be compared.
///
/// The variants are returned in a fixed precedence (see [`crate::comparability::CHECK_ORDER`]),
/// so the caller always learns the *first* blocking dimension rather than a bag of complaints.
/// Every variant maps to a [`ScopeClass`] via [`Incomparability::blocking_class`], which keeps
/// this vocabulary inside the 43.03 typed scope base instead of beside it.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "blocked_by", rename_all = "snake_case")]
pub enum Incomparability {
    /// One is a scalar, the other a position or a locus.
    #[error("{left} is a {left_kind} but {right} is a {right_kind}")]
    KindMismatch {
        left: String,
        right: String,
        left_kind: String,
        right_kind: String,
    },

    #[error("{left} is {left_dimension} but {right} is {right_dimension}")]
    DimensionMismatch {
        left: String,
        right: String,
        left_dimension: Dimension,
        right_dimension: Dimension,
    },

    #[error("{left_unit} and {right_unit} share a dimension but are not commensurable: {reason}")]
    NotCommensurable {
        left_unit: String,
        right_unit: String,
        reason: String,
    },

    /// Same dimension, different unit. Comparable only after a recorded conversion.
    #[error("{left_unit} and {right_unit} differ; comparison requires a recorded conversion")]
    ConversionRequired {
        left_unit: String,
        right_unit: String,
    },

    /// At least one position did not say which frame it lives in.
    ///
    /// This is the failure that silence hides: two undeclared frames look identical because
    /// neither said anything, and the comparison then reads as agreement.
    #[error("{side} has no declared coordinate frame; two unstated frames are not a match")]
    UnstatedFrame { side: String },

    #[error("frame {left} and frame {right} are different frames")]
    FrameMismatch { left: String, right: String },

    #[error("orientation {left} and orientation {right} disagree")]
    OrientationMismatch { left: String, right: String },

    /// Voxel indices against world millimetres, or two different voxel grids.
    #[error("{left} and {right} are different coordinate spaces")]
    SpaceMismatch {
        left: CoordinateSpace,
        right: CoordinateSpace,
    },

    #[error("{side} has no declared reference build")]
    UnstatedBuild { side: String },

    #[error("build {left} and build {right} are different assemblies")]
    BuildMismatch { left: String, right: String },

    /// One locus is 0-based half-open, the other 1-based inclusive.
    #[error("coordinate conventions differ: {left} versus {right}")]
    ConventionMismatch { left: String, right: String },

    #[error("contig {left} and contig {right} are different sequences")]
    ContigMismatch { left: String, right: String },

    /// One side carries a subject term and the other does not.
    #[error("{side} carries no ontology binding for what was measured")]
    UnboundTerm { side: String },

    #[error("local term {local_term:?} is unmapped: {reason}")]
    UnmappedTerm { local_term: String, reason: String },

    #[error("local term {local_term:?} has {candidates} candidate identifiers")]
    AmbiguousTerm {
        local_term: String,
        candidates: usize,
    },

    #[error("{left} and {right} come from different ontologies")]
    NamespaceMismatch { left: String, right: String },

    /// Same identifier, different ontology release.
    #[error("{curie} is bound at release {left_release} on one side and {right_release} on the other")]
    OntologyVersionDrift {
        curie: String,
        left_release: String,
        right_release: String,
    },

    /// A broad label against a narrow one.
    #[error("{left} is bound {left_precision} but {right} is bound {right_precision}")]
    GranularityMismatch {
        left: String,
        right: String,
        left_precision: String,
        right_precision: String,
    },

    #[error("{left} and {right} are different terms")]
    TermMismatch { left: String, right: String },
}

impl Incomparability {
    /// The scope dimension class this refusal belongs to.
    ///
    /// Units, frames, orientation and reference build all live in [`ScopeClass::Coordinate`] in
    /// the `bioprism-scope` registry; ontology binding and version drift live in
    /// [`ScopeClass::Ontology`]. Reusing that taxonomy is the point: a comparability refusal is
    /// a statement about a scope dimension, not a new parallel vocabulary.
    pub fn blocking_class(&self) -> ScopeClass {
        match self {
            Incomparability::KindMismatch { .. }
            | Incomparability::DimensionMismatch { .. }
            | Incomparability::NotCommensurable { .. }
            | Incomparability::ConversionRequired { .. }
            | Incomparability::UnstatedFrame { .. }
            | Incomparability::FrameMismatch { .. }
            | Incomparability::OrientationMismatch { .. }
            | Incomparability::SpaceMismatch { .. }
            | Incomparability::UnstatedBuild { .. }
            | Incomparability::BuildMismatch { .. }
            | Incomparability::ConventionMismatch { .. }
            | Incomparability::ContigMismatch { .. } => ScopeClass::Coordinate,
            Incomparability::UnboundTerm { .. }
            | Incomparability::UnmappedTerm { .. }
            | Incomparability::AmbiguousTerm { .. }
            | Incomparability::NamespaceMismatch { .. }
            | Incomparability::OntologyVersionDrift { .. }
            | Incomparability::GranularityMismatch { .. }
            | Incomparability::TermMismatch { .. } => ScopeClass::Ontology,
        }
    }

    /// True when the block is a missing declaration rather than a stated disagreement.
    ///
    /// Worth separating in a report: a disagreement is a finding about the data, while a
    /// missing declaration is a finding about the *metadata* and is usually fixable upstream.
    pub fn is_silence(&self) -> bool {
        matches!(
            self,
            Incomparability::UnstatedFrame { .. }
                | Incomparability::UnstatedBuild { .. }
                | Incomparability::UnboundTerm { .. }
                | Incomparability::UnmappedTerm { .. }
        )
    }
}

/// The crate-level error, for callers that would rather match one type.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StandardsError {
    #[error(transparent)]
    Unit(#[from] UnitError),
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error(transparent)]
    Reference(#[from] ReferenceError),
    #[error(transparent)]
    Ontology(#[from] OntologyError),
    #[error(transparent)]
    Incomparable(#[from] Incomparability),

    /// Canonical encoding failed, which in practice means a non-finite float reached a digest.
    ///
    /// Worth a variant rather than a panic: a `NaN` tumour volume is a real thing to find in
    /// ingested data, and the honest response is to refuse to hash it, not to hash a
    /// placeholder that will compare equal to some other pipeline's placeholder.
    #[error("could not canonically encode {subject}: {detail}")]
    Encoding { subject: String, detail: String },
}
