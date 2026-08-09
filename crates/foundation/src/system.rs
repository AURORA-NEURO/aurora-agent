//! The evaluated-system contract.
//!
//! Blueprint 24.01 requires one contract to cover four different kinds of thing: predictive
//! models, analysis pipelines, research agents, and agent molecules coordinating through
//! Weave. What makes a single contract possible is not that these four are alike — they are
//! not — but that all four must answer the same three questions before a score means anything:
//! *who are you*, *which version of you*, and *what do you claim you can do*.
//!
//! A system that cannot answer them is not a badly performing system; it is an unevaluable
//! one, and the difference matters. A missing capability declaration produces a zero that looks
//! like a measurement. [`EvaluatedSystem::declare`] refuses instead.
//!
//! What this module does **not** do is verify the declaration. Nothing here checks that a
//! system claiming `choose-next-assay` can in fact choose an assay; that is the job of the
//! evaluation runtime and the oracle mesh. This is the admission gate, not the exam.

use crate::error::EvaluabilityError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The four classes of system blueprint 24.01 requires the hub to evaluate through one
/// contract. The class is not a difficulty tier — it says what kind of thing is being held to
/// the contract, which determines which capabilities are even meaningful to declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemClass {
    /// Maps one biological representation to another. Has outputs; has no actions.
    PredictiveModel,
    /// Transforms raw observations into estimates or artifacts. Has stages; chooses none of them.
    AnalysisPipeline,
    /// Chooses tools, data, analyses, and next experiments.
    ResearchAgent,
    /// Specialized participants coordinating through AURORA Weave.
    AgentMolecule,
}

impl SystemClass {
    pub fn as_str(self) -> &'static str {
        match self {
            SystemClass::PredictiveModel => "predictive_model",
            SystemClass::AnalysisPipeline => "analysis_pipeline",
            SystemClass::ResearchAgent => "research_agent",
            SystemClass::AgentMolecule => "agent_molecule",
        }
    }

    /// Whether systems of this class select their own next action.
    ///
    /// This is the one structural difference the contract has to know about: scarcity scoring
    /// (24.10) and evidence-acquisition scoring only apply to systems that choose. Scoring a
    /// predictive model on assay selection would be scoring it for something it cannot do.
    pub fn selects_actions(self) -> bool {
        matches!(
            self,
            SystemClass::ResearchAgent | SystemClass::AgentMolecule
        )
    }
}

/// The six properties blueprint 24.01 uses to define the category boundary.
///
/// Declaring them is checkable; satisfying them is not checkable from inside this crate. A
/// benchmark that declares fewer than six is not a BioPRISM benchmark — it may still be a
/// perfectly good benchmark, which is why the refusal names the missing property rather than
/// passing judgement on the work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategoryProperty {
    /// Entities, specimens, time, interventions and scope, not only text prompts.
    BiologicalState,
    /// Every measured value is linked to an assay, protocol, batch and uncertainty model.
    ObservationProcess,
    /// The system can acquire evidence, analyse, reserve scarce samples, propose, or abstain.
    Actionability,
    /// Claims have explicit tests, disconfirming outcomes, applicability limits and maturity.
    Falsifiability,
    /// A decision state can be replayed with another architecture without rerunning everything.
    Forkability,
    /// Scores resolve to immutable artifacts, decisions, evaluators and reference standards.
    Traceability,
}

impl CategoryProperty {
    pub const ALL: [CategoryProperty; 6] = [
        CategoryProperty::BiologicalState,
        CategoryProperty::ObservationProcess,
        CategoryProperty::Actionability,
        CategoryProperty::Falsifiability,
        CategoryProperty::Forkability,
        CategoryProperty::Traceability,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            CategoryProperty::BiologicalState => "biological_state",
            CategoryProperty::ObservationProcess => "observation_process",
            CategoryProperty::Actionability => "actionability",
            CategoryProperty::Falsifiability => "falsifiability",
            CategoryProperty::Forkability => "forkability",
            CategoryProperty::Traceability => "traceability",
        }
    }
}

/// A benchmark's claim to sit inside the BioPRISM category (24.01).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CategoryDeclaration {
    /// How the benchmark satisfies each property, in its own words. The text is not validated;
    /// its presence is. An empty string counts as absent, because a blank justification is a
    /// declaration nobody can review.
    pub satisfied: std::collections::BTreeMap<CategoryProperty, String>,
}

