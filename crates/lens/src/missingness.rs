//! Why a quantity has no value — blueprint 42.13.
//!
//! 42.13's stated outcome is to "distinguish biological absence, technical failure, censoring,
//! detection limits, and unknown missingness". Everything else in section 42 is a view; this is
//! the one place where a careless view destroys information that no downstream step can recover.
//!
//! # The distinction that must be unrepresentable to lose
//!
//! A protein that was assayed and found absent, and a protein nobody assayed, are different
//! statements about the world. The first is a measurement. The second is a hole. A table that
//! renders both as an empty cell has silently converted an unknown into a zero, which is the
//! failure `bioprism-section` refuses in its omission manifest (`InfluenceClass::Zero` versus
//! `InfluenceClass::Unknown`) and `bioprism-atlas` refuses in its capability cells (measured-poor
//! versus unmeasured). This module makes the same refusal at the level of a single cell.
//!
//! The mechanism is the shape of the types, not a convention:
//!
//! - [`Observation`] has exactly two arms and neither is an option-like blank. There is no
//!   `Observation::Empty`, no `Option<f64>` accessor, and no `value_or_zero`.
//! - [`Missingness`] has no catch-all variant. Every absence names a class, and every class
//!   names either the assay that ran or the reason none did.
//! - [`Attempt`] is a *tri*-state. "Measured and absent" and "never measured" are joined by
//!   "unknown whether anyone measured", because a record that omits the assay history does not
//!   thereby prove no assay happened.
//! - [`MissingnessSummary`] deliberately exposes no total. Summing the classes is the collapse;
//!   see [`MissingnessSummary::by_class`].
//!
//! # Not implemented
//!
//! No imputation, and no missing-data mechanism inference (MCAR/MAR/MNAR). The blueprint names
//! neither, and inferring a mechanism from the classes here would manufacture exactly the
//! confidence this module exists to withhold.

use crate::error::LensError;
use bioprism_section::InfluenceClass;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Whether a measurement was attempted at all.
///
/// The third arm is the point. A record can fail to say whether an assay ran, and that state is
/// not "no assay ran" — it is a gap in the provenance, which is a different defect with a
/// different remedy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Attempt {
    /// An assay ran and returned something, including a negative result.
    Performed,
    /// No assay ran, and the record says so.
    NotPerformed,
    /// The record does not say. Never counts as evidence of absence in either direction.
    Unknown,
}

impl Attempt {
    pub fn as_str(self) -> &'static str {
        match self {
            Attempt::Performed => "performed",
            Attempt::NotPerformed => "not_performed",
            Attempt::Unknown => "unknown",
        }
    }
}

/// Which side of a bound a censored value lies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CensorDirection {
    /// The true value is at most the bound.
    Left,
    /// The true value is at least the bound.
    Right,
}

impl CensorDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            CensorDirection::Left => "left",
            CensorDirection::Right => "right",
        }
    }
}

/// Why no assay produced a value.
///
/// `Unrecorded` is the honest bottom of the lattice and the only one that admits ignorance about
/// its own ignorance. It maps to [`InfluenceClass::Unknown`] like the rest, but a lens reports it
/// separately because the fix is a provenance fix, not an experiment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnattemptedReason {
    /// The assay was never ordered for this specimen.
    NotOrdered,
    /// The specimen was consumed before this assay could run, so the value is unobtainable.
    SpecimenExhausted,
    /// The platform does not measure this analyte.
    AssayNotAvailable,
    /// The record does not say whether an assay ran.
    Unrecorded,
}

impl UnattemptedReason {
    pub fn as_str(self) -> &'static str {
        match self {
            UnattemptedReason::NotOrdered => "not_ordered",
            UnattemptedReason::SpecimenExhausted => "specimen_exhausted",
            UnattemptedReason::AssayNotAvailable => "assay_not_available",
            UnattemptedReason::Unrecorded => "unrecorded",
        }
    }
}

/// The coarse class of an absence, for grouping without discarding the detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingnessClass {
    BiologicalAbsence,
    TechnicalFailure,
    Censored,
    BelowDetectionLimit,
    PolicyWithheld,
    NeverMeasured,
}

