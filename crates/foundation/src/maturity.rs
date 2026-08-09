//! Evidence maturity and the applicability envelope.
//!
//! Blueprint 24.12 exists to stop a true sentence from becoming a universal one. Its list of
//! ways to lose points is a list of things that are all compatible with the central fact being
//! correct: omitting an applicability restriction, transporting into an unsupported population,
//! reading an association as an intervention effect, reporting exploratory work as established.
//!
//! Two things here are genuinely checkable. First, an [`ApplicabilityEnvelope`] either states
//! its ten dimensions or it does not. Second, given a stated envelope, a request either falls
//! inside it or it does not — and [`ApplicabilityEnvelope::admits`] names the dimension that
//! failed, because "out of scope" without a dimension is not actionable.
//!
//! **A judgement this crate makes that the blueprint does not:** 24.12 says exploratory results
//! must not be reported as established, but never says where on its eight-rung ladder
//! "established" begins. [`EvidenceMaturity::may_be_presented_as_established`] draws the line
//! below independent replication, on the reasoning that internal validation shares the dataset,
//! the analysts and the errors of the original result. That line is a defensible default, not a
//! blueprint quotation, and it is one function to change.
//!
//! **Deliberately absent:** [`EvidenceMaturity`] does not implement `Ord`. The blueprint says
//! the ladder "is descriptive, not an automatic quality ranking" and that "different questions
//! require different evidence", so a mechanistic simulation is not simply worse than a clinical
//! study. Sorting systems by rung would be exactly the collapse 24.12 warns against, and a type
//! that cannot be sorted cannot be sorted by accident.

use crate::error::ApplicabilityError;
use serde::{Deserialize, Serialize};

/// The eight rungs of blueprint 24.12.
///
/// `PartialOrd`/`Ord` are withheld on purpose; see the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceMaturity {
    SyntheticOrMechanisticAssumption,
    ExploratorySingleDataset,
    InternallyValidatedRetrospective,
    IndependentlyReplicatedRetrospective,
    ExternalOrFederatedValidation,
    ProspectiveObservational,
    ExperimentalPerturbation,
    ClinicalStudyOrImplementation,
}

impl EvidenceMaturity {
    pub const ALL: [EvidenceMaturity; 8] = [
        EvidenceMaturity::SyntheticOrMechanisticAssumption,
        EvidenceMaturity::ExploratorySingleDataset,
        EvidenceMaturity::InternallyValidatedRetrospective,
        EvidenceMaturity::IndependentlyReplicatedRetrospective,
        EvidenceMaturity::ExternalOrFederatedValidation,
        EvidenceMaturity::ProspectiveObservational,
        EvidenceMaturity::ExperimentalPerturbation,
        EvidenceMaturity::ClinicalStudyOrImplementation,
    ];

    /// Position on the ladder, 1 to 8. A position, not a score: the number exists so results
    /// can be grouped and rendered in the blueprint's order, and comparing two rungs with `<`
    /// requires a caller to write `rung()` explicitly and own that comparison.
    pub fn rung(self) -> u8 {
        match self {
            EvidenceMaturity::SyntheticOrMechanisticAssumption => 1,
            EvidenceMaturity::ExploratorySingleDataset => 2,
            EvidenceMaturity::InternallyValidatedRetrospective => 3,
            EvidenceMaturity::IndependentlyReplicatedRetrospective => 4,
            EvidenceMaturity::ExternalOrFederatedValidation => 5,
            EvidenceMaturity::ProspectiveObservational => 6,
            EvidenceMaturity::ExperimentalPerturbation => 7,
            EvidenceMaturity::ClinicalStudyOrImplementation => 8,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceMaturity::SyntheticOrMechanisticAssumption => {
                "synthetic or mechanistic assumption"
            }
            EvidenceMaturity::ExploratorySingleDataset => "exploratory single-dataset observation",
            EvidenceMaturity::InternallyValidatedRetrospective => {
                "internally validated retrospective result"
            }
            EvidenceMaturity::IndependentlyReplicatedRetrospective => {
                "independently replicated retrospective result"
            }
            EvidenceMaturity::ExternalOrFederatedValidation => "external or federated validation",
            EvidenceMaturity::ProspectiveObservational => "prospective observational validation",
            EvidenceMaturity::ExperimentalPerturbation => {
                "experimental perturbation or interventional evidence"
            }
            EvidenceMaturity::ClinicalStudyOrImplementation => {
                "clinical-study or implementation evidence"
            }
        }
    }