impl CategoryDeclaration {
    pub fn new() -> Self {
        CategoryDeclaration::default()
    }

    pub fn declare(mut self, property: CategoryProperty, justification: impl Into<String>) -> Self {
        self.satisfied.insert(property, justification.into());
        self
    }

    /// All six or nothing. Blueprint 24.01 lists the six as jointly definitional, so five is
    /// not "mostly in the category".
    pub fn check(&self) -> Result<(), EvaluabilityError> {
        for property in CategoryProperty::ALL {
            let present = self
                .satisfied
                .get(&property)
                .is_some_and(|justification| !justification.trim().is_empty());
            if !present {
                return Err(EvaluabilityError::UndeclaredCategoryProperty {
                    property: property.as_str(),
                });
            }
        }
        Ok(())
    }
}

/// A system admitted for evaluation.
///
/// Fields are private and construction goes through [`EvaluatedSystem::declare`], so holding
/// one of these is evidence the admission checks ran. Serde deserialization deliberately goes
/// through the same gate via `try_from`, because a system that arrives over the wire has not
/// been checked by anyone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SystemDeclaration")]
pub struct EvaluatedSystem {
    identity: String,
    version: String,
    class: SystemClass,
    capabilities: BTreeSet<String>,
    actions: BTreeSet<String>,
}

/// The unchecked wire form of an [`EvaluatedSystem`]. Exists so that JSON input passes through
/// the same admission gate as programmatic construction rather than around it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemDeclaration {
    pub identity: String,
    pub version: String,
    pub class: SystemClass,
    #[serde(default)]
    pub capabilities: BTreeSet<String>,
    #[serde(default)]
    pub actions: BTreeSet<String>,
}

impl TryFrom<SystemDeclaration> for EvaluatedSystem {
    type Error = EvaluabilityError;

    fn try_from(value: SystemDeclaration) -> Result<Self, Self::Error> {
        EvaluatedSystem::declare(
            value.identity,
            value.version,
            value.class,
            value.capabilities,
            value.actions,
        )
    }
}

/// The action every system may always declare, and the reason [`EvaluatedSystem::declare`] can
/// insist on a non-empty action set without excluding predictive models: abstaining is a
/// legitimate response to insufficient evidence, and 24.11 scores it as one.
pub const ABSTAIN: &str = "abstain";

