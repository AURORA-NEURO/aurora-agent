//! BioWorld classes, planes, and what each class can and cannot answer.
//!
//! Blueprint 24.03 gives a table whose last column is the one that matters: counterfactual
//! strength per world class. An observed replay supports counterfactual claims only "unless
//! supported by design"; a mechanistic simulator is "limited by simulator validity"; a fully
//! synthetic world is exact "within the specification". These are not points on one scale, so
//! this module does not pretend they are. [`BioWorldDeclaration::admits`] is a table lookup, and the
//! table has holes on purpose.
//!
//! The single strongest statement it makes: **no world class licenses a real-world treatment
//! effect claim on its own.** Not observed replay, not the simulator, not the federated world.
//! That is 24.03 and 24.09 read together, and it is the claim most likely to be smuggled in by
//! a benchmark that "replays a clinical cohort".
//!
//! The second half of the module is partial observability. 24.03 makes the benchmark author
//! declare six visibility classes, and then requires that "the system must not reward an agent
//! for retrieving hidden labels through unintended paths". [`Visibility::may_be_returned`] is
//! that requirement as a function. It is a necessary condition, not a sufficient one — this
//! crate cannot detect an unintended path, only refuse the intended ones.
//!
//! Not implemented: world *validation*. 24.03 says access control and leakage tests are part of
//! world validation; those tests need a world instance and a runtime, neither of which lives
//! here.

use crate::error::WorldClassError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The six world classes of blueprint 24.03.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldClass {
    ObservedReplay,
    SemiSyntheticOverlay,
    MechanisticSimulator,
    ProspectiveLocked,
    FullySyntheticReference,
    FederatedPrivate,
}

impl WorldClass {
    pub const ALL: [WorldClass; 6] = [
        WorldClass::ObservedReplay,
        WorldClass::SemiSyntheticOverlay,
        WorldClass::MechanisticSimulator,
        WorldClass::ProspectiveLocked,
        WorldClass::FullySyntheticReference,
        WorldClass::FederatedPrivate,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            WorldClass::ObservedReplay => "observed replay",
            WorldClass::SemiSyntheticOverlay => "semi-synthetic overlay",
            WorldClass::MechanisticSimulator => "mechanistic simulator",
            WorldClass::ProspectiveLocked => "prospective locked world",
            WorldClass::FullySyntheticReference => "fully synthetic reference world",
            WorldClass::FederatedPrivate => "federated private world",
        }
    }

    /// The blueprint's own summary of this class's counterfactual strength, verbatim in spirit.
    /// Used in diagnostics so a refusal quotes the table rather than a number this crate made up.
    pub fn counterfactual_strength(self) -> &'static str {
        match self {
            WorldClass::ObservedReplay => "low unless supported by design",
            WorldClass::SemiSyntheticOverlay => "moderate, for injected factors",
            WorldClass::MechanisticSimulator => "limited by simulator validity",
            WorldClass::ProspectiveLocked => "strong, for the observed reveal",
            WorldClass::FullySyntheticReference => "exact within the specification",
            WorldClass::FederatedPrivate => "determined by site protocol",
        }
    }

    /// Claim kinds this class licenses without further declaration.
    fn natively_admits(self) -> &'static [CounterfactualClaim] {
        match self {
            WorldClass::ObservedReplay => {
                &[CounterfactualClaim::Associational, CounterfactualClaim::AnalysisFork]
            }
            WorldClass::SemiSyntheticOverlay => &[
                CounterfactualClaim::Associational,
                CounterfactualClaim::AnalysisFork,
                CounterfactualClaim::InjectedFactorEffect,
            ],
            WorldClass::MechanisticSimulator => &[
                CounterfactualClaim::Associational,
                CounterfactualClaim::AnalysisFork,
                CounterfactualClaim::SimulatedIntervention,
            ],
            WorldClass::ProspectiveLocked => &[
                CounterfactualClaim::Associational,
                CounterfactualClaim::AnalysisFork,
                CounterfactualClaim::RevealPrediction,
            ],
            WorldClass::FullySyntheticReference => &[
                CounterfactualClaim::Associational,
                CounterfactualClaim::AnalysisFork,
                CounterfactualClaim::InjectedFactorEffect,
                CounterfactualClaim::SimulatedIntervention,
                CounterfactualClaim::SpecifiedGroundTruth,
            ],
            WorldClass::FederatedPrivate => {
                &[CounterfactualClaim::Associational, CounterfactualClaim::AnalysisFork]
            }
        }
    }
}

