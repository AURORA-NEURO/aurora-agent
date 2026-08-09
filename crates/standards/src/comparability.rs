//! The payoff: deciding whether two measurements may be compared at all.
//!
//! Everything else in this crate exists so that this question has a checkable answer. Blueprint
//! 39.05 protects units, coordinate system, reference build, orientation and scale as class 3,
//! and assay/ontology/classifier version as class 4; a comparison that ignores either class is
//! not a weak comparison, it is an undefined one.
//!
//! [`comparable`] returns `Ok(())` or the *first* blocking reason, in the order given by
//! [`CHECK_ORDER`]. First-blocking rather than all-blocking is a deliberate choice: a caller
//! that learns "these are in different genome builds" has something actionable, while a caller
//! handed six simultaneous complaints has to guess which one to fix, and the later checks are
//! frequently meaningless until the earlier one is resolved. Contig equality across two
//! different assemblies is the standard example — `chr7` in GRCh37 and `chr7` in GRCh38 are
//! different sequences, so reporting "contigs agree" alongside "builds differ" would be false.
//!
//! Where the declaration order comes from: location is checked before magnitude because an
//! agreeing number in a disagreeing frame is not evidence, whereas a disagreeing unit in an
//! agreeing frame is a fixable conversion.
//!
//! Ontology binding is handled by policy rather than by fiat. Two measurements that are both
//! unbound are *not* blocked by default, because 39.05 keeps ontology version in a different
//! protected class from coordinate system, and requiring a code on every millimetre would make
//! the crate unusable on exactly the raw data it is meant to describe. What is always blocked
//! is *asymmetry* — one side bound and the other not — since that is a comparison between a
//! coded concept and an uncontrolled string. Callers who want the stricter reading set
//! [`ComparabilityPolicy::require_bound_terms`].

use crate::error::{Incomparability, StandardsError, UnitError};
use crate::frame::{Extent, Position};
use crate::ontology::TermBinding;
use crate::reference::GenomicPosition;
use crate::unit::{ConversionRecord, Quantity};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};

/// The dimensions checked, in the order they are checked.
pub const CHECK_ORDER: &[&str] = &[
    "observable kind",
    "coordinate frame or reference build",
    "physical dimension",
    "unit commensurability",
    "unit identity",
    "ontology binding",
];

/// What a measurement actually is.
///
/// The split between [`Observable::Scalar`], [`Observable::Extent`] and [`Observable::Located`]
/// is the frame-dependence boundary made into types: an extent is the same magnitude in every
/// frame, a located point is not, and a bare scalar carries no spatial claim at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "observable", rename_all = "snake_case")]
pub enum Observable {
    Scalar(Quantity),
    /// A frame-independent spatial magnitude: a diameter, an area, a volume.
    Extent(Extent),
    /// A point, which means nothing without its frame.
    Located(Position),
    Locus(GenomicPosition),
}

impl Observable {
    pub fn kind(&self) -> &'static str {
        match self {
            Observable::Scalar(_) => "scalar",
            Observable::Extent(_) => "extent",
            Observable::Located(_) => "located point",
            Observable::Locus(_) => "genomic locus",
        }
    }
}

/// A measurement, with everything needed to decide whether it is comparable to another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    /// A human label. Never used for comparison; it is documentation, not an identifier.
    pub label: String,
    pub observable: Observable,
    /// What was measured, bound to an ontology. `None` means unbound, not "unknown concept".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_term: Option<TermBinding>,
}

impl Measurement {
    pub fn new(label: impl Into<String>, observable: Observable) -> Self {
        Measurement {
            label: label.into(),
            observable,
            subject_term: None,
        }
    }

    pub fn scalar(label: impl Into<String>, quantity: Quantity) -> Self {
        Measurement::new(label, Observable::Scalar(quantity))
    }

    pub fn extent(label: impl Into<String>, extent: Extent) -> Self {
        Measurement::new(label, Observable::Extent(extent))
    }

    pub fn located(label: impl Into<String>, position: Position) -> Self {
        Measurement::new(label, Observable::Located(position))
    }

    pub fn locus(label: impl Into<String>, position: GenomicPosition) -> Self {
        Measurement::new(label, Observable::Locus(position))
    }

