//! Reproduction as a status that refuses to collapse, and an obligation ledger that gates it.
//!
//! Implements blueprint 19.12 (the scientific figure-reproduction case) and 19.22 (the scientific
//! reproduction capability molecule). They are one subject seen twice: 19.12 gives a worked
//! evidence world and the ten obligations a claim carries, and 19.22 gives the eight statuses the
//! molecule "never collapses into one" together with the effects it must never have.
//!
//! Two rules do the work here, and both are stated in the blueprint as prohibitions.
//!
//! **You may not conclude while an obligation is open.** 19.12's decision cell says an immediate
//! final conclusion is prohibited because the execution-configuration obligation is unresolved.
//! [`ReproductionReport::seal`] enforces exactly that: a verification status — reproduced,
//! directionally reproduced, not reproduced — is refused while any obligation is conflicted,
//! unresolved or missing. The statuses that remain available are the honest ones: the claim as
//! *reported*, the artifacts as *unsupported*, the evidence as *conflicted*.
//!
//! **A manuscript claim is not a reproduction result.** [`ReproductionStatus::Reported`] and
//! [`ReproductionStatus::Reproduced`] are different values and [`ReproductionStatus::is_verification`]
//! separates them. There is no `merge`, no `join` and no `to_bool` on the status enum, deliberately:
//! every one of those functions is a way to write the collapse 19.22 forbids, and the absence is
//! the design. [`summarise`] exists for the case where several sub-claims must be described at
//! once, and it returns [`ReproductionStatus::Conflicted`] rather than a winner.
//!
//! # What is not here
//!
//! No reproduction is performed. Nothing reruns a notebook, recomputes an AUROC, compares plot
//! data or diffs a cohort — `bioprism-oracle` and `bioprism-evalengine` own execution-grounded
//! evidence, and a second executor would be a second answer to "did it reproduce". This module is
//! the *bookkeeping* those results are reported through: which obligations are open, which status
//! is therefore available, and which effects the reporting agent was allowed to have.
//!
//! No statistics. The tolerance on a point estimate, the bootstrap interval and the plot-data
//! agreement test named in 19.12's oracle stack are all absent; a numeric tolerance invented here
//! would be a threshold nobody calibrated.
//!
//! No molecule runtime. 19.22's six bound roles, its choreography and its capability profile
//! belong to `bioprism-weave` and `bioprism-weavelang`. [`MoleculeCard`] is the card, not the
//! molecule: the declared effect envelope and the failure modes, which are the parts a reader
//! checks a report against.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::error::ReproError;

/// The ten evidence obligations 19.12's worked example enumerates.
///
/// Closed at ten because that is the list; a claim that needs an eleventh is a different kind of
/// claim, and silently widening the set would let a report look total while omitting something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Obligation {
    PopulationDefinition,
    ExclusionRule,
    Comparator,
    OutcomeDefinition,
    DataRevision,
    CodeRevision,
    ExecutionConfiguration,
    PointEstimate,
    UncertaintyInterval,
    FigureDataLink,
}

impl Obligation {
    pub const ALL: [Obligation; 10] = [
        Obligation::PopulationDefinition,
        Obligation::ExclusionRule,
        Obligation::Comparator,
        Obligation::OutcomeDefinition,
        Obligation::DataRevision,
        Obligation::CodeRevision,
        Obligation::ExecutionConfiguration,
        Obligation::PointEstimate,
        Obligation::UncertaintyInterval,
        Obligation::FigureDataLink,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Obligation::PopulationDefinition => "population_definition",
            Obligation::ExclusionRule => "exclusion_rule",
            Obligation::Comparator => "comparator",
            Obligation::OutcomeDefinition => "outcome_definition",
            Obligation::DataRevision => "data_revision",
            Obligation::CodeRevision => "code_revision",
            Obligation::ExecutionConfiguration => "execution_configuration",
            Obligation::PointEstimate => "point_estimate",
            Obligation::UncertaintyInterval => "uncertainty_interval",
            Obligation::FigureDataLink => "figure_data_link",
        }
    }
}

/// How an obligation stands.
///
/// Five states, from 19.12's own table. [`ObligationStatus::Reported`] is the interesting one: the
/// manuscript states a point estimate, which discharges nothing about whether it is right, and is
/// therefore neither resolved nor open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationStatus {
    /// The artifacts agree and the question is settled.
    Resolved,
    /// The artifacts disagree with each other.
    Conflicted,
    /// No artifact answers the question.
    Unresolved,
    /// Stated by the manuscript, and not independently established.
    Reported,
    /// The artifact that would answer it was not released.
    Missing,
}