impl EvaluatedSystem {
    /// The admission gate. Every refusal here is a refusal to produce an uninterpretable score.
    pub fn declare(
        identity: impl Into<String>,
        version: impl Into<String>,
        class: SystemClass,
        capabilities: impl IntoIterator<Item = String>,
        actions: impl IntoIterator<Item = String>,
    ) -> Result<Self, EvaluabilityError> {
        let identity = identity.into();
        if identity.trim().is_empty() {
            return Err(EvaluabilityError::MissingIdentity);
        }
        let version = version.into();
        if version.trim().is_empty() {
            return Err(EvaluabilityError::MissingVersion { system: identity });
        }
        let capabilities: BTreeSet<String> = capabilities
            .into_iter()
            .filter(|c| !c.trim().is_empty())
            .collect();
        if capabilities.is_empty() {
            return Err(EvaluabilityError::NoDeclaredCapabilities { system: identity });
        }
        let actions: BTreeSet<String> =
            actions.into_iter().filter(|a| !a.trim().is_empty()).collect();
        if actions.is_empty() {
            return Err(EvaluabilityError::NoAdmissibleAction { system: identity });
        }
        Ok(EvaluatedSystem {
            identity,
            version,
            class,
            capabilities,
            actions,
        })
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn class(&self) -> SystemClass {
        self.class
    }

    pub fn capabilities(&self) -> impl Iterator<Item = &str> {
        self.capabilities.iter().map(String::as_str)
    }

    pub fn actions(&self) -> impl Iterator<Item = &str> {
        self.actions.iter().map(String::as_str)
    }

    /// Refuses actions the system never declared.
    ///
    /// This protects the system, not the harness: a benchmark that asks a segmentation model to
    /// order an assay and records the refusal as a failure has measured the benchmark's
    /// confusion, not the model's competence.
    pub fn admit_action(&self, action: &str) -> Result<(), EvaluabilityError> {
        if self.actions.contains(action) {
            Ok(())
        } else {
            Err(EvaluabilityError::UndeclaredAction {
                system: self.identity.clone(),
                action: action.to_string(),
            })
        }
    }

    /// Whether scarcity and evidence-acquisition dimensions (24.10) apply to this system.
    pub fn is_scored_on_acquisition(&self) -> bool {
        self.class.selects_actions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps() -> Vec<String> {
        vec!["segment-enhancing-tumor".to_string()]
    }

    #[test]
    fn a_system_without_an_identity_is_unevaluable_rather_than_scoring_zero() {
        let err =
            EvaluatedSystem::declare("  ", "1.0", SystemClass::PredictiveModel, caps(), vec![])
                .unwrap_err();
        assert_eq!(err, EvaluabilityError::MissingIdentity);
    }

    #[test]
    fn a_system_without_a_version_is_refused_because_its_results_cannot_be_reproduced() {
        let err = EvaluatedSystem::declare(
            "unet",
            "",
            SystemClass::PredictiveModel,
            caps(),
            vec![ABSTAIN.to_string()],
        )
        .unwrap_err();
        assert_eq!(
            err,
            EvaluabilityError::MissingVersion {
                system: "unet".to_string()
            }
        );
    }

    #[test]
    fn a_system_declaring_no_capability_is_refused_because_nothing_holds_it_to_anything() {
        let err = EvaluatedSystem::declare(
            "unet",
            "1.0",
            SystemClass::PredictiveModel,
            Vec::<String>::new(),
            vec![ABSTAIN.to_string()],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            EvaluabilityError::NoDeclaredCapabilities { .. }
        ));
    }

    #[test]
    fn abstention_alone_is_a_sufficient_action_set() {
        let system = EvaluatedSystem::declare(
            "unet",
            "1.0",
            SystemClass::PredictiveModel,
            caps(),
            vec![ABSTAIN.to_string()],
        )
        .unwrap();
        assert!(system.admit_action(ABSTAIN).is_ok());
    }

    #[test]
    fn asking_a_system_to_perform_an_undeclared_action_is_the_harness_error_not_a_failed_score() {
        let system = EvaluatedSystem::declare(
            "unet",
            "1.0",
            SystemClass::PredictiveModel,
            caps(),
            vec![ABSTAIN.to_string()],
        )
        .unwrap();
        let err = system.admit_action("order-perfusion-mri").unwrap_err();
        assert_eq!(
            err,
            EvaluabilityError::UndeclaredAction {
                system: "unet".to_string(),
                action: "order-perfusion-mri".to_string()
            }
        );
    }

    #[test]
    fn only_action_selecting_classes_are_scored_on_evidence_acquisition() {
        let pipeline = EvaluatedSystem::declare(
            "nf-core/rnaseq",
            "3.14",
            SystemClass::AnalysisPipeline,
            caps(),
            vec![ABSTAIN.to_string()],
        )
        .unwrap();
        let agent = EvaluatedSystem::declare(
            "triage-agent",
            "0.2",
            SystemClass::ResearchAgent,
            caps(),
            vec![ABSTAIN.to_string()],
        )
        .unwrap();
        assert!(!pipeline.is_scored_on_acquisition());
        assert!(agent.is_scored_on_acquisition());
    }

    #[test]
    fn a_system_arriving_as_json_passes_through_the_same_admission_gate() {
        let json = r#"{"identity":"x","version":"1","class":"research_agent","capabilities":[]}"#;
        let err = serde_json::from_str::<EvaluatedSystem>(json).unwrap_err();
        assert!(err.to_string().contains("declares no capabilities"));
    }

    #[test]
    fn a_category_declaration_missing_one_of_six_properties_is_not_mostly_in_the_category() {
        let mut declaration = CategoryDeclaration::new();
        for property in CategoryProperty::ALL {
            if property != CategoryProperty::Forkability {
                declaration = declaration.declare(property, "see world spec");
            }
        }
        let err = declaration.check().unwrap_err();
        assert_eq!(
            err,
            EvaluabilityError::UndeclaredCategoryProperty {
                property: "forkability"
            }
        );
    }

    #[test]
    fn a_blank_justification_counts_as_an_undeclared_category_property() {
        let mut declaration = CategoryDeclaration::new();
        for property in CategoryProperty::ALL {
            declaration = declaration.declare(property, "   ");
        }
        assert!(declaration.check().is_err());
    }
}
