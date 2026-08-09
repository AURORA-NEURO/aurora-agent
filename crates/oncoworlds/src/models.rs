//! Patient-derived models, organoids and xenografts (30.19).
//!
//! Blueprint 30.19 evaluates "how faithfully experimental models preserve a source tumor, what
//! selection occurs during establishment, and which conclusions can transport back to patients".
//! The rule this module encodes is the one its title implies: **a model system is evidence about a
//! model system.** An organoid result is a true statement about organoids until somebody states
//! the assumptions that carry it somewhere else.
//!
//! That is enforced structurally. [`ModelResult`] is freely constructible and freely serialisable;
//! [`PatientRelevantClaim`] has no public constructor, no `Deserialize`, and is produced only by
//! [`transport_to_patients`]. An untransported cross-system claim therefore does not typecheck as
//! a claim about the disease — it stays a [`ModelResult`], which says in its own type name what it
//! is about.
//!
//! # The four things that get in the way
//!
//! In the order [`transport_to_patients`] checks them:
//!
//! 1. **Identity.** Ladder item 1 is "verify identity and contamination". A model not checked
//!    against its source specimen is not known to be that patient's model, and every later
//!    question is about an unknown cell population.
//! 2. **Fidelity, per axis.** 30.19's required state lists "genomic, epigenetic, transcriptomic,
//!    phenotypic, and histologic similarity" — five axes, and a claim resting on an unmeasured one
//!    is undeclared on that axis. The caller states which axes the claim rests on; this module
//!    does not guess, because a drug-sensitivity claim and a subtype-fidelity claim rest on
//!    different things.
//! 3. **Establishment selection.** The worked microbenchmark: "only aggressive specimens establish
//!    organoids. A drug appears effective in the established panel." Whenever fewer specimens
//!    established than were attempted, the panel is a selected sample of tumours, and a claim about
//!    the population needs that selection modelled. This is arithmetic on counts the caller
//!    supplies — [`EstablishmentCohort`] has no rate threshold, because 30.19 states none.
//! 4. **Replicates.** "Using technical wells as biological replicates" is a named failure. The
//!    effective n is the number of biological replicates, and a claim asserting more is refused.
//!
//! And then the transport itself, which must carry a non-empty loss ledger and the assumption
//! names in [`REQUIRED_ASSUMPTIONS`].
//!
//! # Not implemented
//!
//! No fidelity metric, no similarity computation, no drug-response model, no engraftment
//! prediction. [`FidelityEvidence`] records that an axis *was measured at a stated passage*, not
//! how similar the result was — 30.19 supplies no similarity scale, and a cutoff invented here
//! would be presented as a fidelity standard that does not exist.

use crate::error::TransportRefusal;
use crate::transport::DeclaredTransport;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Assumptions a model-to-patient transport must state (30.19 required state and ladder item 6).
pub const REQUIRED_ASSUMPTIONS: &[&str] = &[
    "the culture, host, matrix and selection conditions are stated",
    "the passage at which the effect was observed is the passage fidelity was measured at",
    "the population the claim is about is the population models were attempted from",
];

/// The model classes 30.19's title names.
///
/// Deliberately short. The blueprint's title is "Patient-Derived Models, Organoids, and
/// Xenografts", and extending this to immortalised lines, neurospheres or explants would be this
/// crate adding model systems the section does not discuss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSystem {
    Organoid,
    PatientDerivedXenograft,
}

impl ModelSystem {
    pub const fn as_str(self) -> &'static str {
        match self {
            ModelSystem::Organoid => "organoid",
            ModelSystem::PatientDerivedXenograft => "patient-derived xenograft",
        }
    }
}

/// The similarity axes 30.19 lists, verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FidelityAxis {
    Genomic,
    Epigenetic,
    Transcriptomic,
    Phenotypic,
    Histologic,
}

impl FidelityAxis {
    pub const ALL: [FidelityAxis; 5] = [
        FidelityAxis::Genomic,
        FidelityAxis::Epigenetic,
        FidelityAxis::Transcriptomic,
        FidelityAxis::Phenotypic,
        FidelityAxis::Histologic,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            FidelityAxis::Genomic => "genomic",
            FidelityAxis::Epigenetic => "epigenetic",
            FidelityAxis::Transcriptomic => "transcriptomic",
            FidelityAxis::Phenotypic => "phenotypic",
            FidelityAxis::Histologic => "histologic",
        }
    }
}