impl ObligationStatus {
    pub const ALL: [ObligationStatus; 5] = [
        ObligationStatus::Resolved,
        ObligationStatus::Conflicted,
        ObligationStatus::Unresolved,
        ObligationStatus::Reported,
        ObligationStatus::Missing,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ObligationStatus::Resolved => "resolved",
            ObligationStatus::Conflicted => "conflicted",
            ObligationStatus::Unresolved => "unresolved",
            ObligationStatus::Reported => "reported",
            ObligationStatus::Missing => "missing",
        }
    }

    /// Whether this status prevents a verification conclusion.
    ///
    /// `Reported` does not block: a manuscript may state a figure without the reproduction being
    /// blocked on it, and 19.12's own table has `point_estimate: reported` in a case that
    /// nevertheless reaches "partially reproduced".
    pub fn blocks_conclusion(self) -> bool {
        matches!(
            self,
            ObligationStatus::Conflicted | ObligationStatus::Unresolved | ObligationStatus::Missing
        )
    }
}

/// The obligation table for one claim. Total over all ten obligations, by construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    try_from = "BTreeMap<Obligation, ObligationStatus>",
    into = "BTreeMap<Obligation, ObligationStatus>"
)]
pub struct ObligationLedger {
    entries: BTreeMap<Obligation, ObligationStatus>,
}

impl From<ObligationLedger> for BTreeMap<Obligation, ObligationStatus> {
    fn from(ledger: ObligationLedger) -> Self {
        ledger.entries
    }
}

impl ObligationLedger {
    /// Build a ledger. Refuses a partial table.
    ///
    /// Totality is the point: a ledger that may omit an obligation cannot distinguish "resolved"
    /// from "nobody asked", and the second is the more common state of a real manuscript.
    pub fn new(
        entries: impl IntoIterator<Item = (Obligation, ObligationStatus)>,
    ) -> Result<Self, ReproError> {
        let entries: BTreeMap<Obligation, ObligationStatus> = entries.into_iter().collect();
        for obligation in Obligation::ALL {
            if !entries.contains_key(&obligation) {
                return Err(ReproError::IncompleteLedger {
                    kind: obligation.as_str(),
                });
            }
        }
        Ok(ObligationLedger { entries })
    }

    /// Every obligation resolved. The starting point for a ledger a caller then weakens.
    pub fn all_resolved() -> Self {
        ObligationLedger {
            entries: Obligation::ALL
                .into_iter()
                .map(|obligation| (obligation, ObligationStatus::Resolved))
                .collect(),
        }
    }

    /// Set one obligation, returning the ledger. Totality is preserved because nothing is removed.
    pub fn with(mut self, obligation: Obligation, status: ObligationStatus) -> Self {
        self.entries.insert(obligation, status);
        self
    }

    pub fn status(&self, obligation: Obligation) -> ObligationStatus {
        self.entries
            .get(&obligation)
            .copied()
            .expect("the ledger is total by construction")
    }

    /// The obligations that prevent a verification conclusion, in a stable order.
    pub fn blocking(&self) -> Vec<Obligation> {
        Obligation::ALL
            .into_iter()
            .filter(|obligation| self.status(*obligation).blocks_conclusion())
            .collect()
    }

    pub fn is_dischargeable(&self) -> bool {
        self.blocking().is_empty()
    }
}

impl TryFrom<BTreeMap<Obligation, ObligationStatus>> for ObligationLedger {
    type Error = ReproError;

    fn try_from(value: BTreeMap<Obligation, ObligationStatus>) -> Result<Self, Self::Error> {
        ObligationLedger::new(value)
    }
}

/// The eight outcomes 19.22 says a reproduction molecule never collapses into one.
///
/// There is no ordering on this enum beyond the derived one, which exists only so it can sit in a
/// `BTreeSet`. Ranking them would imply a best and a worst, and "out of scope" is neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReproductionStatus {
    /// What the manuscript says. Not a result.
    Reported,
    /// Recomputed, and it matches within the stated tolerance.
    Reproduced,
    /// The direction holds; the magnitude does not.
    DirectionallyReproduced,
    /// Recomputed, and it does not match.
    NotReproduced,
    /// The released artifacts do not run.
    NotExecutable,
    /// The artifacts run and do not contain what the claim needs.
    UnsupportedByReleasedArtifacts,
    /// The artifacts disagree with each other, and no single answer is available.
    Conflicted,
    /// The claim is not the kind of thing this molecule evaluates.
    OutOfScope,
}