impl MissingnessClass {
    pub const ALL: [MissingnessClass; 6] = [
        MissingnessClass::BiologicalAbsence,
        MissingnessClass::TechnicalFailure,
        MissingnessClass::Censored,
        MissingnessClass::BelowDetectionLimit,
        MissingnessClass::PolicyWithheld,
        MissingnessClass::NeverMeasured,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            MissingnessClass::BiologicalAbsence => "biological_absence",
            MissingnessClass::TechnicalFailure => "technical_failure",
            MissingnessClass::Censored => "censored",
            MissingnessClass::BelowDetectionLimit => "below_detection_limit",
            MissingnessClass::PolicyWithheld => "policy_withheld",
            MissingnessClass::NeverMeasured => "never_measured",
        }
    }
}

/// Why a quantity has no value.
///
/// There is no `Missingness::Other` and no `Missingness::Unknown` free of structure: the last
/// variant is `NeverMeasured`, which still demands a reason. An absence that cannot name its
/// class is not representable, so it cannot reach a view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "class", rename_all = "snake_case")]
pub enum Missingness {
    /// An assay ran and the analyte was not present. This is a *result*.
    BiologicalAbsence { assay: String },
    /// An assay ran and failed. There is no value and no negative result either.
    TechnicalFailure { assay: String, detail: String },
    /// An assay ran; the value is known only to lie beyond a bound.
    Censored {
        assay: String,
        direction: CensorDirection,
        bound: f64,
    },
    /// An assay ran; the value is below what the platform can resolve. Distinct from
    /// `BiologicalAbsence` because the analyte may well be present.
    BelowDetectionLimit { assay: String, limit: f64 },
    /// A value may exist but consent or policy forbids access to it. The gap is real and must be
    /// carried into the decision rather than treated as nothing.
    PolicyWithheld { authority: String },
    /// Nobody produced a value. The hole.
    NeverMeasured { reason: UnattemptedReason },
}

impl Missingness {
    pub fn class(&self) -> MissingnessClass {
        match self {
            Missingness::BiologicalAbsence { .. } => MissingnessClass::BiologicalAbsence,
            Missingness::TechnicalFailure { .. } => MissingnessClass::TechnicalFailure,
            Missingness::Censored { .. } => MissingnessClass::Censored,
            Missingness::BelowDetectionLimit { .. } => MissingnessClass::BelowDetectionLimit,
            Missingness::PolicyWithheld { .. } => MissingnessClass::PolicyWithheld,
            Missingness::NeverMeasured { .. } => MissingnessClass::NeverMeasured,
        }
    }

    /// Whether an assay ran.
    ///
    /// `PolicyWithheld` is [`Attempt::Unknown`] rather than `Performed`: a policy boundary hides
    /// the assay history along with the value.
    pub fn attempt(&self) -> Attempt {
        match self {
            Missingness::BiologicalAbsence { .. }
            | Missingness::TechnicalFailure { .. }
            | Missingness::Censored { .. }
            | Missingness::BelowDetectionLimit { .. } => Attempt::Performed,
            Missingness::PolicyWithheld { .. } => Attempt::Unknown,
            Missingness::NeverMeasured { reason } => match reason {
                UnattemptedReason::Unrecorded => Attempt::Unknown,
                _ => Attempt::NotPerformed,
            },
        }
    }

    /// Whether this absence is itself a measurement result.
    ///
    /// True exactly for absences an experiment established. This is the predicate a view must
    /// consult before writing anything resembling "not detected".
    pub fn is_measured_result(&self) -> bool {
        matches!(
            self,
            Missingness::BiologicalAbsence { .. }
                | Missingness::Censored { .. }
                | Missingness::BelowDetectionLimit { .. }
        )
    }

    /// The omission vocabulary of 43.26, so this module and `bioprism-section` gate on one
    /// predicate.
    ///
    /// A measured absence is [`InfluenceClass::Zero`] — the quantity is known, so nothing is
    /// missing from the decision. A censored or truncated value is `Bounded`. A failed assay is
    /// `DeferredAcquisition` because a rerun may recover it. Policy is `InaccessibleByPolicy`.
    /// Everything nobody measured is `Unknown`, and one `Unknown` voids a sufficiency claim.
    pub fn influence_class(&self) -> InfluenceClass {
        match self {
            Missingness::BiologicalAbsence { .. } => InfluenceClass::Zero,
            Missingness::Censored { .. } | Missingness::BelowDetectionLimit { .. } => {
                InfluenceClass::Bounded
            }
            Missingness::TechnicalFailure { .. } => InfluenceClass::DeferredAcquisition,
            Missingness::PolicyWithheld { .. } => InfluenceClass::InaccessibleByPolicy,
            Missingness::NeverMeasured { .. } => InfluenceClass::Unknown,
        }
    }

