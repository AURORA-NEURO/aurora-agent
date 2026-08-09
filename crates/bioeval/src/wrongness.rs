//! The structure of being wrong (26.02).
//!
//! Software evaluation can afford `correct | incorrect` because a failing assertion is a failing
//! assertion. Biology cannot. 26.02 lists five failure modes that a boolean flattens into one
//! cell — "right number from wrong cohort", "correct association claimed as mechanism", "correct
//! mechanism in wrong subtype", "technically valid analysis on swapped specimens", "accurate
//! prose with nonexistent evidence" — and the entire diagnostic value of an evaluation lives in
//! telling them apart. A run that reports 0.62 accuracy has told you nothing about whether the
//! pipeline mixed up two tubes or reasoned badly from the right tube.
//!
//! So the unit of failure here is a *class*, and the class carries a [`Severity`] that governs
//! whether partial credit is even available. Two properties do the work:
//!
//! * [`BiologicalErrorClass::Unclassified`] exists and is never benign. Nobody having looked at
//!   a failure is a fact about the evaluation, not a property of the prediction, and it is
//!   modelled the way [`bioprism_section::InfluenceClass::Unknown`] is: representable, and
//!   disqualifying.
//! * Severity is a property of the class, not a caller-supplied weight. A grader cannot decide
//!   that this particular laterality error was minor, because the leaderboard incentive is
//!   always to decide exactly that.
//!
//! # Not implemented
//!
//! There is no severity *escalation* by context here: 26.02 wants critical failures propagated
//! to downstream claims, and this module supplies only the per-error classification that such a
//! propagation would consume. The propagation itself lives in [`crate::layer`].

use std::fmt;

use serde::{Deserialize, Serialize};

/// How much a failure of this class costs the conclusion.
///
/// Ordered so that `Benign < Material < Critical`, which lets a panel take the worst class
/// present rather than the average — see [`crate::panel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The conclusion survives. A rounding difference, a synonymous identifier.
    Benign,
    /// The conclusion is damaged but a defensible remainder exists — a right-direction estimate
    /// of the wrong size still tells you the sign.
    Material,
    /// Nothing downstream of this is usable. 26.02: partial credit is retained "only when the
    /// remaining conclusion is meaningful", and here it is not.
    Critical,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Benign => "benign",
            Severity::Material => "material",
            Severity::Critical => "critical",
        }
    }

    /// Whether a failure at this severity may still earn partial credit.
    pub fn admits_partial_credit(self) -> bool {
        matches!(self, Severity::Benign | Severity::Material)
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What kind of wrong a prediction is.
///
/// The list is deliberately closed. An open taxonomy of error strings degenerates into free text
/// within one benchmark cycle, and free text cannot carry a severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BiologicalErrorClass {
    /// Called the wrong molecular subtype. The mechanism, the therapy and the prognosis all
    /// change; nothing downstream survives.
    MolecularSubtype,
    /// Left reported as right, ipsilateral as contralateral. Cheap to make, and in a surgical
    /// or radiotherapy context it is the error that reaches a patient.
    Laterality,
    /// The evidence belongs to a different specimen, block, aliquot or subject. 26.02's
    /// "technically valid analysis on swapped specimens": the arithmetic is flawless and the
    /// answer is about somebody else.
    SpecimenIdentity,
    /// The right quantity computed over the wrong population. 26.02's "right number from wrong
    /// cohort".
    CohortMismatch,
    /// A correlation reported as a causal mechanism. The finding may be entirely real and the
    /// claim still unsupported.
    AssociationAsMechanism,
    /// The claim is true somewhere, but not in the scope where it was asserted — right mechanism,
    /// wrong subtype or wrong tissue.
    ScopeViolation,
    /// Milligrams for micrograms, TPM for counts, Gy for cGy. Recoverable in principle and
    /// catastrophic in practice, and distinguished from a magnitude error because the fix is
    /// mechanical.
    Units,
    /// Coordinates read in the wrong frame or reference build — RAS against LPS, GRCh37 against
    /// GRCh38. The numbers are internally consistent and point somewhere else.
    CoordinateFrame,
    /// Right direction, wrong size. The only class here whose remainder — the sign — is often
    /// the thing the decision actually needed.
    MagnitudeRightDirection,
    /// Right size, wrong sign. Kept separate from [`Self::MagnitudeRightDirection`] because an
    /// inverted effect is worse than an imprecise one, not merely different.
    DirectionReversed,
    /// The prose is accurate and the citation does not support it, or does not exist. 26.02's
    /// "accurate prose with nonexistent evidence".
    UnsupportedCitation,
    /// The answer used information that did not exist at the decision time.
    TemporalLeakage,
    /// Nobody classified this failure. Never benign, never creditable.
    Unclassified,
}

