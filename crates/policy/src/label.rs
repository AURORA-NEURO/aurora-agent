//! The information-flow label lattice.
//!
//! Blueprint 43.33 ("define an information-flow order `⊑_P` where outputs may move only to equal
//! or more restrictive labels"), 36.01 (biological data classification) and 13.16 ("derived
//! artifacts inherit the maximum sensitivity of sources unless an approved transformation proves
//! declassification").
//!
//! A [`PolicyLabel`] is a point in a product lattice with seven axes. Six of them are ordinary —
//! more restrictive is larger — and three of those run *backwards* relative to the intuition that
//! joining adds things: permitted purposes, permitted jurisdictions and retention duration all get
//! smaller as the label gets more restrictive, so their join is an intersection or a minimum. Each
//! axis states its direction where it is defined, because getting one of them backwards is a
//! silent privacy failure rather than a compile error.
//!
//! The property that matters is the one the classic information-flow example tests: two inputs
//! that are individually innocuous can join to something neither of them was. Two consents each
//! permitting a wide set of purposes join to their intersection; two datasets each resident in a
//! permissive region join to the intersection of those regions, which may be empty. The join is
//! computed, never assumed, and never approximated by "take the more sensitive of the two".
//!
//! What this is not: [`PolicyLabel`] does not implement `PartialOrd`. The order is genuinely
//! partial, and `a <= b` returning `false` would conflate "b is more restrictive" with "these are
//! incomparable" at the exact point where the distinction decides whether evidence may move. The
//! direction is spelled out in [`PolicyLabel::flows_to`] instead.
//!
//! Also not here: encryption, key management, tokenisation, or any binding of a label to bytes at
//! rest. A label is an assertion about an artifact; enforcing it against a storage layer that does
//! not consult this crate is outside its reach.

use crate::purpose::{Purpose, PurposeSet};
use crate::residency::Residency;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// Sensitivity classes from 36.01.
///
/// 36.01 lists these as an unordered set. The total order below is this crate's choice, and it is
/// the only linearisation imposed anywhere here. It is defensible for the identifiability axis —
/// a controlled genomic record is harder to release than an institutional memo — and frankly
/// arbitrary between [`Classification::CommercialUnpublished`] and its neighbours, which are
/// restricted for a different reason rather than a stronger one.
///
/// Where two concerns are genuinely orthogonal, use a compartment instead: compartments form a
/// true set lattice and force no comparison. Dual-use and paediatric sensitivity appear here as
/// levels *and* are conventionally carried as compartments, so a world that disagrees with this
/// ordering can express the constraint without depending on it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    #[default]
    PublicAggregate,
    PublicIndividualNonIdentifying,
    InstitutionalConfidential,
    CommercialUnpublished,
    ControlledGenomicOrImaging,
    PediatricOrRareSensitive,
    RestrictedDualUse,
}

impl Classification {
    pub const ALL: [Classification; 7] = [
        Classification::PublicAggregate,
        Classification::PublicIndividualNonIdentifying,
        Classification::InstitutionalConfidential,
        Classification::CommercialUnpublished,
        Classification::ControlledGenomicOrImaging,
        Classification::PediatricOrRareSensitive,
        Classification::RestrictedDualUse,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Classification::PublicAggregate => "public_aggregate",
            Classification::PublicIndividualNonIdentifying => "public_individual_non_identifying",
            Classification::InstitutionalConfidential => "institutional_confidential",
            Classification::CommercialUnpublished => "commercial_unpublished",
            Classification::ControlledGenomicOrImaging => "controlled_genomic_or_imaging",
            Classification::PediatricOrRareSensitive => "pediatric_or_rare_sensitive",
            Classification::RestrictedDualUse => "restricted_dual_use",
        }
    }
}

impl fmt::Display for Classification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How far an artifact may travel.
///
/// Ordered so that larger is more restrictive, matching the join direction of every other axis.
/// [`ExportPolicy::NoExport`] is the value 43.33 mandates for an unknown policy.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ExportPolicy {
    #[default]
    Unrestricted,
    /// Approved aggregates may leave; the sections they were computed from may not.
    AggregatesOnly,
    NoExport,
}

impl ExportPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            ExportPolicy::Unrestricted => "unrestricted",
            ExportPolicy::AggregatesOnly => "aggregates_only",
            ExportPolicy::NoExport => "no_export",
        }
    }
}

