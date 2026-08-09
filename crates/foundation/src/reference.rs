//! Reference-standard distributions.
//!
//! Blueprint 24.11 opens by naming its target: "the fiction of a universal single ground-truth
//! label". The fiction is convenient — it makes accuracy computable — and it is why a system
//! that gives a defensible minority reading scores identically to one that is simply wrong.
//!
//! [`ReferenceStandard::collapse_to_label`] is the enforcement point. Where the standard is a
//! distribution over disagreeing reviewers, it refuses, and the refusal reports how many raters
//! were about to be discarded. Where truth really is single-valued — a synthetic fixture, a
//! code-derived answer — it succeeds, because 24.11 is not against certainty, only against
//! manufactured certainty.
//!
//! [`ReferenceStandard::admits`] implements the set-valued acceptance 24.11 lists among its
//! scoring options, so that a system matching any attested reading is not marked wrong.
//!
//! **Deliberately absent: the scoring rules.** 24.11 lists seven acceptable scoring approaches
//! (expected utility across the distribution, proper scoring rules, distance to the expert
//! distribution, selective accuracy at chosen coverage, and so on) without specifying any of
//! them. Implementing a proper scoring rule here would mean choosing one and calling it the
//! reference behaviour. This module supplies the distribution and the refusals; the arithmetic
//! belongs to the oracle crates, where the choice of rule is a visible, per-benchmark decision.

use crate::error::ReferenceError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The eight things blueprint 24.11 says a reference standard can be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    /// Deterministic truth in a synthetic fixture.
    SyntheticGroundTruth,
    /// Derived by assay or by code, reproducibly.
    AssayOrCodeDerived,
    AdjudicatedConsensus,
    /// A distribution of independent expert ratings, not an average of them.
    IndependentExpertDistribution,
    LongitudinalOutcomeDefined,
    MolecularOrPathologyConfirmation,
    /// Several incompatible readings, each defensible.
    MultipleDefensibleInterpretations,
    /// Reviewed and found ungradable. A finding, not a gap.
    UnresolvedOrUngradable,
}

impl ReferenceKind {
    pub const ALL: [ReferenceKind; 8] = [
        ReferenceKind::SyntheticGroundTruth,
        ReferenceKind::AssayOrCodeDerived,
        ReferenceKind::AdjudicatedConsensus,
        ReferenceKind::IndependentExpertDistribution,
        ReferenceKind::LongitudinalOutcomeDefined,
        ReferenceKind::MolecularOrPathologyConfirmation,
        ReferenceKind::MultipleDefensibleInterpretations,
        ReferenceKind::UnresolvedOrUngradable,
    ];

    /// Whether this kind can, in principle, be reduced to one label.
    ///
    /// Adjudicated consensus qualifies: adjudication is the process of producing a single
    /// answer, and the disagreement it resolved is recorded elsewhere. An independent expert
    /// distribution does not, and neither do multiple defensible interpretations — reducing
    /// those is not summarizing, it is deleting the finding.
    pub fn is_single_valued(self) -> bool {
        matches!(
            self,
            ReferenceKind::SyntheticGroundTruth
                | ReferenceKind::AssayOrCodeDerived
                | ReferenceKind::AdjudicatedConsensus
                | ReferenceKind::LongitudinalOutcomeDefined
                | ReferenceKind::MolecularOrPathologyConfirmation
        )
    }
}

/// One reviewer's reading.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Rating {
    pub reviewer_role: String,
    pub label: String,
    /// The reviewer's own confidence, as recorded. Free text so that "moderate" survives when
    /// the source did not supply a number.
    pub confidence: String,
    /// What this reviewer could see. 24.11 requires it, because two experts disagreeing on the
    /// same evidence and two experts shown different evidence are different findings.
    pub evidence_visible: String,
    /// When the interpretation was made, which fixes the guideline era it belongs to.
    pub interpreted_at: String,
    #[serde(default)]
    pub declared_conflicts: Vec<String>,
}

/// A reference standard together with the metadata 24.11 requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceStandard {
    pub id: String,
    pub kind: ReferenceKind,
    pub ratings: Vec<Rating>,
    pub minimum_reviewers: usize,
    /// Whether reviewers worked independently or discussed, and how.
    pub independence_protocol: String,
    /// Ontology and guideline version in force. A label means different things across editions.
    pub ontology_version: String,
    /// How disagreement was resolved, or the statement that it was not.
    pub adjudication_procedure: String,
    /// Inter-rater statistics as reported. Free text: the statistic used varies by task and
    /// forcing a single number would repeat the collapse this module exists to prevent.
    pub inter_rater_statistics: String,
}