/// The kinds of claim a world may be asked to support.
///
/// [`CounterfactualClaim::RealTreatmentEffect`] appears in no class's native set. That is the
/// point of having the enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterfactualClaim {
    /// "These vary together here." Requires no counterfactual licence at all.
    Associational,
    /// "A different analysis of the same evidence would have produced this." The forkability
    /// property of 24.01, and the one counterfactual every class supports.
    AnalysisFork,
    /// "The factor the benchmark injected had this effect." Only where a factor was injected.
    InjectedFactorEffect,
    /// "Under the declared model, this intervention does this." Never stronger than the model.
    SimulatedIntervention,
    /// "The hidden later timepoint will look like this." Licensed by commitment before reveal.
    RevealPrediction,
    /// "This is what the specification says is true." Only in a designed world.
    SpecifiedGroundTruth,
    /// "This treatment would have changed this patient's outcome." Licensed by no world class.
    RealTreatmentEffect,
}

impl CounterfactualClaim {
    pub fn as_str(self) -> &'static str {
        match self {
            CounterfactualClaim::Associational => "associational",
            CounterfactualClaim::AnalysisFork => "analysis fork",
            CounterfactualClaim::InjectedFactorEffect => "injected-factor effect",
            CounterfactualClaim::SimulatedIntervention => "simulated intervention",
            CounterfactualClaim::RevealPrediction => "reveal prediction",
            CounterfactualClaim::SpecifiedGroundTruth => "specified ground truth",
            CounterfactualClaim::RealTreatmentEffect => "real treatment effect",
        }
    }
}

/// The five planes every BioWorld separates (24.03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldPlane {
    Reality,
    Material,
    Observation,
    Decision,
    Evaluation,
}

impl WorldPlane {
    pub const ALL: [WorldPlane; 5] = [
        WorldPlane::Reality,
        WorldPlane::Material,
        WorldPlane::Observation,
        WorldPlane::Decision,
        WorldPlane::Evaluation,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            WorldPlane::Reality => "reality",
            WorldPlane::Material => "material",
            WorldPlane::Observation => "observation",
            WorldPlane::Decision => "decision",
            WorldPlane::Evaluation => "evaluation",
        }
    }
}

/// What a transition actually changed (24.03, "state transitions").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionEffect {
    LatentBiologicalState,
    ObservationState,
    KnowledgeState,
    ComputationalMaterialState,
}

/// One recorded world transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    pub plane: WorldPlane,
    pub effects: BTreeSet<TransitionEffect>,
}

impl Transition {
    /// Running an analysis does not change the tumour.
    ///
    /// A transition on the observation or evaluation plane that claims to have changed latent
    /// biology is a plane confusion, and it is the confusion that turns a preprocessing choice
    /// into an apparent biological finding.
    pub fn check(&self) -> Result<(), WorldClassError> {
        let touches_biology = self
            .effects
            .contains(&TransitionEffect::LatentBiologicalState);
        let may_touch_biology = matches!(self.plane, WorldPlane::Reality | WorldPlane::Material);
        if touches_biology && !may_touch_biology {
            return Err(WorldClassError::PlaneConfusion {
                plane: self.plane.as_str(),
            });
        }
        Ok(())
    }
}

/// The six visibility classes a benchmark author declares (24.03, partial observability).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    InitiallyVisible,
    AccessibleThroughAction,
    PermanentlyHidden,
    WithheldForScoring,
    ProhibitedByPrivacyOrRole,
    UncertainExpertInterpretation,
}

impl Visibility {
    pub const ALL: [Visibility; 6] = [
        Visibility::InitiallyVisible,
        Visibility::AccessibleThroughAction,
        Visibility::PermanentlyHidden,
        Visibility::WithheldForScoring,
        Visibility::ProhibitedByPrivacyOrRole,
        Visibility::UncertainExpertInterpretation,
    ];

    /// Whether this information may reach the system under evaluation.
    ///
    /// `AccessibleThroughAction` returns true only when the action was actually taken, which is
    /// why the caller supplies `action_taken` rather than this being a property of the class.
    /// `UncertainExpertInterpretation` is visible but must never be presented as ground truth;
    /// that distinction belongs to the reference-standard module, not here.
    pub fn may_be_returned(self, action_taken: bool) -> bool {
        match self {
            Visibility::InitiallyVisible | Visibility::UncertainExpertInterpretation => true,
            Visibility::AccessibleThroughAction => action_taken,
            Visibility::PermanentlyHidden
            | Visibility::WithheldForScoring
            | Visibility::ProhibitedByPrivacyOrRole => false,
        }
    }
}

/// A world's declaration of what it is and what it may be asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioWorldDeclaration {
    pub id: String,
    pub class: WorldClass,
    /// Claim kinds this specific world licenses beyond its class default, because its design
    /// supports them. This is the "unless supported by design" clause of 24.03, made explicit
    /// and per-world so that it is a reviewable statement instead of an assumption.
    #[serde(default)]
    pub design_support: BTreeSet<CounterfactualClaim>,
    /// Required when anything is withheld for scoring: how and when it is revealed.
    pub reveal_policy: Option<String>,
    #[serde(default)]
    pub withholds_for_scoring: bool,
}

impl BioWorldDeclaration {
    pub fn new(id: impl Into<String>, class: WorldClass) -> Self {
        BioWorldDeclaration {
            id: id.into(),
            class,
            design_support: BTreeSet::new(),
            reveal_policy: None,
            withholds_for_scoring: false,
        }
    }