/// A model and its link back to the specimen it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelIdentity {
    pub model: String,
    pub system: ModelSystem,
    pub source_specimen: String,
    pub passage: u32,
    /// Whether identity and contamination were checked against the source (ladder item 1).
    pub verified_against_source: bool,
}

impl ModelIdentity {
    pub fn new(
        model: impl Into<String>,
        system: ModelSystem,
        source_specimen: impl Into<String>,
        passage: u32,
    ) -> Self {
        ModelIdentity {
            model: model.into(),
            system,
            source_specimen: source_specimen.into(),
            passage,
            verified_against_source: false,
        }
    }

    pub fn verified(mut self) -> Self {
        self.verified_against_source = true;
        self
    }
}

/// Which fidelity axes were measured, and at which passage.
///
/// Passage is part of the record because "generalizing one passage" is a named failure: fidelity
/// measured at passage 1 says nothing about passage 20.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FidelityEvidence {
    measured: BTreeSet<(FidelityAxis, u32)>,
}

impl FidelityEvidence {
    pub fn new() -> Self {
        FidelityEvidence::default()
    }

    pub fn measured(mut self, axis: FidelityAxis, passage: u32) -> Self {
        self.measured.insert((axis, passage));
        self
    }

    pub fn covers(&self, axis: FidelityAxis, passage: u32) -> bool {
        self.measured.contains(&(axis, passage))
    }
}

/// How many specimens were attempted and how many became models.
///
/// The counts are the caller's. No rate is computed against a threshold, because there is no
/// threshold: any shortfall means the panel is selected, and 1000 of 1001 is still a selected
/// panel — just one whose selection is easier to argue about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstablishmentCohort {
    pub attempted: usize,
    pub established: usize,
    /// Whether the analysis models the selection between the two counts.
    pub selection_modelled: bool,
}

impl EstablishmentCohort {
    pub fn new(attempted: usize, established: usize) -> Self {
        EstablishmentCohort {
            attempted,
            established,
            selection_modelled: false,
        }
    }

    pub fn with_selection_modelled(mut self) -> Self {
        self.selection_modelled = true;
        self
    }

    /// True when some attempted specimen did not become a model.
    pub fn is_selected(&self) -> bool {
        self.established < self.attempted
    }
}

/// Wells versus independent biological units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicateStructure {
    pub technical_wells: usize,
    pub biological_replicates: usize,
}

impl ReplicateStructure {
    /// The number of independent observations. Never the well count.
    pub fn effective_n(&self) -> usize {
        self.biological_replicates
    }
}

/// An effect observed in a model system. True about the model system, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelResult {
    pub model: ModelIdentity,
    /// What was observed, in the caller's words.
    pub effect: String,
    pub replicates: ReplicateStructure,
    /// The fidelity axes this effect's interpretation rests on.
    pub rests_on: BTreeSet<FidelityAxis>,
}

impl ModelResult {
    pub fn new(
        model: ModelIdentity,
        effect: impl Into<String>,
        replicates: ReplicateStructure,
    ) -> Self {
        ModelResult {
            model,
            effect: effect.into(),
            replicates,
            rests_on: BTreeSet::new(),
        }
    }

    pub fn resting_on(mut self, axis: FidelityAxis) -> Self {
        self.rests_on.insert(axis);
        self
    }

    /// A sentence that is true without any transport at all.
    pub fn as_stated(&self) -> String {
        format!(
            "in {} {} at passage {}, {} (n = {} biological replicates)",
            self.model.system.as_str(),
            self.model.model,
            self.model.passage,
            self.effect,
            self.replicates.effective_n()
        )
    }
}

/// A claim about patients, carrying everything that had to be stated to make it.
///
/// No public constructor and no `Deserialize`. The only way to obtain one is
/// [`transport_to_patients`], which is the point: a caller who has not stated the transport is
/// left holding a [`ModelResult`], whose type name says what it is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PatientRelevantClaim {
    result: ModelResult,
    cohort: EstablishmentCohort,
    transport: DeclaredTransport,
    claimed_n: usize,
}

