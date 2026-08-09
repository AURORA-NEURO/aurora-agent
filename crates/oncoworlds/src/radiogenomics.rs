//! Radiogenomics and cross-modal prediction (30.08).
//!
//! Blueprint 30.08 evaluates "whether imaging-to-molecular models and agents obey cohort,
//! measurement, temporal, and causal boundaries". Four of those boundaries are checkable without a
//! model, a scanner or a cohort, and this module checks those four.
//!
//! # The ground truth is a specimen label
//!
//! Ladder item 6 is "detect when specimen heterogeneity invalidates a single ground-truth label",
//! and it is the hinge between this module and [`crate::clonal`]. An imaging model is trained
//! against a molecular call made on one fragment. If that call was **positive**, the fragment came
//! from the tumour and the tumour-level label holds. If it was **negative**, the label is a bound
//! over the sampled regions, and calling the tumour negative is exactly the promotion
//! [`crate::clonal::TumourClaim`] refuses to make. [`tumour_label`] therefore returns
//! [`TransportRefusal::SpecimenScopedTarget`] for the negative case: the target of a radiogenomic
//! model trained on it is *this specimen's call*, and every downstream sentence has to say so.
//!
//! # Splits, features and cohorts
//!
//! Three of 30.08's characteristic failures are properties of a design rather than of a result:
//! "image-level random splits", "training on derived features generated from all data", and
//! "selecting the best external cohort after seeing results". [`EvaluationDesign::check`] refuses
//! all three, and it refuses them in the order that makes the first refusal actionable.
//!
//! # Predicting site is not predicting mechanism
//!
//! The worked microbenchmark is a model with high pooled AUROC that fails within sites, and the
//! failure named is "claiming imaging predicts mechanism when it predicts site". A
//! [`RadiogenomicClaim`] carries what it claims to be about; a [`ClaimTarget::Mechanism`] claim
//! requires evidence stratified by the variables that could be standing in for it, and
//! [`assert_claim`] refuses when it is not.
//!
//! # Not implemented
//!
//! No feature extraction, no model, no metric, no imaging at all. The stratification requirement
//! names `site` and `scanner` because 30.08's own required state and worked microbenchmark name
//! them; it is not a claim that those are the only confounders, and nothing here estimates a
//! confounding effect.

use crate::clonal::{SpecimenObservation, TumourClaim};
use crate::error::TransportRefusal;
use crate::transport::DeclaredTransport;
use bioprism_onco::MolecularMarker;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Assumptions a cross-modal transport must state (30.08 required state).
pub const REQUIRED_ASSUMPTIONS: &[&str] = &[
    "imaging and specimen describe the same disease epoch",
    "the molecular target is defined at the scope the model predicts",
    "the feature representation version is fixed across train and test",
];

/// Variables a mechanistic claim must be stratified by before it can be distinguished from a
/// claim about acquisition (30.08).
pub const MECHANISM_STRATA: &[&str] = &["site", "scanner"];

/// The unit an evaluation splits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitUnit {
    Image,
    ImagingSeries,
    Specimen,
    Participant,
    Site,
}

impl SplitUnit {
    pub const fn as_str(self) -> &'static str {
        match self {
            SplitUnit::Image => "image",
            SplitUnit::ImagingSeries => "imaging series",
            SplitUnit::Specimen => "specimen",
            SplitUnit::Participant => "participant",
            SplitUnit::Site => "site",
        }
    }

    /// Whether splitting here can put one participant's material on both sides.
    ///
    /// Everything below participant can: one participant contributes several series, several
    /// specimens and many images, so a random split over any of them leaks.
    pub const fn leaks_participants(self) -> bool {
        !matches!(self, SplitUnit::Participant | SplitUnit::Site)
    }
}

/// Where a derived feature representation was fitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureProvenance {
    FittedOnTrainingSplitOnly,
    /// Normalisation, harmonisation or a learned representation fitted over every case.
    FittedOnAllData,
}

/// When an external cohort was chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "selection", rename_all = "snake_case")]
pub enum CohortSelection {
    PrespecifiedBeforeResults { cohort: String },
    ChosenAfterResults { cohort: String },
}

/// How an imaging-to-molecular evaluation was set up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationDesign {
    pub split_unit: SplitUnit,
    pub feature_provenance: FeatureProvenance,
    pub feature_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_cohort: Option<CohortSelection>,
    pub strata: BTreeSet<String>,
}