    /// Attaches the ontology binding for the measured concept.
    pub fn of(mut self, term: TermBinding) -> Self {
        self.subject_term = Some(term);
        self
    }
}

/// How strict a comparison should be about ontology binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ComparabilityPolicy {
    /// When true, two unbound measurements are also blocked.
    ///
    /// Appropriate for a cross-cohort merge, where "tumor volume" from two sites is exactly the
    /// kind of string that means two different segmentation protocols.
    pub require_bound_terms: bool,
}

impl ComparabilityPolicy {
    pub fn strict() -> Self {
        ComparabilityPolicy {
            require_bound_terms: true,
        }
    }
}

/// Whether two measurements may be compared as they stand.
///
/// "As they stand" is load-bearing: two lengths in millimetres and centimetres return
/// [`Incomparability::ConversionRequired`] rather than `Ok`, because comparing them requires an
/// act — a conversion — that must be recorded. Use [`reconcile`] to perform and capture it.
pub fn comparable(left: &Measurement, right: &Measurement) -> Result<(), Incomparability> {
    comparable_under(left, right, ComparabilityPolicy::default())
}

/// [`comparable`] with an explicit policy.
pub fn comparable_under(
    left: &Measurement,
    right: &Measurement,
    policy: ComparabilityPolicy,
) -> Result<(), Incomparability> {
    check_observables(left, right)?;
    check_terms(left, right, policy)
}

/// Compares after performing whatever unit conversions are needed, returning their records.
///
/// Only unit differences are reconcilable. A frame mismatch, a build mismatch or a granularity
/// mismatch is returned unchanged, because reconciling those would mean inventing a
/// registration, a chain file or an ontology hierarchy — none of which this crate has.
///
/// The returned records describe the rescale, not a rewritten measurement: the inputs are left
/// untouched. For a located point the record applies to all three coordinates, since a unit
/// change scales the whole triple.
pub fn reconcile(
    left: &Measurement,
    right: &Measurement,
    policy: ComparabilityPolicy,
) -> Result<Vec<ConversionRecord>, Incomparability> {
    match comparable_under(left, right, policy) {
        Ok(()) => Ok(Vec::new()),
        Err(Incomparability::ConversionRequired { .. }) => {
            let record = convert_right_into_left(left, right)?;
            Ok(vec![record])
        }
        Err(other) => Err(other),
    }
}

/// A comparison and everything that had to be true or assumed for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparabilityReport {
    pub left: String,
    pub right: String,
    pub verdict: Verdict,
    /// Conversions performed to reach the verdict. Empty when none were needed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conversions: Vec<ConversionRecord>,
    /// Things that did not block but that a reader should know.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    Comparable,
    Blocked { reason: Incomparability },
}

impl Verdict {
    pub fn is_comparable(&self) -> bool {
        matches!(self, Verdict::Comparable)
    }
}

/// Builds a full report, reconciling units where possible.
///
/// The caveats are the point of having a report type at all: a verdict of `Comparable` reached
/// via a conventional conversion, or with both sides unbound to any ontology, is a different
/// object from one reached with everything declared, and a receipt that hid the difference
/// would be the silent coercion 28.00 forbids.
pub fn report(
    left: &Measurement,
    right: &Measurement,
    policy: ComparabilityPolicy,
) -> ComparabilityReport {
    let mut caveats = Vec::new();
    if left.subject_term.is_none() && right.subject_term.is_none() {
        caveats.push(
            "neither measurement is bound to an ontology term; comparison rests on local labels"
                .to_string(),
        );
    }
    let (verdict, conversions) = match reconcile(left, right, policy) {
        Ok(records) => {
            for record in &records {
                if !record.is_exact() {
                    caveats.push(format!(
                        "conversion {} -> {} is conventional, not exact",
                        record.from, record.to
                    ));
                }
            }
            (Verdict::Comparable, records)
        }
        Err(reason) => (Verdict::Blocked { reason }, Vec::new()),
    };
    ComparabilityReport {
        left: left.label.clone(),
        right: right.label.clone(),
        verdict,
        conversions,
        caveats,
    }
}

