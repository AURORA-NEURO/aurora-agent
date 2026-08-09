//! Purpose binding.
//!
//! Blueprint 36.04 (consent, data use and purpose limitation) and step 1 of 43.33's compiler
//! procedure: "Bind user/agent role and purpose to the query."
//!
//! Purpose creep is the failure mode this module is shaped against. A cohort collected for one
//! study is quietly reused for another because the second purpose was never written down, so no
//! check could fail. Two choices make that hard to express by accident:
//!
//! * A request binds exactly one [`Purpose`], and [`Purpose`] is a closed enumeration. There is
//!   no free-text purpose and no `Any` variant on the requesting side, so "general research" is
//!   not sayable. A purpose the platform does not know about cannot be silently accepted.
//! * There is no implication order between purposes. [`Purpose::ResearchAnalysis`] does not
//!   entail [`Purpose::ModelTraining`], and [`Purpose::MethodDevelopment`] does not entail
//!   [`Purpose::BenchmarkPublication`]. A consent that permits one and is silent about the other
//!   refuses the other. Widening requires editing the consent, which is a governance act with an
//!   audit trail, not an inference the compiler is allowed to make.
//!
//! Not implemented here: the governance workflow that grants a new purpose (36.15), the appeal
//! path of 36.04, or any notion of who is entitled to edit a consent. This crate consumes
//! consents; it does not administer them.

use crate::error::PolicyError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// A declared reason for touching evidence.
///
/// The variants are the operational purposes 36.04 distinguishes. They are deliberately coarse:
/// a finer taxonomy invites the reader to treat one purpose as a special case of another, which
/// is the entry point for creep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    /// Answering a scientific question about the cohort the evidence describes.
    ResearchAnalysis,
    /// Building or debugging an analysis method, where the data is a test input.
    MethodDevelopment,
    /// Emitting a public benchmark, leaderboard entry or paper artifact.
    BenchmarkPublication,
    /// Fitting model weights. Distinct from analysis because the data survives in the artifact.
    ModelTraining,
    /// Work whose output has commercial value to a party other than the data steward.
    CommercialDevelopment,
    /// Patient-specific direction. 36.09 keeps this separate from research for a reason: the
    /// evidence bar and the liability differ, and a research consent never implies it.
    ClinicalDecisionSupport,
    /// Verifying that a pipeline behaved, without drawing conclusions about the subjects.
    QualityAssurance,
    /// Investigating a suspected policy violation or incident (36.20).
    SecurityAudit,
}

impl Purpose {
    pub const ALL: [Purpose; 8] = [
        Purpose::ResearchAnalysis,
        Purpose::MethodDevelopment,
        Purpose::BenchmarkPublication,
        Purpose::ModelTraining,
        Purpose::CommercialDevelopment,
        Purpose::ClinicalDecisionSupport,
        Purpose::QualityAssurance,
        Purpose::SecurityAudit,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Purpose::ResearchAnalysis => "research_analysis",
            Purpose::MethodDevelopment => "method_development",
            Purpose::BenchmarkPublication => "benchmark_publication",
            Purpose::ModelTraining => "model_training",
            Purpose::CommercialDevelopment => "commercial_development",
            Purpose::ClinicalDecisionSupport => "clinical_decision_support",
            Purpose::QualityAssurance => "quality_assurance",
            Purpose::SecurityAudit => "security_audit",
        }
    }

    /// Parses a wire name.
    ///
    /// An unrecognised name is an error rather than a new purpose. A world that could mint
    /// purposes by spelling them differently would have no purpose limitation at all.
    pub fn parse(text: &str) -> Result<Purpose, PolicyError> {
        Purpose::ALL
            .into_iter()
            .find(|purpose| purpose.as_str() == text)
            .ok_or_else(|| PolicyError::UnknownPurpose(text.to_string()))
    }
}

impl fmt::Display for Purpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The purposes a label or consent permits.
///
/// [`PurposeSet::Any`] exists only on the *permitting* side, for genuinely open artifacts such as
/// a published aggregate. It is the identity of [`PurposeSet::intersect`], which is what makes it
/// the bottom of this axis: joining it with anything leaves the other set untouched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PurposeSet {
    Any,
    Only(BTreeSet<Purpose>),
}