impl ReferenceStandard {
    /// Checks the metadata block of 24.11 and the reviewer floor.
    ///
    /// Synthetic and code-derived standards have no reviewers, so the floor applies only where
    /// reviewers are what the standard is made of.
    pub fn check(&self) -> Result<(), ReferenceError> {
        if self.ratings.is_empty() {
            return Err(ReferenceError::Empty {
                standard: self.id.clone(),
            });
        }
        let required: [(&'static str, &String); 4] = [
            ("independence or discussion protocol", &self.independence_protocol),
            ("ontology and guideline version", &self.ontology_version),
            ("adjudication procedure", &self.adjudication_procedure),
            ("inter-rater statistics", &self.inter_rater_statistics),
        ];
        for (field, value) in required {
            if value.trim().is_empty() {
                return Err(ReferenceError::MissingMetadata {
                    standard: self.id.clone(),
                    field,
                });
            }
        }
        for rating in &self.ratings {
            if rating.evidence_visible.trim().is_empty() {
                return Err(ReferenceError::MissingMetadata {
                    standard: self.id.clone(),
                    field: "the evidence visible to each reviewer",
                });
            }
        }
        if self.ratings.len() < self.minimum_reviewers {
            return Err(ReferenceError::TooFewReviewers {
                standard: self.id.clone(),
                required: self.minimum_reviewers,
                found: self.ratings.len(),
            });
        }
        Ok(())
    }

    /// The distribution over attested labels.
    pub fn distribution(&self) -> BTreeMap<&str, usize> {
        let mut counts = BTreeMap::new();
        for rating in &self.ratings {
            *counts.entry(rating.label.as_str()).or_insert(0) += 1;
        }
        counts
    }

    pub fn labels(&self) -> BTreeSet<&str> {
        self.ratings.iter().map(|r| r.label.as_str()).collect()
    }

    /// Whether reviewers who agreed on nothing are the finding.
    ///
    /// 24.11: "Cases with stable expert disagreement are not necessarily bad data." They are
    /// labelled rather than dropped, so that a benchmark can select them on purpose.
    pub fn is_disagreement(&self) -> bool {
        self.labels().len() > 1
            || self.kind == ReferenceKind::MultipleDefensibleInterpretations
            || self.kind == ReferenceKind::UnresolvedOrUngradable
    }

    /// Reduces to one label, or refuses.
    pub fn collapse_to_label(&self) -> Result<&str, ReferenceError> {
        if !self.kind.is_single_valued() || self.labels().len() > 1 {
            return Err(ReferenceError::Collapsed {
                standard: self.id.clone(),
                raters: self.ratings.len(),
            });
        }
        self.labels()
            .into_iter()
            .next()
            .ok_or_else(|| ReferenceError::Empty {
                standard: self.id.clone(),
            })
    }

    /// Set-valued acceptance: a response matching any attested reading is admitted.
    ///
    /// This is the mechanism 24.11 asks for so that a system is not "worse" for matching a
    /// minority but defensible view. It says nothing about how much credit the match earns —
    /// that is the scoring rule, which lives elsewhere on purpose.
    pub fn admits(&self, response: &str) -> bool {
        self.labels().contains(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rating(role: &str, label: &str) -> Rating {
        Rating {
            reviewer_role: role.to_string(),
            label: label.to_string(),
            confidence: "moderate".to_string(),
            evidence_visible: "T1c, T2-FLAIR, treatment timeline".to_string(),
            interpreted_at: "2024-02-01".to_string(),
            declared_conflicts: vec![],
        }
    }

    fn standard(kind: ReferenceKind, ratings: Vec<Rating>) -> ReferenceStandard {
        ReferenceStandard {
            id: "ref:response:0042".to_string(),
            kind,
            ratings,
            minimum_reviewers: 2,
            independence_protocol: "independent, blinded to each other".to_string(),
            ontology_version: "RANO 2023".to_string(),
            adjudication_procedure: "none; disagreement retained".to_string(),
            inter_rater_statistics: "Cohen kappa 0.41".to_string(),
        }
    }

    #[test]
    fn two_neuroradiologists_who_disagree_cannot_be_collapsed_into_one_answer() {
        let standard = standard(
            ReferenceKind::IndependentExpertDistribution,
            vec![
                rating("neuroradiologist", "progression"),
                rating("neuroradiologist", "treatment-effect"),
            ],
        );
        let err = standard.collapse_to_label().unwrap_err();
        assert_eq!(
            err,
            ReferenceError::Collapsed {
                standard: "ref:response:0042".to_string(),
                raters: 2
            }
        );
    }

    #[test]
    fn an_expert_distribution_stays_a_distribution_even_when_the_experts_happen_to_agree() {
        let standard = standard(
            ReferenceKind::IndependentExpertDistribution,
            vec![
                rating("neuroradiologist", "progression"),
                rating("neuroradiologist", "progression"),
            ],
        );
        assert!(standard.collapse_to_label().is_err());
    }

    #[test]
    fn a_synthetic_fixture_may_be_collapsed_because_its_truth_really_is_single_valued() {
        let standard = standard(
            ReferenceKind::SyntheticGroundTruth,
            vec![rating("generator", "progression")],
        );
        assert_eq!(standard.collapse_to_label().unwrap(), "progression");
    }

    #[test]
    fn a_minority_but_attested_reading_is_admitted_rather_than_marked_wrong() {
        let standard = standard(
            ReferenceKind::IndependentExpertDistribution,
            vec![
                rating("neuroradiologist", "progression"),
                rating("neuroradiologist", "progression"),
                rating("neuropathologist", "treatment-effect"),
            ],
        );
        assert!(standard.admits("treatment-effect"));
        assert!(standard.admits("progression"));
        assert!(!standard.admits("pseudoresponse"));
    }

    #[test]
    fn stable_disagreement_is_labelled_rather_than_treated_as_missing_data() {
        let standard = standard(
            ReferenceKind::IndependentExpertDistribution,
            vec![
                rating("neuroradiologist", "progression"),
                rating("neuroradiologist", "treatment-effect"),
            ],
        );
        assert!(standard.is_disagreement());
        assert_eq!(standard.distribution().len(), 2);
    }

    #[test]
    fn an_ungradable_case_counts_as_disagreement_even_with_a_single_recorded_reading() {
        let standard = standard(
            ReferenceKind::UnresolvedOrUngradable,
            vec![rating("neuroradiologist", "ungradable")],
        );
        assert!(standard.is_disagreement());
    }

    #[test]
    fn a_standard_that_does_not_say_what_each_reviewer_could_see_is_refused() {
        let mut standard = standard(
            ReferenceKind::IndependentExpertDistribution,
            vec![rating("neuroradiologist", "progression")],
        );
        standard.minimum_reviewers = 1;
        standard.ratings[0].evidence_visible = "  ".to_string();
        assert_eq!(
            standard.check().unwrap_err(),
            ReferenceError::MissingMetadata {
                standard: "ref:response:0042".to_string(),
                field: "the evidence visible to each reviewer"
            }
        );
    }

    #[test]
    fn a_standard_without_an_ontology_version_is_refused_because_labels_shift_between_editions() {
        let mut standard = standard(
            ReferenceKind::AdjudicatedConsensus,
            vec![rating("panel", "progression"), rating("panel", "progression")],
        );
        standard.ontology_version = String::new();
        assert!(matches!(
            standard.check().unwrap_err(),
            ReferenceError::MissingMetadata { .. }
        ));
    }

    #[test]
    fn a_standard_below_its_own_reviewer_floor_is_refused() {
        let standard = standard(
            ReferenceKind::IndependentExpertDistribution,
            vec![rating("neuroradiologist", "progression")],
        );
        assert_eq!(
            standard.check().unwrap_err(),
            ReferenceError::TooFewReviewers {
                standard: "ref:response:0042".to_string(),
                required: 2,
                found: 1
            }
        );
    }

    #[test]
    fn a_well_formed_expert_distribution_passes_its_metadata_check() {
        let standard = standard(
            ReferenceKind::IndependentExpertDistribution,
            vec![
                rating("neuroradiologist", "progression"),
                rating("neuropathologist", "treatment-effect"),
            ],
        );
        assert!(standard.check().is_ok());
    }
}