    /// Whether evidence at this rung may be described as established.
    ///
    /// The cut sits below independent replication. See the module docs: this is this crate's
    /// judgement, not the blueprint's.
    pub fn may_be_presented_as_established(self) -> bool {
        self.rung() >= EvidenceMaturity::IndependentlyReplicatedRetrospective.rung()
    }

    /// Whether this rung carries intervention evidence, which 24.13 requires before an
    /// interventional claim is admissible.
    pub fn carries_intervention_evidence(self) -> bool {
        matches!(
            self,
            EvidenceMaturity::ExperimentalPerturbation
                | EvidenceMaturity::ClinicalStudyOrImplementation
        )
    }
}

/// The ten dimensions blueprint 24.12 requires every claim to record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeDimension {
    SpeciesAndModelSystem,
    AgeAndDevelopmentalStage,
    DiseaseOrSubtype,
    TreatmentAndTemporalContext,
    SpecimenAndAssay,
    SiteAndPopulation,
    OutcomeAndFollowUpWindow,
}

impl EnvelopeDimension {
    pub const ALL: [EnvelopeDimension; 7] = [
        EnvelopeDimension::SpeciesAndModelSystem,
        EnvelopeDimension::AgeAndDevelopmentalStage,
        EnvelopeDimension::DiseaseOrSubtype,
        EnvelopeDimension::TreatmentAndTemporalContext,
        EnvelopeDimension::SpecimenAndAssay,
        EnvelopeDimension::SiteAndPopulation,
        EnvelopeDimension::OutcomeAndFollowUpWindow,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            EnvelopeDimension::SpeciesAndModelSystem => "species and model system",
            EnvelopeDimension::AgeAndDevelopmentalStage => "age and developmental stage",
            EnvelopeDimension::DiseaseOrSubtype => "disease or subtype",
            EnvelopeDimension::TreatmentAndTemporalContext => "treatment and temporal context",
            EnvelopeDimension::SpecimenAndAssay => "specimen and assay",
            EnvelopeDimension::SiteAndPopulation => "site and population",
            EnvelopeDimension::OutcomeAndFollowUpWindow => "outcome and follow-up window",
        }
    }
}

/// Where a claim holds, and where it stops holding.
///
/// The seven positional dimensions are required; exclusions, maturity and unresolved moderators
/// complete the ten items 24.12 lists. Unresolved moderators may legitimately be empty, so their
/// emptiness is not an error — but they are carried, because a claim that lists none has said so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicabilityEnvelope {
    pub claim: String,
    pub bindings: std::collections::BTreeMap<EnvelopeDimension, String>,
    /// Values explicitly outside the envelope even where the positional binding would match.
    #[serde(default)]
    pub exclusions: std::collections::BTreeSet<String>,
    pub maturity: EvidenceMaturity,
    #[serde(default)]
    pub unresolved_moderators: Vec<String>,
}

impl ApplicabilityEnvelope {
    pub fn new(claim: impl Into<String>, maturity: EvidenceMaturity) -> Self {
        ApplicabilityEnvelope {
            claim: claim.into(),
            bindings: std::collections::BTreeMap::new(),
            exclusions: std::collections::BTreeSet::new(),
            maturity,
            unresolved_moderators: Vec::new(),
        }
    }

    pub fn bind(mut self, dimension: EnvelopeDimension, value: impl Into<String>) -> Self {
        self.bindings.insert(dimension, value.into());
        self
    }

    pub fn exclude(mut self, value: impl Into<String>) -> Self {
        self.exclusions.insert(value.into());
        self
    }

    /// Every positional dimension must be stated. A blank binding is treated as absent, since
    /// an empty string restricts nothing while looking like it does.
    pub fn check(&self) -> Result<(), ApplicabilityError> {
        for dimension in EnvelopeDimension::ALL {
            let stated = self
                .bindings
                .get(&dimension)
                .is_some_and(|value| !value.trim().is_empty());
            if !stated {
                return Err(ApplicabilityError::IncompleteEnvelope {
                    claim: self.claim.clone(),
                    field: dimension.as_str(),
                });
            }
        }
        Ok(())
    }

