//! The biological scale and capability lattice.
//!
//! Blueprint 24.05 ends with a section titled "Why no single score", and the argument is
//! concrete: "An agent that excels at protein sequence interpretation may be untested in
//! longitudinal MRI." Untested. Not bad — untested. A profile that averages over cells silently
//! converts an absence of measurement into a middling number, and the number is then compared
//! against a system that was measured everywhere.
//!
//! So [`CapabilityProfile::overall_score`] does not return a number. It returns an error, always,
//! naming how many cells were about to be averaged. And [`CapabilityProfile::at`] distinguishes
//! "measured, and it went badly" from "never measured" with two different errors.
//!
//! Not implemented: difficulty curves and Pareto fronts, which 24.05 also asks the hub to
//! publish. Both are statistics over many results, and a foundation crate that computed them
//! would be guessing at the shape of the result stream.

use crate::error::CapabilityError;
use crate::worldclass::WorldClass;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The scale axis of 24.05, coarsest resolution first.
///
/// Ordering here is spatial containment as the blueprint lists it. 24.05 warns that "edges are
/// not assumed to be reversible" and that evidence at one scale may constrain another "without
/// uniquely determining it", so adjacency in this list licenses nothing — crossing scales is
/// the Translation Spine's job, and it requires a certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scale {
    Molecule,
    MacromolecularComplex,
    Organelle,
    Cell,
    CellState,
    MicroenvironmentOrNiche,
    Tissue,
    OrganOrSystem,
    OrganismOrPatient,
    CohortOrPopulation,
    Ecosystem,
}

impl Scale {
    pub const ALL: [Scale; 11] = [
        Scale::Molecule,
        Scale::MacromolecularComplex,
        Scale::Organelle,
        Scale::Cell,
        Scale::CellState,
        Scale::MicroenvironmentOrNiche,
        Scale::Tissue,
        Scale::OrganOrSystem,
        Scale::OrganismOrPatient,
        Scale::CohortOrPopulation,
        Scale::Ecosystem,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Scale::Molecule => "molecule",
            Scale::MacromolecularComplex => "macromolecular complex",
            Scale::Organelle => "organelle",
            Scale::Cell => "cell",
            Scale::CellState => "cell state",
            Scale::MicroenvironmentOrNiche => "microenvironment or niche",
            Scale::Tissue => "tissue",
            Scale::OrganOrSystem => "organ or system",
            Scale::OrganismOrPatient => "organism or patient",
            Scale::CohortOrPopulation => "cohort or population",
            Scale::Ecosystem => "ecosystem",
        }
    }
}

/// The activity axis of 24.05.
///
/// The axis matters because systems that share a domain do not share an activity: a model that
/// predicts labels "may have no capacity to select an experiment". Reporting both under one
/// heading is the collapse this axis exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    Observe,
    Identify,
    Quantify,
    Compare,
    Explain,
    Predict,
    Design,
    Execute,
    Verify,
    Communicate,
    Govern,
}

impl Activity {
    pub const ALL: [Activity; 11] = [
        Activity::Observe,
        Activity::Identify,
        Activity::Quantify,
        Activity::Compare,
        Activity::Explain,
        Activity::Predict,
        Activity::Design,
        Activity::Execute,
        Activity::Verify,
        Activity::Communicate,
        Activity::Govern,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Activity::Observe => "observe",
            Activity::Identify => "identify",
            Activity::Quantify => "quantify",
            Activity::Compare => "compare",
            Activity::Explain => "explain",
            Activity::Predict => "predict",
            Activity::Design => "design",
            Activity::Execute => "execute",
            Activity::Verify => "verify",
            Activity::Communicate => "communicate",
            Activity::Govern => "govern",
        }
    }
}

/// One cell of the capability lattice (24.05, "capability cell").
///
/// Modality and resource regime are free strings rather than enums. 24.05's modality list is
/// explicitly open-ended and its resource-regime example ("one remaining tissue aliquot") is
/// prose; freezing either into an enum here would make adding a modality a breaking change to
/// a foundation crate, which is the wrong place for that cost to land.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CapabilityCell {
    pub activity: Activity,
    pub scale: Scale,
    pub modality: String,
    pub world_class: WorldClass,
    pub resource_regime: String,
}

impl CapabilityCell {
    pub fn new(
        activity: Activity,
        scale: Scale,
        modality: impl Into<String>,
        world_class: WorldClass,
        resource_regime: impl Into<String>,
    ) -> Self {
        CapabilityCell {
            activity,
            scale,
            modality: modality.into(),
            world_class,
            resource_regime: resource_regime.into(),
        }
    }
}

/// What was actually measured in one cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellEvidence {
    /// How many distinct generated instances contributed. Zero means the cell was declared but
    /// never populated, which is not evidence.
    pub instances: usize,
    pub trials: usize,
    /// The measured value in whatever units the cell's oracle uses. No range is imposed: this
    /// crate does not know whether the cell reports a Dice coefficient or a cost in aliquots.
    pub value: f64,
    pub summary: String,
}

/// A system's evidence profile across the lattice.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CapabilityProfile {
    pub system: String,
    cells: BTreeMap<CapabilityCell, CellEvidence>,
}

impl CapabilityProfile {
    pub fn new(system: impl Into<String>) -> Self {
        CapabilityProfile {
            system: system.into(),
            cells: BTreeMap::new(),
        }
    }