impl ReproductionStatus {
    pub const ALL: [ReproductionStatus; 8] = [
        ReproductionStatus::Reported,
        ReproductionStatus::Reproduced,
        ReproductionStatus::DirectionallyReproduced,
        ReproductionStatus::NotReproduced,
        ReproductionStatus::NotExecutable,
        ReproductionStatus::UnsupportedByReleasedArtifacts,
        ReproductionStatus::Conflicted,
        ReproductionStatus::OutOfScope,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ReproductionStatus::Reported => "reported",
            ReproductionStatus::Reproduced => "reproduced",
            ReproductionStatus::DirectionallyReproduced => "directionally reproduced",
            ReproductionStatus::NotReproduced => "not reproduced",
            ReproductionStatus::NotExecutable => "not executable",
            ReproductionStatus::UnsupportedByReleasedArtifacts => {
                "unsupported by released artifacts"
            }
            ReproductionStatus::Conflicted => "conflicted",
            ReproductionStatus::OutOfScope => "out of scope",
        }
    }

    /// Whether this status asserts something about a *recomputation* rather than about the paper.
    ///
    /// The three verification statuses are the ones an open obligation blocks. The other five say
    /// something about the artifacts or the claim, and remain available precisely when the
    /// evidence is too weak to verify anything.
    pub fn is_verification(self) -> bool {
        matches!(
            self,
            ReproductionStatus::Reproduced
                | ReproductionStatus::DirectionallyReproduced
                | ReproductionStatus::NotReproduced
        )
    }
}

/// Describe several sub-claims at once without picking a winner.
///
/// Returns the single status when they agree, [`ReproductionStatus::Conflicted`] when they do not,
/// and `None` for an empty set. This is the only many-to-one function on the status type, and it
/// is deliberately unable to produce a majority verdict: 19.22's rule is that the distinctions
/// survive, and a summariser that returned the most common status would erase them at the exact
/// moment they matter.
pub fn summarise(
    statuses: impl IntoIterator<Item = ReproductionStatus>,
) -> Option<ReproductionStatus> {
    let set: BTreeSet<ReproductionStatus> = statuses.into_iter().collect();
    match set.len() {
        0 => None,
        1 => set.into_iter().next(),
        _ => Some(ReproductionStatus::Conflicted),
    }
}

/// A reproduction result, which cannot claim more than its obligations allow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ReproductionReportWire")]
pub struct ReproductionReport {
    claim: String,
    status: ReproductionStatus,
    ledger: ObligationLedger,
    /// The precise disagreement, when there is one. 19.12's expected report leads with it.
    discrepancy: Option<String>,
}

impl ReproductionReport {
    /// The only constructor. Refuses a verification status under an open obligation.
    pub fn seal(
        claim: impl Into<String>,
        status: ReproductionStatus,
        ledger: ObligationLedger,
        discrepancy: Option<String>,
    ) -> Result<Self, ReproError> {
        let claim: String = claim.into();
        if claim.trim().is_empty() {
            return Err(ReproError::NoClaim);
        }
        let blocking = ledger.blocking();
        if status.is_verification() && !blocking.is_empty() {
            return Err(ReproError::ConcludedUnderOpenObligation {
                status: status.as_str(),
                blocking: blocking
                    .into_iter()
                    .map(Obligation::as_str)
                    .collect::<Vec<_>>(),
            });
        }
        Ok(ReproductionReport {
            claim,
            status,
            ledger,
            discrepancy,
        })
    }

    pub fn claim(&self) -> &str {
        &self.claim
    }

    pub fn status(&self) -> ReproductionStatus {
        self.status
    }

    pub fn ledger(&self) -> &ObligationLedger {
        &self.ledger
    }

    pub fn discrepancy(&self) -> Option<&str> {
        self.discrepancy.as_deref()
    }

    /// The statuses this report *could* have carried, given its ledger.
    ///
    /// Useful to a reviewer asking whether a weak conclusion was forced by the evidence or chosen.
    pub fn available_statuses(&self) -> Vec<ReproductionStatus> {
        let open = !self.ledger.is_dischargeable();
        ReproductionStatus::ALL
            .into_iter()
            .filter(|status| !(open && status.is_verification()))
            .collect()
    }
}

#[derive(Deserialize)]
struct ReproductionReportWire {
    claim: String,
    status: ReproductionStatus,
    ledger: ObligationLedger,
    #[serde(default)]
    discrepancy: Option<String>,
}

impl TryFrom<ReproductionReportWire> for ReproductionReport {
    type Error = ReproError;

    fn try_from(wire: ReproductionReportWire) -> Result<Self, Self::Error> {
        ReproductionReport::seal(wire.claim, wire.status, wire.ledger, wire.discrepancy)
    }
}