impl BiologicalErrorClass {
    pub const CANONICAL: [BiologicalErrorClass; 12] = [
        BiologicalErrorClass::MolecularSubtype,
        BiologicalErrorClass::Laterality,
        BiologicalErrorClass::SpecimenIdentity,
        BiologicalErrorClass::CohortMismatch,
        BiologicalErrorClass::AssociationAsMechanism,
        BiologicalErrorClass::ScopeViolation,
        BiologicalErrorClass::Units,
        BiologicalErrorClass::CoordinateFrame,
        BiologicalErrorClass::MagnitudeRightDirection,
        BiologicalErrorClass::DirectionReversed,
        BiologicalErrorClass::UnsupportedCitation,
        BiologicalErrorClass::TemporalLeakage,
    ];

    /// The severity of this class, fixed by the taxonomy rather than by the grader.
    ///
    /// [`Self::Unclassified`] reports [`Severity::Critical`]. This is the conservative reading
    /// and it is intentional: an unexamined failure must not be cheaper than an examined one,
    /// or the incentive is to stop examining.
    pub fn severity(self) -> Severity {
        match self {
            BiologicalErrorClass::MolecularSubtype
            | BiologicalErrorClass::Laterality
            | BiologicalErrorClass::SpecimenIdentity
            | BiologicalErrorClass::CohortMismatch
            | BiologicalErrorClass::TemporalLeakage
            | BiologicalErrorClass::Unclassified => Severity::Critical,
            BiologicalErrorClass::AssociationAsMechanism
            | BiologicalErrorClass::ScopeViolation
            | BiologicalErrorClass::DirectionReversed
            | BiologicalErrorClass::CoordinateFrame
            | BiologicalErrorClass::UnsupportedCitation => Severity::Material,
            BiologicalErrorClass::Units | BiologicalErrorClass::MagnitudeRightDirection => {
                Severity::Benign
            }
        }
    }

    /// Whether the failure is mechanically repairable given the same underlying work.
    ///
    /// A units error is a multiplication away from correct; a wrong cohort is not. This is the
    /// axis that separates [`Self::Units`] from [`Self::CohortMismatch`] even though both can
    /// produce the same wrong number, and it is what a triage queue should sort on.
    pub fn is_mechanically_repairable(self) -> bool {
        matches!(
            self,
            BiologicalErrorClass::Units | BiologicalErrorClass::CoordinateFrame
        )
    }

    /// Whether a failure of this class can reach a patient or a specimen without passing another
    /// check first.
    ///
    /// Deliberately orthogonal to [`Self::severity`], and the orthogonality is the interesting
    /// part. [`Self::Units`] is [`Severity::Benign`] — the conclusion survives a unit slip, you
    /// multiply by a thousand and carry on — and it is safety-reaching, because a thousandfold
    /// dose is what leaves the building. Severity asks whether the remaining conclusion is
    /// meaningful; this asks whether being wrong is survivable. Collapsing them into one number
    /// loses one of the two questions.
    ///
    /// Used by [`crate::aggregate`] to give a lone dissenter a veto: 26.20 requires "task-specific
    /// vetoes", and a single reader calling laterality is the canonical case where the minority
    /// must not be averaged away.
    pub fn is_safety_reaching(self) -> bool {
        matches!(
            self,
            BiologicalErrorClass::Laterality
                | BiologicalErrorClass::SpecimenIdentity
                | BiologicalErrorClass::MolecularSubtype
                | BiologicalErrorClass::Units
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            BiologicalErrorClass::MolecularSubtype => "molecular_subtype",
            BiologicalErrorClass::Laterality => "laterality",
            BiologicalErrorClass::SpecimenIdentity => "specimen_identity",
            BiologicalErrorClass::CohortMismatch => "cohort_mismatch",
            BiologicalErrorClass::AssociationAsMechanism => "association_as_mechanism",
            BiologicalErrorClass::ScopeViolation => "scope_violation",
            BiologicalErrorClass::Units => "units",
            BiologicalErrorClass::CoordinateFrame => "coordinate_frame",
            BiologicalErrorClass::MagnitudeRightDirection => "magnitude_right_direction",
            BiologicalErrorClass::DirectionReversed => "direction_reversed",
            BiologicalErrorClass::UnsupportedCitation => "unsupported_citation",
            BiologicalErrorClass::TemporalLeakage => "temporal_leakage",
            BiologicalErrorClass::Unclassified => "unclassified",
        }
    }
}

impl fmt::Display for BiologicalErrorClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