    pub fn record(mut self, cell: CapabilityCell, evidence: CellEvidence) -> Self {
        self.cells.insert(cell, evidence);
        self
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Reads one cell, distinguishing "never measured" from "measured badly".
    ///
    /// Both are refusals, and they are different refusals, because the correct response to the
    /// first is to run the benchmark and the correct response to the second is to look at the
    /// system.
    pub fn at(&self, cell: &CapabilityCell) -> Result<&CellEvidence, CapabilityError> {
        match self.cells.get(cell) {
            None => Err(CapabilityError::Untested {
                activity: cell.activity.as_str(),
                scale: cell.scale.as_str(),
            }),
            Some(evidence) if evidence.instances == 0 || evidence.trials == 0 => {
                Err(CapabilityError::UnevidencedCell {
                    activity: cell.activity.as_str(),
                    scale: cell.scale.as_str(),
                })
            }
            Some(evidence) => Ok(evidence),
        }
    }

    /// There is no overall biological intelligence number.
    ///
    /// This function exists so that the request has somewhere to go and comes back with a
    /// reason. Deleting it would mean the first caller who wants a leaderboard column writes
    /// their own mean over `cells` and nobody ever sees a refusal.
    pub fn overall_score(&self) -> Result<f64, CapabilityError> {
        Err(CapabilityError::ScalarCollapse {
            cells: self.cells.len(),
        })
    }

    /// Cells that were measured, as a vector — the shape 24.05 says to publish instead.
    pub fn vector(&self) -> Vec<(&CapabilityCell, &CellEvidence)> {
        self.cells.iter().collect()
    }

    /// Cells of `candidates` this profile has no evidence for. The honest denominator for a
    /// comparison: two systems are only comparable across the cells they were both measured on.
    pub fn untested_among<'a>(&self, candidates: &'a [CapabilityCell]) -> Vec<&'a CapabilityCell> {
        candidates
            .iter()
            .filter(|cell| self.at(cell).is_err())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(activity: Activity, scale: Scale) -> CapabilityCell {
        CapabilityCell::new(
            activity,
            scale,
            "single-cell RNA + spatial transcriptomics",
            WorldClass::ObservedReplay,
            "one remaining tissue aliquot",
        )
    }

    fn evidence() -> CellEvidence {
        CellEvidence {
            instances: 40,
            trials: 120,
            value: 0.71,
            summary: "selects the informative assay in 71% of instances".to_string(),
        }
    }

    fn profile() -> CapabilityProfile {
        CapabilityProfile::new("triage-agent@0.2").record(
            cell(Activity::Design, Scale::MicroenvironmentOrNiche),
            evidence(),
        )
    }

    #[test]
    fn a_capability_profile_refuses_to_produce_an_overall_biological_intelligence_number() {
        let err = profile().overall_score().unwrap_err();
        assert_eq!(err, CapabilityError::ScalarCollapse { cells: 1 });
    }

    #[test]
    fn an_unmeasured_cell_reports_as_untested_rather_than_as_a_low_score() {
        let err = profile()
            .at(&cell(Activity::Predict, Scale::OrganismOrPatient))
            .unwrap_err();
        assert_eq!(
            err,
            CapabilityError::Untested {
                activity: "predict",
                scale: "organism or patient"
            }
        );
    }

    #[test]
    fn a_declared_cell_with_zero_instances_is_unevidenced_not_measured() {
        let empty = CellEvidence {
            instances: 0,
            trials: 0,
            value: 0.0,
            summary: "declared".to_string(),
        };
        let profile = CapabilityProfile::new("x")
            .record(cell(Activity::Verify, Scale::Tissue), empty);
        assert_eq!(
            profile.at(&cell(Activity::Verify, Scale::Tissue)).unwrap_err(),
            CapabilityError::UnevidencedCell {
                activity: "verify",
                scale: "tissue"
            }
        );
    }

    #[test]
    fn a_measured_cell_returns_its_evidence_including_how_many_trials_stood_behind_it() {
        let evidence = profile()
            .at(&cell(Activity::Design, Scale::MicroenvironmentOrNiche))
            .unwrap()
            .clone();
        assert_eq!(evidence.instances, 40);
        assert_eq!(evidence.trials, 120);
    }

    #[test]
    fn the_same_activity_at_a_different_scale_is_a_different_cell() {
        let profile = profile();
        assert!(profile
            .at(&cell(Activity::Design, Scale::MicroenvironmentOrNiche))
            .is_ok());
        assert!(profile
            .at(&cell(Activity::Design, Scale::OrganismOrPatient))
            .is_err());
    }

    #[test]
    fn the_same_cell_in_a_different_world_class_is_a_different_cell() {
        let mut other = cell(Activity::Design, Scale::MicroenvironmentOrNiche);
        other.world_class = WorldClass::MechanisticSimulator;
        assert!(profile().at(&other).is_err());
    }

    #[test]
    fn comparing_two_systems_starts_by_naming_the_cells_one_of_them_was_never_measured_on() {
        let candidates = [
            cell(Activity::Design, Scale::MicroenvironmentOrNiche),
            cell(Activity::Predict, Scale::OrganismOrPatient),
            cell(Activity::Explain, Scale::Molecule),
        ];
        let untested = profile().untested_among(&candidates);
        assert_eq!(untested.len(), 2);
        assert!(untested
            .iter()
            .all(|cell| cell.activity != Activity::Design));
    }

    #[test]
    fn the_two_axes_have_eleven_positions_each_as_the_blueprint_lists_them() {
        assert_eq!(Scale::ALL.len(), 11);
        assert_eq!(Activity::ALL.len(), 11);
    }
}