impl fmt::Display for ExportPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How long an artifact may be kept.
///
/// The restrictive direction is *shorter*, so the join takes the minimum. 36.18 requires that a
/// deletion obligation reach caches and derived artifacts; representing retention as a lattice
/// axis is how that propagation happens without a separate traversal — a derived artifact's label
/// already carries the shortest window of anything that went into it.
///
/// Not modelled: legal hold, which suspends deletion and therefore is not a point on this axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Retention {
    #[default]
    Indefinite,
    Days(u32),
}

impl Retention {
    /// Rank in the "how long may this be kept" order. Larger is longer, hence less restrictive.
    fn rank(self) -> u64 {
        match self {
            Retention::Indefinite => u64::MAX,
            Retention::Days(days) => u64::from(days),
        }
    }

    pub fn days(self) -> Option<u32> {
        match self {
            Retention::Indefinite => None,
            Retention::Days(days) => Some(days),
        }
    }

    fn shorter(self, other: Retention) -> Retention {
        if self.rank() <= other.rank() {
            self
        } else {
            other
        }
    }

    fn longer(self, other: Retention) -> Retention {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }
}

impl fmt::Display for Retention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Retention::Indefinite => f.write_str("indefinite"),
            Retention::Days(days) => write!(f, "{days}d"),
        }
    }
}

/// The policy label attached to a scope, a fact, or a derived artifact.
///
/// Axis directions, all stated as "the join does this":
///
/// | axis | join | why that direction |
/// |---|---|---|
/// | `classification` | maximum | 13.16: derived artifacts inherit the maximum sensitivity |
/// | `compartments` | union | a derived artifact belongs to every compartment it touched |
/// | `purposes` | intersection | only purposes *every* source permits survive |
/// | `residency` | intersection | only sites *every* source permits survive |
/// | `export` | maximum | the strictest export limit wins |
/// | `retention` | shorter | the earliest deletion obligation wins |
/// | `min_cell_size` | maximum | the largest small-cell threshold wins |
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyLabel {
    pub classification: Classification,
    /// Orthogonal need-to-know groups: `pediatric`, `dual_use`, `site_confidential`, and whatever
    /// else the governance registry of 36.15 has minted. Deliberately open strings: this crate
    /// does not own the vocabulary, and an unrecognised compartment must still be carried and
    /// still block a principal who does not hold it.
    #[serde(default)]
    pub compartments: BTreeSet<String>,
    pub purposes: PurposeSet,
    pub residency: Residency,
    pub export: ExportPolicy,
    pub retention: Retention,
    /// Aggregate cells with fewer members than this must not be released as counts (36.02, 36.03).
    #[serde(default)]
    pub min_cell_size: u32,
}

impl Default for PolicyLabel {
    fn default() -> Self {
        PolicyLabel::public()
    }
}

impl PolicyLabel {
    /// The bottom of the lattice: a published aggregate with no constraint at all.
    ///
    /// It is the identity of [`PolicyLabel::join`], which is why it is the correct starting value
    /// for folding a derivation's inputs.
    pub fn public() -> Self {
        PolicyLabel {
            classification: Classification::PublicAggregate,
            compartments: BTreeSet::new(),
            purposes: PurposeSet::Any,
            residency: Residency::Anywhere,
            export: ExportPolicy::Unrestricted,
            retention: Retention::Indefinite,
            min_cell_size: 0,
        }
    }

    /// The label of evidence no rule claims.
    ///
    /// 43.33: "Unknown policy defaults to no export." This goes further and permits no purpose and
    /// no site either, so an unlabelled artifact is not merely unexportable but unusable until
    /// somebody classifies it. That is the intended friction.
    ///
    /// This is *not* the top of the lattice — the compartment axis is an unbounded set lattice and
    /// has no greatest element — which is why it cannot be modelled as a rule that always applies.
    /// See [`crate::lattice::PolicyLattice`] for how the unlabelled case is kept separate from the
    /// labelled one.
    pub fn unknown() -> Self {
        PolicyLabel {
            classification: Classification::RestrictedDualUse,
            compartments: BTreeSet::new(),
            purposes: PurposeSet::none(),
            residency: Residency::nowhere(),
            export: ExportPolicy::NoExport,
            retention: Retention::Days(0),
            min_cell_size: u32::MAX,
        }
    }

    pub fn with_classification(mut self, classification: Classification) -> Self {
        self.classification = classification;
        self
    }

    pub fn with_compartment(mut self, compartment: impl Into<String>) -> Self {
        self.compartments.insert(compartment.into());
        self
    }

    pub fn with_purposes(mut self, purposes: PurposeSet) -> Self {
        self.purposes = purposes;
        self
    }