impl PatientRelevantClaim {
    pub fn result(&self) -> &ModelResult {
        &self.result
    }

    pub fn establishment(&self) -> EstablishmentCohort {
        self.cohort
    }

    pub fn transport(&self) -> &DeclaredTransport {
        &self.transport
    }

    pub fn claimed_n(&self) -> usize {
        self.claimed_n
    }
}

/// Whether a model-system result may be stated as a claim about patients.
///
/// See the module header for the order and the reason for it.
pub fn transport_to_patients(
    result: &ModelResult,
    fidelity: &FidelityEvidence,
    cohort: EstablishmentCohort,
    claimed_n: usize,
    transport: &DeclaredTransport,
) -> Result<PatientRelevantClaim, TransportRefusal> {
    if !result.model.verified_against_source {
        return Err(TransportRefusal::UnverifiedModelIdentity {
            model: result.model.model.clone(),
            specimen: result.model.source_specimen.clone(),
        });
    }
    for axis in &result.rests_on {
        if !fidelity.covers(*axis, result.model.passage) {
            return Err(TransportRefusal::UnmeasuredFidelity {
                axis: format!("{} at passage {}", axis.as_str(), result.model.passage),
            });
        }
    }
    if cohort.is_selected() && !cohort.selection_modelled {
        return Err(TransportRefusal::UnmodelledEstablishmentSelection {
            attempted: cohort.attempted,
            established: cohort.established,
        });
    }
    if claimed_n > result.replicates.effective_n() {
        return Err(TransportRefusal::TechnicalReplicatesAsBiological {
            wells: result.replicates.technical_wells,
            claimed: claimed_n,
        });
    }
    transport.check(REQUIRED_ASSUMPTIONS)?;
    Ok(PatientRelevantClaim {
        result: result.clone(),
        cohort,
        transport: transport.clone(),
        claimed_n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_scope::ScopeKey;

    fn model() -> ModelIdentity {
        ModelIdentity::new("ORG-1", ModelSystem::Organoid, "S1", 3).verified()
    }

    fn replicates() -> ReplicateStructure {
        ReplicateStructure {
            technical_wells: 24,
            biological_replicates: 3,
        }
    }

    fn result() -> ModelResult {
        ModelResult::new(model(), "the compound reduced viability", replicates())
            .resting_on(FidelityAxis::Genomic)
    }

    fn fidelity() -> FidelityEvidence {
        FidelityEvidence::new().measured(FidelityAxis::Genomic, 3)
    }

    fn sound_transport() -> DeclaredTransport {
        let mut transport = DeclaredTransport::new(
            ScopeKey::new().exact("specimen", "S1"),
            ScopeKey::new().exact("cohort", "newly diagnosed"),
            "an ex vivo sensitivity stands for a population-level statement",
        )
        .losing("the microenvironment, immune compartment and blood-brain barrier")
        .adding_uncertainty("passage drift between establishment and assay");
        for assumption in REQUIRED_ASSUMPTIONS {
            transport = transport.assuming(*assumption, "stated by the study protocol");
        }
        transport
    }

    #[test]
    fn a_model_result_states_what_it_is_about_without_any_transport() {
        let stated = result().as_stated();
        assert!(stated.contains("organoid"));
        assert!(stated.contains("passage 3"));
        assert!(stated.contains("n = 3"));
    }

    #[test]
    fn an_unverified_model_is_not_known_to_be_the_patients_model() {
        let mut unverified = result();
        unverified.model.verified_against_source = false;
        let refusal = transport_to_patients(
            &unverified,
            &fidelity(),
            EstablishmentCohort::new(10, 10),
            3,
            &sound_transport(),
        )
        .unwrap_err();
        assert!(matches!(
            refusal,
            TransportRefusal::UnverifiedModelIdentity { .. }
        ));
    }

    #[test]
    fn a_claim_resting_on_an_unmeasured_fidelity_axis_is_refused() {
        let resting_on_more = result().resting_on(FidelityAxis::Transcriptomic);
        let refusal = transport_to_patients(
            &resting_on_more,
            &fidelity(),
            EstablishmentCohort::new(10, 10),
            3,
            &sound_transport(),
        )
        .unwrap_err();
        assert!(matches!(
            refusal,
            TransportRefusal::UnmeasuredFidelity { .. }
        ));
    }

    #[test]
    fn fidelity_measured_at_another_passage_does_not_cover_this_one() {
        let at_passage_one = FidelityEvidence::new().measured(FidelityAxis::Genomic, 1);
        assert!(!at_passage_one.covers(FidelityAxis::Genomic, 3));
        let refusal = transport_to_patients(
            &result(),
            &at_passage_one,
            EstablishmentCohort::new(10, 10),
            3,
            &sound_transport(),
        )
        .unwrap_err();
        assert!(matches!(
            refusal,
            TransportRefusal::UnmeasuredFidelity { .. }
        ));
    }

    #[test]
    fn a_drug_effect_in_an_established_panel_is_not_a_population_claim_while_selection_is_unmodelled(
    ) {
        let refusal = transport_to_patients(
            &result(),
            &fidelity(),
            EstablishmentCohort::new(40, 12),
            3,
            &sound_transport(),
        )
        .unwrap_err();
        assert_eq!(
            refusal,
            TransportRefusal::UnmodelledEstablishmentSelection {
                attempted: 40,
                established: 12
            }
        );
        assert!(transport_to_patients(
            &result(),
            &fidelity(),
            EstablishmentCohort::new(40, 12).with_selection_modelled(),
            3,
            &sound_transport(),
        )
        .is_ok());
    }

    #[test]
    fn any_shortfall_in_establishment_makes_the_panel_selected() {
        assert!(EstablishmentCohort::new(1001, 1000).is_selected());
        assert!(!EstablishmentCohort::new(10, 10).is_selected());
    }

    #[test]
    fn technical_wells_are_not_biological_replicates() {
        let refusal = transport_to_patients(
            &result(),
            &fidelity(),
            EstablishmentCohort::new(10, 10),
            24,
            &sound_transport(),
        )
        .unwrap_err();
        assert_eq!(
            refusal,
            TransportRefusal::TechnicalReplicatesAsBiological {
                wells: 24,
                claimed: 24
            }
        );
        assert_eq!(replicates().effective_n(), 3);
    }

    #[test]
    fn an_organoid_result_without_a_declared_transport_stays_a_model_result() {
        let bare = DeclaredTransport::new(
            ScopeKey::new().exact("specimen", "S1"),
            ScopeKey::new().exact("cohort", "newly diagnosed"),
            "justification",
        );
        let refusal = transport_to_patients(
            &result(),
            &fidelity(),
            EstablishmentCohort::new(10, 10),
            3,
            &bare,
        )
        .unwrap_err();
        assert!(matches!(refusal, TransportRefusal::UndeclaredLoss { .. }));
    }

    #[test]
    fn a_transport_missing_the_conditions_assumption_is_refused_by_name() {
        let mut partial = DeclaredTransport::new(
            ScopeKey::new().exact("specimen", "S1"),
            ScopeKey::new().exact("cohort", "newly diagnosed"),
            "justification",
        )
        .losing("the host microenvironment");
        partial = partial.assuming(REQUIRED_ASSUMPTIONS[0], "stated");
        let refusal = transport_to_patients(
            &result(),
            &fidelity(),
            EstablishmentCohort::new(10, 10),
            3,
            &partial,
        )
        .unwrap_err();
        assert_eq!(
            refusal,
            TransportRefusal::UnstatedAssumption {
                assumption: REQUIRED_ASSUMPTIONS[1].to_string()
            }
        );
    }

    #[test]
    fn a_transported_claim_keeps_the_establishment_counts_it_rested_on() {
        let claim = transport_to_patients(
            &result(),
            &fidelity(),
            EstablishmentCohort::new(40, 12).with_selection_modelled(),
            2,
            &sound_transport(),
        )
        .expect("every condition is stated");
        assert_eq!(claim.establishment().attempted, 40);
        assert_eq!(claim.claimed_n(), 2);
        assert!(!claim.transport().loss.is_empty());
    }
}