impl ComparabilityReport {
    /// A content hash of the report, so a downstream artefact can cite this verdict.
    pub fn digest(&self) -> Result<ContentHash, StandardsError> {
        let value = serde_json::to_value(self).map_err(|error| StandardsError::Encoding {
            subject: "comparability report".to_string(),
            detail: error.to_string(),
        })?;
        ContentHash::of_value(&value).map_err(|error| StandardsError::Encoding {
            subject: "comparability report".to_string(),
            detail: error.to_string(),
        })
    }
}

fn check_observables(left: &Measurement, right: &Measurement) -> Result<(), Incomparability> {
    match (&left.observable, &right.observable) {
        (Observable::Scalar(a), Observable::Scalar(b)) => quantities_comparable(a, b),
        (Observable::Extent(a), Observable::Extent(b)) => a.comparable_with(b),
        (Observable::Located(a), Observable::Located(b)) => a.comparable_with(b),
        (Observable::Locus(a), Observable::Locus(b)) => a.comparable_with(b),
        (a, b) => Err(Incomparability::KindMismatch {
            left: left.label.clone(),
            right: right.label.clone(),
            left_kind: a.kind().to_string(),
            right_kind: b.kind().to_string(),
        }),
    }
}

fn quantities_comparable(left: &Quantity, right: &Quantity) -> Result<(), Incomparability> {
    left.unit
        .commensurable_with(&right.unit)
        .map_err(|error| from_unit_error(error, &left.unit.symbol, &right.unit.symbol))?;
    if left.unit.symbol != right.unit.symbol {
        return Err(Incomparability::ConversionRequired {
            left_unit: left.unit.symbol.clone(),
            right_unit: right.unit.symbol.clone(),
        });
    }
    Ok(())
}

fn check_terms(
    left: &Measurement,
    right: &Measurement,
    policy: ComparabilityPolicy,
) -> Result<(), Incomparability> {
    match (&left.subject_term, &right.subject_term) {
        (Some(a), Some(b)) => a.comparable_with(b),
        (None, None) if policy.require_bound_terms => Err(Incomparability::UnboundTerm {
            side: "both measurements".to_string(),
        }),
        (None, None) => Ok(()),
        (None, Some(_)) => Err(Incomparability::UnboundTerm {
            side: left.label.clone(),
        }),
        (Some(_), None) => Err(Incomparability::UnboundTerm {
            side: right.label.clone(),
        }),
    }
}

fn convert_right_into_left(
    left: &Measurement,
    right: &Measurement,
) -> Result<ConversionRecord, Incomparability> {
    let (target, source) = match (&left.observable, &right.observable) {
        (Observable::Scalar(a), Observable::Scalar(b)) => (a, b),
        (Observable::Extent(a), Observable::Extent(b)) => (&a.magnitude, &b.magnitude),
        (Observable::Located(a), Observable::Located(b)) => {
            let converted = Quantity::new(1.0, b.unit.clone())
                .convert_to(&a.unit)
                .map_err(|error| from_unit_error(error, &a.unit.symbol, &b.unit.symbol))?;
            return Ok(converted.record);
        }
        _ => {
            return Err(Incomparability::KindMismatch {
                left: left.label.clone(),
                right: right.label.clone(),
                left_kind: left.observable.kind().to_string(),
                right_kind: right.observable.kind().to_string(),
            })
        }
    };
    let converted = source
        .convert_to(&target.unit)
        .map_err(|error| from_unit_error(error, &target.unit.symbol, &source.unit.symbol))?;
    Ok(converted.record)
}

fn from_unit_error(error: UnitError, left: &str, right: &str) -> Incomparability {
    match error {
        UnitError::DimensionMismatch {
            left_dimension,
            right_dimension,
            ..
        } => Incomparability::DimensionMismatch {
            left: left.to_string(),
            right: right.to_string(),
            left_dimension,
            right_dimension,
        },
        UnitError::NotCommensurable { reason, .. } => Incomparability::NotCommensurable {
            left_unit: left.to_string(),
            right_unit: right.to_string(),
            reason,
        },
        other => Incomparability::NotCommensurable {
            left_unit: left.to_string(),
            right_unit: right.to_string(),
            reason: other.to_string(),
        },
    }
}