    /// Whether the claim may be applied at `requested` along `dimension`.
    ///
    /// Matching is exact, and deliberately so: this crate has no ontology and cannot know that
    /// "GBM" and "glioblastoma" name the same thing. Callers that need synonymy normalize
    /// first, which keeps the normalization visible instead of hiding it in a comparison.
    pub fn admits(
        &self,
        dimension: EnvelopeDimension,
        requested: &str,
    ) -> Result<(), ApplicabilityError> {
        if self.exclusions.contains(requested) {
            return Err(ApplicabilityError::OutsideEnvelope {
                claim: self.claim.clone(),
                dimension: dimension.as_str(),
                declared: format!("excluded: {requested}"),
                requested: requested.to_string(),
            });
        }
        match self.bindings.get(&dimension) {
            Some(declared) if declared == requested => Ok(()),
            Some(declared) => Err(ApplicabilityError::OutsideEnvelope {
                claim: self.claim.clone(),
                dimension: dimension.as_str(),
                declared: declared.clone(),
                requested: requested.to_string(),
            }),
            None => Err(ApplicabilityError::IncompleteEnvelope {
                claim: self.claim.clone(),
                field: dimension.as_str(),
            }),
        }
    }

    /// Refuses to describe evidence below the replication cut as established.
    pub fn present_as_established(&self) -> Result<(), ApplicabilityError> {
        if self.maturity.may_be_presented_as_established() {
            Ok(())
        } else {
            Err(ApplicabilityError::OverstatedMaturity {
                claim: self.claim.clone(),
                maturity: self.maturity.as_str(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> ApplicabilityEnvelope {
        let mut envelope = ApplicabilityEnvelope::new(
            "MGMT methylation predicts temozolomide benefit",
            EvidenceMaturity::IndependentlyReplicatedRetrospective,
        );
        for dimension in EnvelopeDimension::ALL {
            envelope = envelope.bind(dimension, "stated");
        }
        envelope
            .bind(EnvelopeDimension::AgeAndDevelopmentalStage, "adult")
            .bind(EnvelopeDimension::SpeciesAndModelSystem, "human")
    }

    #[test]
    fn an_envelope_missing_one_of_seven_dimensions_is_incomplete() {
        let mut envelope = envelope();
        envelope.bindings.remove(&EnvelopeDimension::SiteAndPopulation);
        assert_eq!(
            envelope.check().unwrap_err(),
            ApplicabilityError::IncompleteEnvelope {
                claim: envelope.claim.clone(),
                field: "site and population"
            }
        );
    }

    #[test]
    fn a_claim_stated_for_adults_is_refused_for_paediatric_use_and_names_the_dimension() {
        let err = envelope()
            .admits(EnvelopeDimension::AgeAndDevelopmentalStage, "paediatric")
            .unwrap_err();
        assert_eq!(
            err,
            ApplicabilityError::OutsideEnvelope {
                claim: "MGMT methylation predicts temozolomide benefit".to_string(),
                dimension: "age and developmental stage",
                declared: "adult".to_string(),
                requested: "paediatric".to_string()
            }
        );
    }

    #[test]
    fn an_explicit_exclusion_overrides_a_matching_positional_binding() {
        let envelope = envelope().exclude("human");
        assert!(envelope
            .admits(EnvelopeDimension::SpeciesAndModelSystem, "human")
            .is_err());
    }

    #[test]
    fn an_exploratory_single_dataset_result_may_not_be_presented_as_established() {
        let mut envelope = envelope();
        envelope.maturity = EvidenceMaturity::ExploratorySingleDataset;
        assert!(matches!(
            envelope.present_as_established().unwrap_err(),
            ApplicabilityError::OverstatedMaturity { .. }
        ));
    }

    #[test]
    fn internal_validation_alone_does_not_make_a_result_established() {
        assert!(
            !EvidenceMaturity::InternallyValidatedRetrospective.may_be_presented_as_established()
        );
        assert!(
            EvidenceMaturity::IndependentlyReplicatedRetrospective.may_be_presented_as_established()
        );
    }

    #[test]
    fn only_perturbation_and_clinical_study_rungs_carry_intervention_evidence() {
        let carriers: Vec<EvidenceMaturity> = EvidenceMaturity::ALL
            .into_iter()
            .filter(|m| m.carries_intervention_evidence())
            .collect();
        assert_eq!(
            carriers,
            vec![
                EvidenceMaturity::ExperimentalPerturbation,
                EvidenceMaturity::ClinicalStudyOrImplementation
            ]
        );
    }

    #[test]
    fn the_eight_rungs_are_distinct_and_numbered_one_through_eight() {
        let mut rungs: Vec<u8> = EvidenceMaturity::ALL.iter().map(|m| m.rung()).collect();
        rungs.sort_unstable();
        assert_eq!(rungs, (1..=8).collect::<Vec<u8>>());
    }
}