    /// A sentence a screen reader can speak, per 42.27. Never "blank", never "n/a".
    pub fn sentence(&self) -> String {
        match self {
            Missingness::BiologicalAbsence { assay } => {
                format!("measured by {assay} and found absent")
            }
            Missingness::TechnicalFailure { assay, detail } => {
                format!("{assay} was attempted and failed: {detail}")
            }
            Missingness::Censored {
                assay,
                direction,
                bound,
            } => match direction {
                CensorDirection::Left => format!("measured by {assay}; value at most {bound}"),
                CensorDirection::Right => format!("measured by {assay}; value at least {bound}"),
            },
            Missingness::BelowDetectionLimit { assay, limit } => {
                format!("measured by {assay}; below the detection limit of {limit}")
            }
            Missingness::PolicyWithheld { authority } => {
                format!("withheld by {authority}; whether it was measured is not disclosed")
            }
            Missingness::NeverMeasured { reason } => match reason {
                UnattemptedReason::NotOrdered => "never measured: the assay was not ordered".into(),
                UnattemptedReason::SpecimenExhausted => {
                    "never measured: the specimen was exhausted".into()
                }
                UnattemptedReason::AssayNotAvailable => {
                    "never measured: no assay measures this analyte".into()
                }
                UnattemptedReason::Unrecorded => {
                    "never measured, or measured without a record; the provenance does not say"
                        .into()
                }
            },
        }
    }
}

/// A quantity that exists.
///
/// Private fields with one fallible constructor, because a non-finite float is the standard
/// route by which a "present" value becomes a blank cell two layers downstream. NaN is not a
/// value; it is an absence that forgot to say why, and this type refuses to carry it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "MeasuredFields")]
pub struct Measured {
    value: f64,
    unit: String,
    assay: String,
}

#[derive(Deserialize)]
struct MeasuredFields {
    value: f64,
    unit: String,
    assay: String,
}

impl TryFrom<MeasuredFields> for Measured {
    type Error = LensError;

    fn try_from(fields: MeasuredFields) -> Result<Self, Self::Error> {
        Measured::new(fields.value, fields.unit, fields.assay, "<deserialized>")
    }
}

impl Measured {
    /// `analyte` names the quantity only so a rejection can say what it was.
    pub fn new(
        value: f64,
        unit: impl Into<String>,
        assay: impl Into<String>,
        analyte: &str,
    ) -> Result<Self, LensError> {
        let assay = assay.into();
        if !value.is_finite() {
            return Err(LensError::NonFiniteMeasurement {
                assay,
                analyte: analyte.to_string(),
            });
        }
        Ok(Measured {
            value,
            unit: unit.into(),
            assay,
        })
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn unit(&self) -> &str {
        &self.unit
    }

    pub fn assay(&self) -> &str {
        &self.assay
    }
}

/// What is known about one quantity: a value, or a named reason there is none.
///
/// Deliberately not `Option<Measured>`. `None` is a single state, and this module's entire claim
/// is that absence has six of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Observation {
    Present(Measured),
    Absent(Missingness),
}

impl Observation {
    pub fn attempt(&self) -> Attempt {
        match self {
            Observation::Present(_) => Attempt::Performed,
            Observation::Absent(missing) => missing.attempt(),
        }
    }

    /// The influence class of the *gap*, for a decision that needs this quantity.
    pub fn influence_class(&self) -> InfluenceClass {
        match self {
            Observation::Present(_) => InfluenceClass::Zero,
            Observation::Absent(missing) => missing.influence_class(),
        }
    }

    /// Whether this observation may participate in a sufficiency claim.
    pub fn supports_sufficiency(&self) -> bool {
        self.influence_class().supports_sufficiency()
    }

    pub fn sentence(&self) -> String {
        match self {
            Observation::Present(m) => format!("{} {} by {}", m.value(), m.unit(), m.assay()),
            Observation::Absent(missing) => missing.sentence(),
        }
    }
}

/// A categorical attribute that is either recorded or absent with a named reason.
///
/// [`Observation`] does this for quantities; this does it for everything else — a site, a
/// scanner, a label timestamp, the token cost of an expansion. `Option<T>` would be the obvious
/// choice and is the wrong one for the same reason as before: `None` cannot say whether the field
/// was checked and empty or never populated, and a lens that reads `None` as "no site" will
/// cheerfully certify that a cohort has no site confounding when in truth nobody recorded sites.
///
/// Note the asymmetry this type creates in every check built on it: a check over a `Missing`
/// input cannot return a negative result. It can only report that it did not run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "recorded", rename_all = "snake_case")]
pub enum Recorded<T> {
    Known { value: T },
    Missing { missingness: Missingness },
}