    pub fn with_residency(mut self, residency: Residency) -> Self {
        self.residency = residency;
        self
    }

    pub fn with_export(mut self, export: ExportPolicy) -> Self {
        self.export = export;
        self
    }

    pub fn with_retention(mut self, retention: Retention) -> Self {
        self.retention = retention;
        self
    }

    pub fn with_min_cell_size(mut self, min_cell_size: u32) -> Self {
        self.min_cell_size = min_cell_size;
        self
    }

    /// The least upper bound: the label a derived artifact must carry.
    pub fn join(&self, other: &PolicyLabel) -> PolicyLabel {
        PolicyLabel {
            classification: self.classification.max(other.classification),
            compartments: self
                .compartments
                .union(&other.compartments)
                .cloned()
                .collect(),
            purposes: self.purposes.intersect(&other.purposes),
            residency: self.residency.intersect(&other.residency),
            export: self.export.max(other.export),
            retention: self.retention.shorter(other.retention),
            min_cell_size: self.min_cell_size.max(other.min_cell_size),
        }
    }

    /// The greatest lower bound.
    ///
    /// No safe operation on live evidence produces this: taking the meet of two sources' labels
    /// would grant permissions neither source gave. It exists so the lattice laws are stateable
    /// and testable, and so a declassification target can be checked against a floor.
    pub fn meet(&self, other: &PolicyLabel) -> PolicyLabel {
        PolicyLabel {
            classification: self.classification.min(other.classification),
            compartments: self
                .compartments
                .intersection(&other.compartments)
                .cloned()
                .collect(),
            purposes: self.purposes.union(&other.purposes),
            residency: self.residency.union(&other.residency),
            export: self.export.min(other.export),
            retention: self.retention.longer(other.retention),
            min_cell_size: self.min_cell_size.min(other.min_cell_size),
        }
    }

    /// True when information labelled `self` may legally become information labelled `other`;
    /// that is, when `other` is at least as restrictive on every axis.
    ///
    /// The name states the direction because `<=` does not. `a.flows_to(b) == false` does not mean
    /// `b.flows_to(a)`: incomparable labels flow in neither direction, and that case is common
    /// once compartments and jurisdictions are in play.
    pub fn flows_to(&self, other: &PolicyLabel) -> bool {
        self.classification <= other.classification
            && self.compartments.is_subset(&other.compartments)
            && other.purposes.is_subset_of(&self.purposes)
            && other.residency.is_subset_of(&self.residency)
            && self.export <= other.export
            && other.retention.rank() <= self.retention.rank()
            && self.min_cell_size <= other.min_cell_size
    }

    /// Folds a derivation's inputs. An artifact with no inputs is a constant, hence public.
    pub fn join_all<'a, I: IntoIterator<Item = &'a PolicyLabel>>(labels: I) -> PolicyLabel {
        labels
            .into_iter()
            .fold(PolicyLabel::public(), |accumulated, label| {
                accumulated.join(label)
            })
    }

