//! Perturbation, rescue, and causal oracles (31.08), and the interventional mutations that probe
//! them (32.17).
//!
//! 31.08's worked case: "A knockdown phenotype that disappears with a second reagent and cannot be
//! rescued does not establish the proposed target mechanism." Note what it does *not* say — it does
//! not say the phenotype was not observed. The first reagent's phenotype is real; what fails is the
//! inference from it to a mechanism. So the unit of decision here is a ([`ClaimPlane`], evidence)
//! pair, and the same experiment supports one plane while contradicting another.
//!
//! 32.17's worked relation is the same structure from the other direction: "A gene-expression
//! signature predicts response but intervention on its top gene has no effect in the simulator; the
//! benchmark separates prediction from target validity." [`ClaimPlane::Prediction`] and
//! [`ClaimPlane::Target`] are that separation, and [`decide`] will not let observational evidence
//! reach the interventional planes at all.
//!
//! # Controls are a precondition on the plane, not a checklist
//!
//! [`ClaimPlane::required_controls`] states which of positive, negative and vehicle each plane needs.
//! A missing control makes the *claim* unresolved and names the control; a protocol that declared the
//! control and did not run it is a different failure, and [`controls_complete`] reports that one as a
//! contradiction with a [`Witness::ControlAbsent`]. The two are separate because one is a gap in
//! knowledge and the other is a defect in an artifact, and 31.05's failure containment depends on
//! never confusing those.
//!
//! # Positivity
//!
//! 32.17's failure risks list "positivity violation" second. [`positivity`] takes per-stratum exposed
//! and unexposed counts and returns [`Determination::Unresolved`] naming every stratum where one arm
//! is empty. A stratum with no unexposed units has no counterfactual, and an estimator that runs
//! anyway has extrapolated without saying so.
//!
//! # Not implemented
//!
//! No effect estimation, no dose-response fitting, no off-target scoring, no causal graph. 31.08's
//! "measure context and dose response" and its "causal effect recovery" metric need the measurements;
//! this module decides what a set of already-summarised experiments licenses. The counterfactual
//! oracle of 32.17's validation program — a generator with known potential outcomes — belongs to a
//! simulation crate, not here.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_oracle::EvidenceTier;
use serde::{Deserialize, Serialize};

use crate::verdict::{Determination, Missing, Unresolved, Witness};

/// The three controls 31.08 requires perturbation experiments to declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    /// A perturbation known to produce the phenotype: shows the assay can see it.
    Positive,
    /// A perturbation known not to: shows the assay is not seeing everything.
    Negative,
    /// The delivery without the agent: separates the agent from its vehicle.
    Vehicle,
}

impl ControlKind {
    pub const ALL: [ControlKind; 3] = [
        ControlKind::Positive,
        ControlKind::Negative,
        ControlKind::Vehicle,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ControlKind::Positive => "positive",
            ControlKind::Negative => "negative",
            ControlKind::Vehicle => "vehicle",
        }
    }
}

/// How a perturbation was delivered.
///
/// Two reagents of the same modality share their modality's off-target profile; 31.08's "use multiple
/// reagents **or modalities**" is satisfied more strongly by two modalities than by two guides.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Reagent {
    pub id: String,
    pub modality: String,
}

impl Reagent {
    pub fn new(id: impl Into<String>, modality: impl Into<String>) -> Self {
        Reagent {
            id: id.into(),
            modality: modality.into(),
        }
    }
}

/// Whether re-expressing the target restored the unperturbed state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rescue {
    Rescued,
    NotRescued,
    /// Nobody tried. Distinct from `NotRescued`, and the distinction is the difference between
    /// evidence against a mechanism and no evidence about it.
    NotAttempted,
}

/// Where the evidence came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    /// Units were assigned to the perturbation by the experimenter.
    Interventional,
    /// Exposure was observed as it occurred.
    Observational,
}

/// The claim a caller wants the evidence to license.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimPlane {
    /// The measured quantity forecasts the outcome. Observational evidence reaches this.
    Prediction,
    /// Perturbing produced the phenotype. Needs an intervention and a vehicle control.
    Phenotype,
    /// The phenotype runs through the named target. Needs reagent agreement across modalities.
    Target,
    /// The named pathway carries the effect. Needs rescue or epistasis on top of target evidence.
    Mechanism,
}