impl PurposeSet {
    /// The empty set: permits nothing. The deny-by-default value of 43.33.
    pub fn none() -> Self {
        PurposeSet::Only(BTreeSet::new())
    }

    pub fn of<I: IntoIterator<Item = Purpose>>(purposes: I) -> Self {
        PurposeSet::Only(purposes.into_iter().collect())
    }

    pub fn admits(&self, purpose: Purpose) -> bool {
        match self {
            PurposeSet::Any => true,
            PurposeSet::Only(set) => set.contains(&purpose),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, PurposeSet::Only(set) if set.is_empty())
    }

    /// The purposes both sets permit. This is the join direction: combining two sources may only
    /// narrow what the result may be used for.
    pub fn intersect(&self, other: &PurposeSet) -> PurposeSet {
        match (self, other) {
            (PurposeSet::Any, rest) | (rest, PurposeSet::Any) => rest.clone(),
            (PurposeSet::Only(a), PurposeSet::Only(b)) => {
                PurposeSet::Only(a.intersection(b).copied().collect())
            }
        }
    }

    /// The purposes either set permits. Only used by the meet, which is not a safe operation on
    /// live evidence and exists so the lattice laws can be stated and tested.
    pub fn union(&self, other: &PurposeSet) -> PurposeSet {
        match (self, other) {
            (PurposeSet::Any, _) | (_, PurposeSet::Any) => PurposeSet::Any,
            (PurposeSet::Only(a), PurposeSet::Only(b)) => {
                PurposeSet::Only(a.union(b).copied().collect())
            }
        }
    }

    /// True when every purpose `self` permits is also permitted by `wider`.
    pub fn is_subset_of(&self, wider: &PurposeSet) -> bool {
        match (self, wider) {
            (_, PurposeSet::Any) => true,
            (PurposeSet::Any, PurposeSet::Only(_)) => false,
            (PurposeSet::Only(a), PurposeSet::Only(b)) => a.is_subset(b),
        }
    }
}

impl fmt::Display for PurposeSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PurposeSet::Any => f.write_str("any"),
            PurposeSet::Only(set) => {
                let names: Vec<&str> = set.iter().map(|p| p.as_str()).collect();
                write!(f, "only[{}]", names.join(","))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broad_sounding_purpose_does_not_entail_a_narrower_one() {
        let consented = PurposeSet::of([Purpose::ResearchAnalysis]);
        assert!(consented.admits(Purpose::ResearchAnalysis));
        assert!(!consented.admits(Purpose::ModelTraining));
        assert!(!consented.admits(Purpose::BenchmarkPublication));
        assert!(!consented.admits(Purpose::ClinicalDecisionSupport));
    }

    #[test]
    fn a_purpose_name_that_merely_resembles_a_known_one_fails_to_parse() {
        assert!(Purpose::parse("research_analysis").is_ok());
        for impostor in ["research", "Research_Analysis", "research analysis", "research_analysis "] {
            assert!(
                Purpose::parse(impostor).is_err(),
                "{impostor:?} must not mint a purpose"
            );
        }
    }

    #[test]
    fn every_purpose_round_trips_through_its_wire_name() {
        for purpose in Purpose::ALL {
            assert_eq!(Purpose::parse(purpose.as_str()).unwrap(), purpose);
        }
    }

    #[test]
    fn intersecting_two_overlapping_consents_keeps_only_the_common_purpose() {
        let left = PurposeSet::of([Purpose::ResearchAnalysis, Purpose::MethodDevelopment]);
        let right = PurposeSet::of([Purpose::ResearchAnalysis, Purpose::QualityAssurance]);
        assert_eq!(
            left.intersect(&right),
            PurposeSet::of([Purpose::ResearchAnalysis])
        );
    }

    #[test]
    fn any_is_the_identity_of_intersection_so_public_evidence_never_widens_a_consent() {
        let bound = PurposeSet::of([Purpose::ResearchAnalysis]);
        assert_eq!(PurposeSet::Any.intersect(&bound), bound);
        assert_eq!(bound.intersect(&PurposeSet::Any), bound);
    }
}