impl EvaluationDesign {
    pub fn new(split_unit: SplitUnit, feature_version: impl Into<String>) -> Self {
        EvaluationDesign {
            split_unit,
            feature_provenance: FeatureProvenance::FittedOnAllData,
            feature_version: feature_version.into(),
            external_cohort: None,
            strata: BTreeSet::new(),
        }
    }

    pub fn features_fitted_on_training_split(mut self) -> Self {
        self.feature_provenance = FeatureProvenance::FittedOnTrainingSplitOnly;
        self
    }

    pub fn validated_on(mut self, selection: CohortSelection) -> Self {
        self.external_cohort = Some(selection);
        self
    }

    pub fn stratified_by(mut self, stratum: impl Into<String>) -> Self {
        self.strata.insert(stratum.into());
        self
    }

    /// Whether the design supports any claim at all.
    pub fn check(&self) -> Result<(), TransportRefusal> {
        if self.split_unit.leaks_participants() {
            return Err(TransportRefusal::LeakySplit {
                unit: self.split_unit.as_str().to_string(),
            });
        }
        if self.feature_provenance == FeatureProvenance::FittedOnAllData {
            return Err(TransportRefusal::LeakySplit {
                unit: format!(
                    "derived features ({}) fitted over every case",
                    self.feature_version
                ),
            });
        }
        if let Some(CohortSelection::ChosenAfterResults { cohort }) = &self.external_cohort {
            return Err(TransportRefusal::PostHocCohortSelection {
                cohort: cohort.clone(),
            });
        }
        Ok(())
    }
}

/// A molecular target defined at the tumour, which is the only scope an imaging model of a whole
/// lesion can be said to predict.
///
/// Constructed only by [`tumour_label`]. There is no public constructor, no `Deserialize` and no
/// `From<SpecimenObservation>`, so a specimen-scoped call cannot become one by being moved into a
/// struct literal — the same device `bioprism_onco::ResearchOutput` uses for its boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TumourLabel {
    marker: MolecularMarker,
    basis: String,
}

impl TumourLabel {
    pub fn marker(&self) -> MolecularMarker {
        self.marker
    }

    pub fn basis(&self) -> &str {
        &self.basis
    }
}

/// Whether a specimen's molecular call may serve as a tumour-level target.
///
/// Positive calls promote; negative calls do not. See the module header and
/// [`crate::clonal::TumourClaim`].
pub fn tumour_label(observation: &SpecimenObservation) -> Result<TumourLabel, TransportRefusal> {
    let claim = observation
        .as_tumour_claim()
        .map_err(|refusal| TransportRefusal::SpecimenScopedTarget {
            detail: refusal.to_string(),
        })?;
    match claim {
        TumourClaim::PresentInSampledRegions { marker, regions } => {
            let regions: Vec<&str> = regions.iter().map(|region| region.as_str()).collect();
            Ok(TumourLabel {
                marker,
                basis: format!("detected in region(s) {}", regions.join(", ")),
            })
        }
        TumourClaim::UndetectedAboveFraction {
            marker, fraction, ..
        } => Err(TransportRefusal::SpecimenScopedTarget {
            detail: format!(
                "{} was undetected above {}/10000 in the sampled regions; that bounds the specimen, not the tumour",
                marker.describe(),
                fraction.parts_per_ten_thousand()
            ),
        }),
    }
}

/// What a radiogenomic result is being said to be about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimTarget {
    /// Imaging carries information about the molecular state in this cohort.
    Association,
    /// Imaging reflects the biology of the alteration itself.
    Mechanism,
}

impl ClaimTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            ClaimTarget::Association => "an association in this cohort",
            ClaimTarget::Mechanism => "a mechanistic relationship",
        }
    }
}

/// A claim someone wants to make from an imaging-to-molecular result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadiogenomicClaim {
    pub target: ClaimTarget,
    pub statement: String,
}

/// A claim that survived the design, target-scope and stratification checks.
///
/// Like [`TumourLabel`], constructible only by the function below.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SupportedClaim {
    claim: RadiogenomicClaim,
    label: TumourLabel,
    strata: BTreeSet<String>,
    transport: DeclaredTransport,
}