impl<T> Recorded<T> {
    pub fn known(value: T) -> Self {
        Recorded::Known { value }
    }

    pub fn unrecorded() -> Self {
        Recorded::Missing {
            missingness: Missingness::NeverMeasured {
                reason: UnattemptedReason::Unrecorded,
            },
        }
    }

    pub fn missing(missingness: Missingness) -> Self {
        Recorded::Missing { missingness }
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Recorded::Known { value } => Some(value),
            Recorded::Missing { .. } => None,
        }
    }

    pub fn missingness(&self) -> Option<&Missingness> {
        match self {
            Recorded::Known { .. } => None,
            Recorded::Missing { missingness } => Some(missingness),
        }
    }

    pub fn is_known(&self) -> bool {
        matches!(self, Recorded::Known { .. })
    }
}

/// Counts of absences by class over some set of cells.
///
/// There is no `total_missing`, and that omission is the design. A single number over these six
/// classes reads as "how much data is missing", which is precisely the sentence that treats a
/// biological absence and an unordered assay as the same event. Callers that genuinely need a
/// denominator use [`MissingnessSummary::observed`] together with [`MissingnessSummary::by_class`]
/// and decide, in the open, which classes they are willing to add up.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingnessSummary {
    observed: usize,
    counts: BTreeMap<MissingnessClass, usize>,
}

impl MissingnessSummary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(&mut self, observation: &Observation) {
        match observation {
            Observation::Present(_) => self.observed += 1,
            Observation::Absent(missing) => {
                *self.counts.entry(missing.class()).or_insert(0) += 1;
            }
        }
    }

    /// How many cells carried a value.
    pub fn observed(&self) -> usize {
        self.observed
    }

    pub fn by_class(&self) -> &BTreeMap<MissingnessClass, usize> {
        &self.counts
    }

    pub fn count(&self, class: MissingnessClass) -> usize {
        self.counts.get(&class).copied().unwrap_or(0)
    }

    /// Absences an experiment established. Safe to reason from.
    pub fn measured_absences(&self) -> usize {
        self.count(MissingnessClass::BiologicalAbsence)
            + self.count(MissingnessClass::Censored)
            + self.count(MissingnessClass::BelowDetectionLimit)
    }

    /// Absences nobody established. Never safe to reason from, and never the same number as
    /// [`MissingnessSummary::measured_absences`].
    pub fn holes(&self) -> usize {
        self.count(MissingnessClass::NeverMeasured)
    }

    /// True when every absence in the set is either a measured result or an explicit bound.
    pub fn supports_sufficiency_claim(&self) -> bool {
        self.counts
            .keys()
            .all(|class| class_influence(*class).supports_sufficiency())
    }
}