/// An effect a molecule may have on the world.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Effect(String);

impl Effect {
    pub fn new(name: impl Into<String>) -> Self {
        Effect(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The two effects 19.22's molecule card forbids by name.
pub fn forbidden_by_default() -> Vec<Effect> {
    vec![Effect::new("patient.advice"), Effect::new("external.publish")]
}

/// The declared envelope of a reproduction molecule: what it needs, what it must never do.
///
/// This is the card, not the molecule. Nothing here binds a role, runs a choreography or executes
/// anything; the value of the card is that a report can be checked against it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MoleculeCardWire")]
pub struct MoleculeCard {
    name: String,
    required: BTreeSet<Effect>,
    forbidden: BTreeSet<Effect>,
    /// Failure modes the card admits up front. 19.22 lists two.
    known_failures: Vec<String>,
}

impl MoleculeCard {
    /// Refuses a card whose required and forbidden sets overlap, and a card that declares nothing.
    pub fn seal(
        name: impl Into<String>,
        required: impl IntoIterator<Item = Effect>,
        forbidden: impl IntoIterator<Item = Effect>,
        known_failures: Vec<String>,
    ) -> Result<Self, ReproError> {
        let name: String = name.into();
        let required: BTreeSet<Effect> = required.into_iter().collect();
        let forbidden: BTreeSet<Effect> = forbidden.into_iter().collect();
        if required.is_empty() && forbidden.is_empty() {
            return Err(ReproError::NoEffectsDeclared { molecule: name });
        }
        if let Some(effect) = required.intersection(&forbidden).next() {
            return Err(ReproError::EffectBothRequiredAndForbidden {
                effect: effect.as_str().to_string(),
                molecule: name,
            });
        }
        Ok(MoleculeCard {
            name,
            required,
            forbidden,
            known_failures,
        })
    }

    /// The card 19.22 prints, with its two required effects and its two forbidden ones.
    pub fn paper_reproducer() -> Result<Self, ReproError> {
        MoleculeCard::seal(
            "paper-reproducer",
            [
                Effect::new("artifact.read"),
                Effect::new("sandbox.execute"),
            ],
            forbidden_by_default(),
            vec![
                "proprietary-data-unavailable".to_string(),
                "non-deterministic-external-service".to_string(),
            ],
        )
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn required(&self) -> &BTreeSet<Effect> {
        &self.required
    }

    pub fn forbidden(&self) -> &BTreeSet<Effect> {
        &self.forbidden
    }

    pub fn known_failures(&self) -> &[String] {
        &self.known_failures
    }

    /// Whether an observed effect is inside the envelope.
    ///
    /// An effect that is neither required nor forbidden is *not* permitted. The default is closed
    /// because 19.22 writes the envelope as `effects <= [artifact.read, sandbox.execute]`, which
    /// is an upper bound, and an open default would make the bound decorative.
    pub fn permits(&self, effect: &Effect) -> bool {
        self.required.contains(effect)
    }

    /// Check an observed effect set against the card, returning the first violation.
    pub fn check(&self, observed: impl IntoIterator<Item = Effect>) -> Result<(), ReproError> {
        for effect in observed {
            if !self.permits(&effect) {
                return Err(ReproError::EffectNotPermitted {
                    molecule: self.name.clone(),
                    effect: effect.as_str().to_string(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct MoleculeCardWire {
    name: String,
    required: BTreeSet<Effect>,
    forbidden: BTreeSet<Effect>,
    #[serde(default)]
    known_failures: Vec<String>,
}

impl TryFrom<MoleculeCardWire> for MoleculeCard {
    type Error = ReproError;

    fn try_from(wire: MoleculeCardWire) -> Result<Self, Self::Error> {
        MoleculeCard::seal(wire.name, wire.required, wire.forbidden, wire.known_failures)
    }
}

/// The ledger 19.12's worked case actually reports: two open obligations, one of them a conflict.
///
/// Supplied as a fixture because it is the case the module is about, and because a test asserting
/// "this ledger admits no verification status" is a test of the rule rather than of an invented
/// example.
pub fn figure_reproduction_case() -> ObligationLedger {
    ObligationLedger::all_resolved()
        .with(Obligation::ExclusionRule, ObligationStatus::Conflicted)
        .with(
            Obligation::ExecutionConfiguration,
            ObligationStatus::Unresolved,
        )
        .with(Obligation::PointEstimate, ObligationStatus::Reported)
        .with(Obligation::UncertaintyInterval, ObligationStatus::Missing)
        .with(Obligation::FigureDataLink, ObligationStatus::Unresolved)
}