impl SupportedClaim {
    pub fn claim(&self) -> &RadiogenomicClaim {
        &self.claim
    }

    pub fn label(&self) -> &TumourLabel {
        &self.label
    }

    pub fn transport(&self) -> &DeclaredTransport {
        &self.transport
    }

    pub fn strata(&self) -> impl Iterator<Item = &str> {
        self.strata.iter().map(String::as_str)
    }
}

/// Whether a radiogenomic claim is supported by its design, target and transport.
///
/// Checked in the order design, transport, target scope, stratification. The design comes first
/// because a leaky split makes every later question about a number that means nothing.
pub fn assert_claim(
    claim: RadiogenomicClaim,
    design: &EvaluationDesign,
    observation: &SpecimenObservation,
    transport: &DeclaredTransport,
) -> Result<SupportedClaim, TransportRefusal> {
    design.check()?;
    transport.check(REQUIRED_ASSUMPTIONS)?;
    let label = tumour_label(observation)?;
    if claim.target == ClaimTarget::Mechanism {
        for stratum in MECHANISM_STRATA {
            if !design.strata.contains(*stratum) {
                let available: Vec<&str> = design.strata.iter().map(String::as_str).collect();
                return Err(TransportRefusal::UnstratifiedClaim {
                    target: claim.target.as_str().to_string(),
                    available: if available.is_empty() {
                        "nothing".to_string()
                    } else {
                        available.join(", ")
                    },
                });
            }
        }
    }
    Ok(SupportedClaim {
        claim,
        label,
        strata: design.strata.clone(),
        transport: transport.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clonal::{DetectionSensitivity, CellularFraction, RegionId, SpecimenSampling};
    use bioprism_onco::{MarkerCall, Observed};
    use bioprism_scope::ScopeKey;

    fn positive_observation() -> SpecimenObservation {
        SpecimenObservation::new(
            MolecularMarker::IdhMutation,
            SpecimenSampling::new("S1").sampling(RegionId::new("core")),
            Observed::Value(MarkerCall::Present),
        )
    }

    fn negative_observation() -> SpecimenObservation {
        SpecimenObservation::new(
            MolecularMarker::IdhMutation,
            SpecimenSampling::new("S1")
                .sampling(RegionId::new("core"))
                .detecting_down_to(DetectionSensitivity {
                    smallest_detectable_fraction: CellularFraction::from_parts_per_ten_thousand(500)
                        .unwrap(),
                    declared_by: "the caller's assay validation".to_string(),
                }),
            Observed::Value(MarkerCall::Absent),
        )
    }

    fn sound_transport() -> DeclaredTransport {
        let mut transport = DeclaredTransport::new(
            ScopeKey::new().exact("imaging_series", "SER-1"),
            ScopeKey::new().exact("patient", "PT-1"),
            "an imaging feature of the lesion stands for the molecular state of the tumour",
        )
        .losing("regional molecular variation within the lesion")
        .adding_uncertainty("segmentation boundary variability");
        for assumption in REQUIRED_ASSUMPTIONS {
            transport = transport.assuming(*assumption, "stated by the study protocol");
        }
        transport
    }

    fn sound_design() -> EvaluationDesign {
        EvaluationDesign::new(SplitUnit::Participant, "features-v1")
            .features_fitted_on_training_split()
    }

    #[test]
    fn an_image_level_split_puts_one_participant_on_both_sides() {
        let design = EvaluationDesign::new(SplitUnit::Image, "features-v1")
            .features_fitted_on_training_split();
        assert!(matches!(
            design.check().unwrap_err(),
            TransportRefusal::LeakySplit { .. }
        ));
        assert!(SplitUnit::ImagingSeries.leaks_participants());
        assert!(SplitUnit::Specimen.leaks_participants());
        assert!(!SplitUnit::Participant.leaks_participants());
    }

    #[test]
    fn features_fitted_over_every_case_leak_even_under_a_participant_split() {
        let design = EvaluationDesign::new(SplitUnit::Participant, "features-v1");
        assert!(matches!(
            design.check().unwrap_err(),
            TransportRefusal::LeakySplit { .. }
        ));
    }

    #[test]
    fn an_external_cohort_chosen_after_results_is_refused() {
        let design = sound_design().validated_on(CohortSelection::ChosenAfterResults {
            cohort: "the one that worked".to_string(),
        });
        assert!(matches!(
            design.check().unwrap_err(),
            TransportRefusal::PostHocCohortSelection { .. }
        ));
        let prespecified = sound_design().validated_on(CohortSelection::PrespecifiedBeforeResults {
            cohort: "named in the analysis plan".to_string(),
        });
        assert!(prespecified.check().is_ok());
    }

    #[test]
    fn a_radiogenomic_target_taken_from_one_negative_fragment_is_not_a_tumour_label() {
        let refusal = tumour_label(&negative_observation()).unwrap_err();
        assert!(matches!(
            refusal,
            TransportRefusal::SpecimenScopedTarget { .. }
        ));
    }

    #[test]
    fn a_positive_call_on_a_fragment_does_give_a_tumour_label() {
        let label = tumour_label(&positive_observation()).expect("presence promotes");
        assert_eq!(label.marker(), MolecularMarker::IdhMutation);
        assert!(label.basis().contains("core"));
    }

    #[test]
    fn an_unmeasured_marker_is_not_a_target_either() {
        let mut observation = positive_observation();
        observation.call =
            Observed::Unobserved(bioprism_onco::ObservationStatus::TechnicallyFailed);
        assert!(matches!(
            tumour_label(&observation).unwrap_err(),
            TransportRefusal::SpecimenScopedTarget { .. }
        ));
    }

    #[test]
    fn a_mechanistic_claim_needs_site_and_scanner_strata() {
        let claim = RadiogenomicClaim {
            target: ClaimTarget::Mechanism,
            statement: "the imaging phenotype reflects the alteration's biology".to_string(),
        };
        let refusal = assert_claim(
            claim.clone(),
            &sound_design(),
            &positive_observation(),
            &sound_transport(),
        )
        .unwrap_err();
        assert!(matches!(
            refusal,
            TransportRefusal::UnstratifiedClaim { .. }
        ));

        let stratified = sound_design().stratified_by("site").stratified_by("scanner");
        assert!(assert_claim(
            claim,
            &stratified,
            &positive_observation(),
            &sound_transport()
        )
        .is_ok());
    }

    #[test]
    fn an_association_claim_does_not_need_the_mechanism_strata() {
        let claim = RadiogenomicClaim {
            target: ClaimTarget::Association,
            statement: "the imaging feature carries information about the call in this cohort"
                .to_string(),
        };
        assert!(assert_claim(
            claim,
            &sound_design(),
            &positive_observation(),
            &sound_transport()
        )
        .is_ok());
    }

    #[test]
    fn an_undeclared_transport_refuses_before_the_target_is_examined() {
        let bare = DeclaredTransport::new(
            ScopeKey::new().exact("imaging_series", "SER-1"),
            ScopeKey::new().exact("patient", "PT-1"),
            "justification",
        );
        let claim = RadiogenomicClaim {
            target: ClaimTarget::Association,
            statement: "anything".to_string(),
        };
        let refusal =
            assert_claim(claim, &sound_design(), &positive_observation(), &bare).unwrap_err();
        assert!(matches!(refusal, TransportRefusal::UndeclaredLoss { .. }));
    }

    #[test]
    fn the_design_is_checked_before_the_transport() {
        let leaky = EvaluationDesign::new(SplitUnit::Image, "features-v1");
        let bare = DeclaredTransport::new(ScopeKey::new(), ScopeKey::new(), "justification");
        let claim = RadiogenomicClaim {
            target: ClaimTarget::Association,
            statement: "anything".to_string(),
        };
        assert!(matches!(
            assert_claim(claim, &leaky, &positive_observation(), &bare).unwrap_err(),
            TransportRefusal::LeakySplit { .. }
        ));
    }

    #[test]
    fn a_supported_claim_carries_the_transport_that_licensed_it() {
        let claim = RadiogenomicClaim {
            target: ClaimTarget::Association,
            statement: "an association".to_string(),
        };
        let supported = assert_claim(
            claim,
            &sound_design(),
            &positive_observation(),
            &sound_transport(),
        )
        .expect("the design and transport are sound");
        assert!(!supported.transport().loss.is_empty());
        assert!(supported
            .transport()
            .assumption_names()
            .any(|name| name.contains("disease epoch")));
    }
}