impl ClaimPlane {
    pub fn as_str(self) -> &'static str {
        match self {
            ClaimPlane::Prediction => "prediction",
            ClaimPlane::Phenotype => "phenotype",
            ClaimPlane::Target => "target",
            ClaimPlane::Mechanism => "mechanism",
        }
    }

    /// Whether this plane can be reached without assigning the perturbation.
    pub fn needs_intervention(self) -> bool {
        !matches!(self, ClaimPlane::Prediction)
    }

    /// The controls this plane requires.
    pub fn required_controls(self) -> BTreeSet<ControlKind> {
        match self {
            ClaimPlane::Prediction => BTreeSet::new(),
            ClaimPlane::Phenotype => [ControlKind::Vehicle, ControlKind::Negative]
                .into_iter()
                .collect(),
            ClaimPlane::Target | ClaimPlane::Mechanism => ControlKind::ALL.into_iter().collect(),
        }
    }

    /// How many independent reagents must reproduce the phenotype.
    ///
    /// One for the phenotype itself, two for anything that names a target. Not a tunable: 31.08's
    /// "use multiple reagents or modalities" and its worked case both turn on the second reagent, and
    /// a configurable minimum of one would let a caller opt out of the case the oracle exists for.
    pub fn required_agreeing_reagents(self) -> usize {
        match self {
            ClaimPlane::Prediction => 0,
            ClaimPlane::Phenotype => 1,
            ClaimPlane::Target | ClaimPlane::Mechanism => 2,
        }
    }
}

/// One perturbation experiment, already summarised.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerturbationEvidence {
    pub target: String,
    pub source: EvidenceSource,
    pub controls_run: BTreeSet<ControlKind>,
    /// Controls the protocol declared, whether or not they were run.
    pub controls_declared: BTreeSet<ControlKind>,
    /// Whether each reagent reproduced the phenotype.
    phenotype_by_reagent: BTreeMap<Reagent, bool>,
    pub rescue: Rescue,
}

impl PerturbationEvidence {
    pub fn new(target: impl Into<String>, source: EvidenceSource) -> Self {
        PerturbationEvidence {
            target: target.into(),
            source,
            controls_run: BTreeSet::new(),
            controls_declared: BTreeSet::new(),
            phenotype_by_reagent: BTreeMap::new(),
            rescue: Rescue::NotAttempted,
        }
    }

    /// Records a control as both declared and run.
    pub fn with_control(mut self, control: ControlKind) -> Self {
        self.controls_declared.insert(control);
        self.controls_run.insert(control);
        self
    }

    /// Records a control the protocol promised and the experiment did not deliver.
    pub fn with_declared_but_unrun_control(mut self, control: ControlKind) -> Self {
        self.controls_declared.insert(control);
        self
    }

    pub fn with_reagent(mut self, reagent: Reagent, phenotype: bool) -> Self {
        self.phenotype_by_reagent.insert(reagent, phenotype);
        self
    }

    pub fn with_rescue(mut self, rescue: Rescue) -> Self {
        self.rescue = rescue;
        self
    }

    /// Reagents that reproduced the phenotype.
    pub fn agreeing_reagents(&self) -> Vec<&Reagent> {
        self.phenotype_by_reagent
            .iter()
            .filter(|(_, phenotype)| **phenotype)
            .map(|(reagent, _)| reagent)
            .collect()
    }

    /// Reagents that did not.
    pub fn dissenting_reagents(&self) -> Vec<&Reagent> {
        self.phenotype_by_reagent
            .iter()
            .filter(|(_, phenotype)| !**phenotype)
            .map(|(reagent, _)| reagent)
            .collect()
    }

    /// Distinct modalities among the reagents that reproduced the phenotype.
    pub fn agreeing_modalities(&self) -> BTreeSet<&str> {
        self.agreeing_reagents()
            .into_iter()
            .map(|reagent| reagent.modality.as_str())
            .collect()
    }
}

/// Whether every control the protocol declared was actually run.
///
/// A defect in the artifact, reported as a contradiction with a witness naming the control. Distinct
/// from a plane that needs a control nobody promised, which [`decide`] reports as unresolved.
pub fn controls_complete(evidence: &PerturbationEvidence) -> Determination {
    if evidence.controls_declared.is_empty() {
        return Determination::not_evaluable("the protocol declared no controls to check against");
    }
    let missing: Vec<ControlKind> = evidence
        .controls_declared
        .difference(&evidence.controls_run)
        .copied()
        .collect();
    match missing.first() {
        Some(control) => Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::ControlAbsent {
                claim: format!("protocol for {}", evidence.target),
                control: control.as_str().to_string(),
            },
        ),
        None => Determination::supported(
            EvidenceTier::Deterministic,
            "every declared control was run",
        ),
    }
}