fn class_influence(class: MissingnessClass) -> InfluenceClass {
    match class {
        MissingnessClass::BiologicalAbsence => InfluenceClass::Zero,
        MissingnessClass::Censored | MissingnessClass::BelowDetectionLimit => {
            InfluenceClass::Bounded
        }
        MissingnessClass::TechnicalFailure => InfluenceClass::DeferredAcquisition,
        MissingnessClass::PolicyWithheld => InfluenceClass::InaccessibleByPolicy,
        MissingnessClass::NeverMeasured => InfluenceClass::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn never_measured() -> Observation {
        Observation::Absent(Missingness::NeverMeasured {
            reason: UnattemptedReason::NotOrdered,
        })
    }

    fn measured_absent() -> Observation {
        Observation::Absent(Missingness::BiologicalAbsence {
            assay: "IHC-PDL1".into(),
        })
    }

    #[test]
    fn a_quantity_nobody_measured_is_not_rendered_as_an_absent_measurement() {
        let hole = never_measured();
        let absence = measured_absent();

        assert_ne!(hole, absence);
        assert_ne!(hole.sentence(), absence.sentence());
        assert_eq!(hole.attempt(), Attempt::NotPerformed);
        assert_eq!(absence.attempt(), Attempt::Performed);
        assert_eq!(hole.influence_class(), InfluenceClass::Unknown);
        assert_eq!(absence.influence_class(), InfluenceClass::Zero);
    }

    #[test]
    fn a_hole_never_supports_a_sufficiency_claim_but_a_measured_absence_does() {
        assert!(!never_measured().supports_sufficiency());
        assert!(measured_absent().supports_sufficiency());
    }

    #[test]
    fn an_unrecorded_assay_history_is_unknown_not_not_performed() {
        let unrecorded = Missingness::NeverMeasured {
            reason: UnattemptedReason::Unrecorded,
        };
        let not_ordered = Missingness::NeverMeasured {
            reason: UnattemptedReason::NotOrdered,
        };
        assert_eq!(unrecorded.attempt(), Attempt::Unknown);
        assert_eq!(not_ordered.attempt(), Attempt::NotPerformed);
    }

    #[test]
    fn a_withheld_value_does_not_disclose_whether_it_was_measured() {
        let withheld = Missingness::PolicyWithheld {
            authority: "consent-tier-3".into(),
        };
        assert_eq!(withheld.attempt(), Attempt::Unknown);
        assert!(!withheld.is_measured_result());
        assert_eq!(
            withheld.influence_class(),
            InfluenceClass::InaccessibleByPolicy
        );
    }

    #[test]
    fn below_detection_limit_is_not_biological_absence() {
        let bdl = Missingness::BelowDetectionLimit {
            assay: "MS-shotgun".into(),
            limit: 0.01,
        };
        let absent = Missingness::BiologicalAbsence {
            assay: "MS-shotgun".into(),
        };
        assert_ne!(bdl.class(), absent.class());
        assert_eq!(bdl.influence_class(), InfluenceClass::Bounded);
        assert_eq!(absent.influence_class(), InfluenceClass::Zero);
    }

    #[test]
    fn a_failed_assay_is_deferred_not_a_negative_result() {
        let failed = Missingness::TechnicalFailure {
            assay: "RNAseq".into(),
            detail: "library prep failed QC".into(),
        };
        assert!(!failed.is_measured_result());
        assert_eq!(
            failed.influence_class(),
            InfluenceClass::DeferredAcquisition
        );
        assert!(!failed.influence_class().supports_sufficiency());
    }

    #[test]
    fn a_non_finite_value_is_not_a_measurement() {
        let err = Measured::new(f64::NAN, "ng/mL", "ELISA", "CEA").unwrap_err();
        assert!(matches!(err, LensError::NonFiniteMeasurement { .. }));
        assert!(Measured::new(f64::INFINITY, "ng/mL", "ELISA", "CEA").is_err());
        assert!(Measured::new(0.0, "ng/mL", "ELISA", "CEA").is_ok());
    }

    #[test]
    fn a_measurement_cannot_be_deserialized_without_a_numeric_value() {
        let json = r#"{"value":null,"unit":"ng/mL","assay":"ELISA"}"#;
        assert!(serde_json::from_str::<Measured>(json).is_err());
        let round_trip =
            serde_json::to_string(&Measured::new(3.0, "ng/mL", "ELISA", "CEA").unwrap())
                .and_then(|s| serde_json::from_str::<Measured>(&s));
        assert_eq!(round_trip.unwrap().value(), 3.0);
    }

    #[test]
    fn the_summary_counts_holes_and_measured_absences_separately() {
        let mut summary = MissingnessSummary::new();
        summary.observe(&measured_absent());
        summary.observe(&measured_absent());
        summary.observe(&never_measured());
        summary.observe(&Observation::Present(
            Measured::new(3.0, "ng/mL", "ELISA", "CEA").unwrap(),
        ));

        assert_eq!(summary.observed(), 1);
        assert_eq!(summary.measured_absences(), 2);
        assert_eq!(summary.holes(), 1);
        assert!(!summary.supports_sufficiency_claim());
    }

    #[test]
    fn a_set_of_only_measured_absences_supports_sufficiency() {
        let mut summary = MissingnessSummary::new();
        summary.observe(&measured_absent());
        summary.observe(&Observation::Absent(Missingness::Censored {
            assay: "survival".into(),
            direction: CensorDirection::Right,
            bound: 60.0,
        }));
        assert!(summary.supports_sufficiency_claim());
    }

    #[test]
    fn an_absence_serializes_with_its_class_and_no_value_field() {
        let json = serde_json::to_string(&never_measured()).unwrap();
        assert!(json.contains("never_measured"));
        assert!(!json.contains("\"value\""));
    }
}