    pub fn admits_purpose(&self, purpose: Purpose) -> bool {
        self.purposes.admits(purpose)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::residency::Jurisdiction;

    fn eu_research() -> PolicyLabel {
        PolicyLabel::public()
            .with_classification(Classification::InstitutionalConfidential)
            .with_purposes(PurposeSet::of([
                Purpose::ResearchAnalysis,
                Purpose::MethodDevelopment,
            ]))
            .with_residency(Residency::only(["eu", "uk"]))
    }

    fn us_training() -> PolicyLabel {
        PolicyLabel::public()
            .with_classification(Classification::InstitutionalConfidential)
            .with_purposes(PurposeSet::of([
                Purpose::ResearchAnalysis,
                Purpose::ModelTraining,
            ]))
            .with_residency(Residency::only(["us", "uk"]))
    }

    #[test]
    fn combining_two_low_inputs_yields_their_join_not_the_lower_label() {
        let joined = eu_research().join(&us_training());

        assert_eq!(
            joined.purposes,
            PurposeSet::of([Purpose::ResearchAnalysis]),
            "the join keeps only what both sources permitted"
        );
        assert_eq!(joined.residency, Residency::only(["uk"]));
        assert_ne!(joined, eu_research());
        assert_ne!(joined, us_training());
        assert!(!joined.flows_to(&eu_research()));
        assert!(!joined.flows_to(&us_training()));
    }

    #[test]
    fn two_public_looking_inputs_can_join_to_a_label_no_site_may_hold() {
        let eu = PolicyLabel::public().with_residency(Residency::only(["eu"]));
        let us = PolicyLabel::public().with_residency(Residency::only(["us"]));

        let joined = eu.join(&us);

        assert!(joined.residency.is_nowhere());
        assert!(!joined.residency.admits(&Jurisdiction::new("eu")));
    }

    #[test]
    fn the_join_is_an_upper_bound_of_both_inputs() {
        let joined = eu_research().join(&us_training());
        assert!(eu_research().flows_to(&joined));
        assert!(us_training().flows_to(&joined));
    }

    #[test]
    fn the_join_is_the_least_of_the_upper_bounds() {
        let left = eu_research();
        let right = us_training();
        let joined = left.join(&right);

        let looser_candidate = joined
            .clone()
            .with_classification(Classification::RestrictedDualUse);
        assert!(left.flows_to(&looser_candidate));
        assert!(right.flows_to(&looser_candidate));
        assert!(
            joined.flows_to(&looser_candidate),
            "any upper bound must lie above the join"
        );
        assert!(!looser_candidate.flows_to(&joined));
    }

    #[test]
    fn the_join_is_idempotent_commutative_and_associative() {
        let a = eu_research();
        let b = us_training();
        let c = PolicyLabel::public()
            .with_compartment("pediatric")
            .with_retention(Retention::Days(30));

        assert_eq!(a.join(&a), a);
        assert_eq!(a.join(&b), b.join(&a));
        assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
    }

    #[test]
    fn public_is_the_identity_of_the_join_so_folding_inputs_starts_there() {
        let label = eu_research();
        assert_eq!(PolicyLabel::public().join(&label), label);
        assert_eq!(
            PolicyLabel::join_all([] as [&PolicyLabel; 0]),
            PolicyLabel::public()
        );
    }

    #[test]
    fn absorption_holds_between_join_and_meet() {
        let a = eu_research();
        let b = us_training();
        assert_eq!(a.join(&a.meet(&b)), a);
        assert_eq!(a.meet(&a.join(&b)), a);
    }

    #[test]
    fn a_derived_artifact_inherits_the_shortest_retention_of_its_inputs() {
        let long = PolicyLabel::public().with_retention(Retention::Indefinite);
        let medium = PolicyLabel::public().with_retention(Retention::Days(365));
        let short = PolicyLabel::public().with_retention(Retention::Days(30));

        let joined = PolicyLabel::join_all([&long, &medium, &short]);

        assert_eq!(joined.retention, Retention::Days(30));
    }

    #[test]
    fn a_derived_artifact_belongs_to_every_compartment_it_touched() {
        let peds = PolicyLabel::public().with_compartment("pediatric");
        let dual = PolicyLabel::public().with_compartment("dual_use");

        let joined = peds.join(&dual);

        assert!(joined.compartments.contains("pediatric"));
        assert!(joined.compartments.contains("dual_use"));
    }

    #[test]
    fn the_strictest_small_cell_threshold_survives_the_join() {
        let loose = PolicyLabel::public().with_min_cell_size(5);
        let strict = PolicyLabel::public().with_min_cell_size(20);
        assert_eq!(loose.join(&strict).min_cell_size, 20);
    }

    #[test]
    fn incomparable_labels_flow_in_neither_direction() {
        let a = PolicyLabel::public().with_compartment("pediatric");
        let b = PolicyLabel::public().with_compartment("dual_use");
        assert!(!a.flows_to(&b));
        assert!(!b.flows_to(&a));
    }

    #[test]
    fn flows_to_is_reflexive_and_transitive() {
        let a = PolicyLabel::public();
        let b = eu_research();
        let c = eu_research().join(&us_training());

        assert!(a.flows_to(&a));
        assert!(a.flows_to(&b));
        assert!(b.flows_to(&c));
        assert!(a.flows_to(&c));
    }

    #[test]
    fn an_unlabelled_artifact_defaults_to_no_export_and_no_purpose() {
        let unknown = PolicyLabel::unknown();
        assert_eq!(unknown.export, ExportPolicy::NoExport);
        assert!(unknown.purposes.is_empty());
        assert!(unknown.residency.is_nowhere());
        for purpose in Purpose::ALL {
            assert!(!unknown.admits_purpose(purpose));
        }
    }

    #[test]
    fn a_label_round_trips_through_json() {
        let label = eu_research()
            .with_compartment("pediatric")
            .with_retention(Retention::Days(90))
            .with_min_cell_size(11);
        let text = serde_json::to_string(&label).unwrap();
        let parsed: PolicyLabel = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed, label);
    }
}