/// Decides what one experiment licenses on one plane.
pub fn decide(plane: ClaimPlane, evidence: &PerturbationEvidence) -> Determination {
    if plane.needs_intervention() && evidence.source == EvidenceSource::Observational {
        return Determination::not_evaluable(
            "observational evidence cannot reach an interventional plane, at any effect size",
        );
    }

    if plane == ClaimPlane::Prediction {
        return Determination::supported(
            EvidenceTier::Statistical,
            format!("a predictive association was recorded for {}", evidence.target),
        );
    }

    let missing_controls: Vec<Missing> = plane
        .required_controls()
        .difference(&evidence.controls_run)
        .map(|control| {
            Missing::new(
                format!("{} control", control.as_str()),
                format!(
                    "the {} plane cannot be reached without it",
                    plane.as_str()
                ),
            )
        })
        .collect();
    if !missing_controls.is_empty() {
        return Determination::Unresolved(
            Unresolved::new(missing_controls).expect("the vector was checked non-empty"),
        );
    }

    let agreeing = evidence.agreeing_reagents().len();
    let dissenting = evidence.dissenting_reagents();

    if plane == ClaimPlane::Phenotype {
        return if agreeing >= plane.required_agreeing_reagents() {
            Determination::supported(
                EvidenceTier::Execution,
                format!(
                    "{agreeing} reagent(s) reproduced the phenotype for {} under vehicle and negative controls",
                    evidence.target
                ),
            )
        } else {
            Determination::unresolved(
                "a reagent reproducing the phenotype",
                format!("no reagent produced the phenotype for {}", evidence.target),
            )
        };
    }

    if agreeing < plane.required_agreeing_reagents() {
        if !dissenting.is_empty() && evidence.rescue == Rescue::NotRescued {
            return Determination::contradicted(
                EvidenceTier::Execution,
                Witness::RelationViolated {
                    relation: format!("{} claim for {}", plane.as_str(), evidence.target),
                    expected: format!(
                        "{} agreeing reagents and a rescue",
                        plane.required_agreeing_reagents()
                    ),
                    observed: format!(
                        "{agreeing} agreeing, {} dissenting, rescue attempted and failed",
                        dissenting.len()
                    ),
                },
            );
        }
        let mut missing = vec![Missing::new(
            "a second independent reagent reproducing the phenotype",
            format!(
                "{agreeing} of the required {} reagents agree",
                plane.required_agreeing_reagents()
            ),
        )];
        if evidence.rescue == Rescue::NotAttempted {
            missing.push(Missing::new(
                "a rescue experiment",
                "no rescue was attempted, so a failed rescue cannot be distinguished from an untried one",
            ));
        }
        return Determination::Unresolved(
            Unresolved::new(missing).expect("the vector was checked non-empty"),
        );
    }

    if evidence.agreeing_modalities().len() < 2 {
        return Determination::unresolved(
            "a reagent of a second modality",
            "every agreeing reagent shares one modality, so they share its off-target profile",
        );
    }

    match (plane, evidence.rescue) {
        (ClaimPlane::Target, _) => Determination::supported(
            EvidenceTier::Execution,
            format!(
                "{agreeing} reagents across {} modalities reproduced the phenotype for {}",
                evidence.agreeing_modalities().len(),
                evidence.target
            ),
        ),
        (ClaimPlane::Mechanism, Rescue::Rescued) => Determination::supported(
            EvidenceTier::Execution,
            format!(
                "multi-modality reagent agreement plus rescue of {}",
                evidence.target
            ),
        ),
        (ClaimPlane::Mechanism, Rescue::NotRescued) => Determination::contradicted(
            EvidenceTier::Execution,
            Witness::RelationViolated {
                relation: format!("mechanism claim for {}", evidence.target),
                expected: "re-expression restores the unperturbed state".to_string(),
                observed: "rescue was attempted and failed".to_string(),
            },
        ),
        (ClaimPlane::Mechanism, Rescue::NotAttempted) => Determination::unresolved(
            "a rescue or epistasis experiment",
            "reagent agreement establishes the target, not the pathway carrying the effect",
        ),
        (ClaimPlane::Prediction | ClaimPlane::Phenotype, _) => {
            unreachable!("both planes returned above")
        }
    }
}

/// Per-stratum exposure counts, for the positivity check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stratum {
    pub label: String,
    pub exposed: u64,
    pub unexposed: u64,
}

impl Stratum {
    pub fn new(label: impl Into<String>, exposed: u64, unexposed: u64) -> Self {
        Stratum {
            label: label.into(),
            exposed,
            unexposed,
        }
    }

    pub fn has_both_arms(&self) -> bool {
        self.exposed > 0 && self.unexposed > 0
    }
}

/// Whether every stratum contains both arms.
///
/// A stratum with an empty arm has no counterfactual: an estimate for it is an extrapolation from
/// strata that do. Returned as unresolved naming each offending stratum, because the fix is data and
/// the harm is a number that looks like an estimate.
pub fn positivity(strata: &[Stratum]) -> Determination {
    if strata.is_empty() {
        return Determination::not_evaluable("no strata were supplied");
    }
    let violations: Vec<Missing> = strata
        .iter()
        .filter(|stratum| !stratum.has_both_arms())
        .map(|stratum| {
            Missing::new(
                format!("units in the empty arm of stratum '{}'", stratum.label),
                format!(
                    "stratum '{}' has {} exposed and {} unexposed",
                    stratum.label, stratum.exposed, stratum.unexposed
                ),
            )
        })
        .collect();
    match Unresolved::new(violations) {
        Ok(unresolved) => Determination::Unresolved(unresolved),
        Err(_) => Determination::supported(
            EvidenceTier::Deterministic,
            "every stratum contains both an exposed and an unexposed arm",
        ),
    }
}