    /// Whether this world may be asked for `claim`.
    pub fn admits(&self, claim: CounterfactualClaim) -> Result<(), WorldClassError> {
        if self.class.natively_admits().contains(&claim) || self.design_support.contains(&claim) {
            Ok(())
        } else {
            Err(WorldClassError::InsufficientCounterfactualStrength {
                class: self.class.as_str(),
                available: self.class.counterfactual_strength(),
                required: claim.as_str(),
            })
        }
    }

    /// A world that hides an answer for scoring must say when it stops hiding it.
    pub fn check_reveal_policy(&self) -> Result<(), WorldClassError> {
        if self.withholds_for_scoring
            && self
                .reveal_policy
                .as_ref()
                .is_none_or(|policy| policy.trim().is_empty())
        {
            return Err(WorldClassError::NoRevealPolicy {
                class: self.class.as_str(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_world_class_licenses_a_real_treatment_effect_claim_by_itself() {
        for class in WorldClass::ALL {
            let world = BioWorldDeclaration::new("w", class);
            assert!(
                world.admits(CounterfactualClaim::RealTreatmentEffect).is_err(),
                "{} admitted a real treatment counterfactual",
                class.as_str()
            );
        }
    }

    #[test]
    fn an_observed_replay_cannot_answer_a_simulated_intervention_question() {
        let world = BioWorldDeclaration::new("w", WorldClass::ObservedReplay);
        let err = world
            .admits(CounterfactualClaim::SimulatedIntervention)
            .unwrap_err();
        assert_eq!(
            err,
            WorldClassError::InsufficientCounterfactualStrength {
                class: "observed replay",
                available: "low unless supported by design",
                required: "simulated intervention"
            }
        );
    }

    #[test]
    fn the_unless_supported_by_design_clause_must_be_declared_per_world_not_assumed() {
        let mut world = BioWorldDeclaration::new("w", WorldClass::ObservedReplay);
        assert!(world
            .admits(CounterfactualClaim::InjectedFactorEffect)
            .is_err());
        world
            .design_support
            .insert(CounterfactualClaim::InjectedFactorEffect);
        assert!(world
            .admits(CounterfactualClaim::InjectedFactorEffect)
            .is_ok());
    }

    #[test]
    fn every_world_class_supports_the_analysis_fork_because_forkability_is_definitional() {
        for class in WorldClass::ALL {
            assert!(BioWorldDeclaration::new("w", class)
                .admits(CounterfactualClaim::AnalysisFork)
                .is_ok());
        }
    }

    #[test]
    fn only_a_designed_world_can_be_asked_what_is_specified_to_be_true() {
        let admitting: Vec<WorldClass> = WorldClass::ALL
            .into_iter()
            .filter(|class| {
                BioWorldDeclaration::new("w", *class)
                    .admits(CounterfactualClaim::SpecifiedGroundTruth)
                    .is_ok()
            })
            .collect();
        assert_eq!(admitting, vec![WorldClass::FullySyntheticReference]);
    }

    #[test]
    fn a_world_withholding_an_answer_for_scoring_must_declare_when_it_reveals_it() {
        let mut world = BioWorldDeclaration::new("w", WorldClass::ProspectiveLocked);
        world.withholds_for_scoring = true;
        assert!(matches!(
            world.check_reveal_policy().unwrap_err(),
            WorldClassError::NoRevealPolicy { .. }
        ));
        world.reveal_policy = Some("after plan commitment at t+12w".to_string());
        assert!(world.check_reveal_policy().is_ok());
    }

    #[test]
    fn an_analysis_on_the_observation_plane_cannot_have_changed_the_tumour() {
        let transition = Transition {
            plane: WorldPlane::Observation,
            effects: [TransitionEffect::LatentBiologicalState].into(),
        };
        assert_eq!(
            transition.check().unwrap_err(),
            WorldClassError::PlaneConfusion {
                plane: "observation"
            }
        );
    }

    #[test]
    fn a_reprocessing_step_changing_only_observation_and_material_state_is_legal() {
        let transition = Transition {
            plane: WorldPlane::Observation,
            effects: [
                TransitionEffect::ObservationState,
                TransitionEffect::ComputationalMaterialState,
            ]
            .into(),
        };
        assert!(transition.check().is_ok());
    }

    #[test]
    fn information_withheld_for_scoring_is_never_returned_however_hard_the_agent_works() {
        assert!(!Visibility::WithheldForScoring.may_be_returned(true));
        assert!(!Visibility::PermanentlyHidden.may_be_returned(true));
        assert!(!Visibility::ProhibitedByPrivacyOrRole.may_be_returned(true));
    }

    #[test]
    fn action_gated_information_arrives_only_after_the_action_is_taken() {
        assert!(!Visibility::AccessibleThroughAction.may_be_returned(false));
        assert!(Visibility::AccessibleThroughAction.may_be_returned(true));
    }
}
