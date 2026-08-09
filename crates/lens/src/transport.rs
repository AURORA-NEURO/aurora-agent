//! Causal assumptions and transport — blueprint 42.12.
//!
//! 42.12 asks to "expose causal assumptions, confounding, selection, mediation, interventions,
//! target populations, and transport requirements". Exposing an assumption is easy; the useful
//! part is refusing to let an *unexamined* assumption pass for a satisfied one.
//!
//! [`AssumptionStatus`] has four arms and the ordering between them is the whole module.
//! `Justified` carries the artifact that justifies it. `Asserted` is a human saying so, which is
//! a claim rather than a check. `Violated` carries a witness. `Unexamined` means nobody looked —
//! and [`transport_status`] returns [`OracleStatus::Underdetermined`] whenever one is present,
//! never `Valid`. This is `InfluenceClass::Unknown` wearing a causal hat: a transport conclusion
//! that rests on an assumption nobody checked is not a weak conclusion, it is not a conclusion.
//!
//! # Positivity is the one that yields a concrete witness
//!
//! Most causal assumptions are untestable from data alone. Positivity is not: a target stratum
//! with zero source support is a fact you can point at, and
//! [`TransportFinding::PositivityViolation`] names the stratum and its counts. That is the house
//! pattern — a checkable object, not a score — and it is why this lens takes stratum support as
//! input at all.
//!
//! # Not implemented
//!
//! **No identification algorithm.** There is no do-calculus here, no backdoor search, no graph.
//! 42.12 names "confounding, selection, mediation" but specifies no causal graph representation
//! anywhere in section 42, and inventing one to run identification on would be a different
//! project wearing this module's id. This lens checks the *status* of declared assumptions and
//! the *support* of declared strata; it does not derive which assumptions are needed.
//!
//! **No effect estimation and no transport formula.** Reweighting requires the estimator layer.
//! **No mediation decomposition** for the same reason.

use crate::grammar::{
    Coverage, EvidenceRequirement, Lens, LensDeclaration, LensId, LensOutcome, Refusal,
    RefusalReason, ScopePrecondition,
};
use crate::nonvisual::{Cell, Witness};
use bioprism_scope::ScopeKey;
use bioprism_section::OracleStatus;
use serde::{Deserialize, Serialize};

/// The assumptions a transport argument can rest on.
///
/// A closed set. An open string field would let a study declare an assumption nobody can check
/// the status of, which defeats the purpose of tracking status at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Assumption {
    /// Treated and untreated groups are comparable given the measured covariates.
    Exchangeability,
    /// Every covariate stratum in the target has support in the source.
    Positivity,
    /// The observed treatment corresponds to a well-defined intervention.
    Consistency,
    /// One subject's treatment does not affect another's outcome.
    NoInterference,
    /// Selection into the study is independent of the outcome given covariates.
    SelectionIndependence,
    /// The effect-modifier distribution differences between source and target are measured.
    Transportability,
}

impl Assumption {
    pub const ALL: [Assumption; 6] = [
        Assumption::Exchangeability,
        Assumption::Positivity,
        Assumption::Consistency,
        Assumption::NoInterference,
        Assumption::SelectionIndependence,
        Assumption::Transportability,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Assumption::Exchangeability => "exchangeability",
            Assumption::Positivity => "positivity",
            Assumption::Consistency => "consistency",
            Assumption::NoInterference => "no_interference",
            Assumption::SelectionIndependence => "selection_independence",
            Assumption::Transportability => "transportability",
        }
    }
}

/// What is known about one assumption.
///
/// The arms are not a scale. `Asserted` is not "slightly less than justified"; it is a different
/// kind of statement, and a reader who wants to know whether anyone checked must be able to see
/// the difference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AssumptionStatus {
    /// Checked, with the artifact that checked it.
    Justified { by: String },
    /// Stated by an investigator, unchecked.
    Asserted { by: String },
    /// Checked and found false, with a witness.
    Violated { witness: String },
    /// Nobody looked.
    Unexamined,
}

impl AssumptionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            AssumptionStatus::Justified { .. } => "justified",
            AssumptionStatus::Asserted { .. } => "asserted",
            AssumptionStatus::Violated { .. } => "violated",
            AssumptionStatus::Unexamined => "unexamined",
        }
    }
}

/// One assumption and its status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssumptionRecord {
    pub assumption: Assumption,
    pub status: AssumptionStatus,
}

impl AssumptionRecord {
    pub fn new(assumption: Assumption, status: AssumptionStatus) -> Self {
        AssumptionRecord { assumption, status }
    }
}

/// How the source evidence was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudyDesign {
    Randomised,
    Observational,
}

/// A named identification strategy, when the design is observational.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentificationStrategy {
    Backdoor,
    FrontDoor,
    InstrumentalVariable,
    DifferenceInDifferences,
}

impl IdentificationStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            IdentificationStrategy::Backdoor => "backdoor",
            IdentificationStrategy::FrontDoor => "front_door",
            IdentificationStrategy::InstrumentalVariable => "instrumental_variable",
            IdentificationStrategy::DifferenceInDifferences => "difference_in_differences",
        }
    }
}

/// How many source units support one target covariate stratum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StratumSupport {
    pub stratum: String,
    pub target_units: usize,
    pub source_units: usize,
}

/// A transport question and everything declared about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportQuestion {
    pub effect: String,
    pub source_population: ScopeKey,
    pub target_population: ScopeKey,
    pub design: StudyDesign,
    #[serde(default)]
    pub identification: Option<IdentificationStrategy>,
    pub assumptions: Vec<AssumptionRecord>,
    #[serde(default)]
    pub strata: Vec<StratumSupport>,
}

impl TransportQuestion {
    pub fn new(
        effect: impl Into<String>,
        source_population: ScopeKey,
        target_population: ScopeKey,
        design: StudyDesign,
    ) -> Self {
        TransportQuestion {
            effect: effect.into(),
            source_population,
            target_population,
            design,
            identification: None,
            assumptions: Vec::new(),
            strata: Vec::new(),
        }
    }

    pub fn with_identification(mut self, strategy: IdentificationStrategy) -> Self {
        self.identification = Some(strategy);
        self
    }

    pub fn assuming(mut self, assumption: Assumption, status: AssumptionStatus) -> Self {
        self.assumptions
            .push(AssumptionRecord::new(assumption, status));
        self
    }

    pub fn with_strata(mut self, strata: Vec<StratumSupport>) -> Self {
        self.strata = strata;
        self
    }
}

/// What the transport lens found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportFinding {
    /// An assumption the argument needs that nobody examined.
    UnexaminedAssumption { assumption: Assumption },
    /// An assumption declared true by a person rather than established by a check.
    AssertedWithoutJustification { assumption: Assumption, by: String },
    /// An assumption checked and found false.
    ViolatedAssumption {
        assumption: Assumption,
        witness: String,
    },
    /// A target stratum with no source units. The one assumption failure that yields counts.
    PositivityViolation {
        stratum: String,
        target_units: usize,
    },
    /// An assumption the argument needs that was never listed at all.
    AssumptionNotDeclared { assumption: Assumption },
}

impl Witness for TransportFinding {
    fn kind(&self) -> &'static str {
        match self {
            TransportFinding::UnexaminedAssumption { .. } => "unexamined_assumption",
            TransportFinding::AssertedWithoutJustification { .. } => {
                "asserted_without_justification"
            }
            TransportFinding::ViolatedAssumption { .. } => "violated_assumption",
            TransportFinding::PositivityViolation { .. } => "positivity_violation",
            TransportFinding::AssumptionNotDeclared { .. } => "assumption_not_declared",
        }
    }

    fn columns(&self) -> &'static [&'static str] {
        match self {
            TransportFinding::UnexaminedAssumption { .. }
            | TransportFinding::AssumptionNotDeclared { .. } => &["assumption", "status"],
            TransportFinding::AssertedWithoutJustification { .. } => &["assumption", "asserted_by"],
            TransportFinding::ViolatedAssumption { .. } => &["assumption", "witness"],
            TransportFinding::PositivityViolation { .. } => {
                &["stratum", "target_units", "source_units"]
            }
        }
    }

    fn cells(&self) -> Vec<Cell> {
        match self {
            TransportFinding::UnexaminedAssumption { assumption } => {
                vec![Cell::text(assumption.as_str()), Cell::text("unexamined")]
            }
            TransportFinding::AssumptionNotDeclared { assumption } => {
                vec![Cell::text(assumption.as_str()), Cell::text("not_declared")]
            }
            TransportFinding::AssertedWithoutJustification { assumption, by } => {
                vec![Cell::text(assumption.as_str()), Cell::text(by.clone())]
            }
            TransportFinding::ViolatedAssumption {
                assumption,
                witness,
            } => vec![Cell::text(assumption.as_str()), Cell::text(witness.clone())],
            TransportFinding::PositivityViolation {
                stratum,
                target_units,
            } => vec![
                Cell::text(stratum.clone()),
                Cell::count(*target_units),
                Cell::count(0),
            ],
        }
    }

    fn sentence(&self) -> String {
        match self {
            TransportFinding::UnexaminedAssumption { assumption } => format!(
                "{} was never examined; the transport conclusion does not rest on it, it \
                 presumes it",
                assumption.as_str()
            ),
            TransportFinding::AssumptionNotDeclared { assumption } => format!(
                "{} is required for this design and was not declared at all",
                assumption.as_str()
            ),
            TransportFinding::AssertedWithoutJustification { assumption, by } => format!(
                "{} was asserted by {by} without a check attached",
                assumption.as_str()
            ),
            TransportFinding::ViolatedAssumption {
                assumption,
                witness,
            } => format!("{} is violated: {witness}", assumption.as_str()),
            TransportFinding::PositivityViolation {
                stratum,
                target_units,
            } => format!(
                "target stratum `{stratum}` holds {target_units} unit(s) and has zero source \
                 support, so no transported estimate exists for it"
            ),
        }
    }
}

/// The verdict over a set of transport findings.
///
/// `Valid` requires that every needed assumption was examined and none was violated. An
/// unexamined assumption yields `Underdetermined` — never `Valid`, and never `Invalid` either,
/// because failing to look is not the same as looking and finding a problem.
pub fn transport_status(findings: &[TransportFinding]) -> OracleStatus {
    let violated = findings.iter().any(|f| {
        matches!(
            f,
            TransportFinding::ViolatedAssumption { .. }
                | TransportFinding::PositivityViolation { .. }
        )
    });
    if violated {
        return OracleStatus::Invalid;
    }
    let unknown = findings.iter().any(|f| {
        matches!(
            f,
            TransportFinding::UnexaminedAssumption { .. }
                | TransportFinding::AssumptionNotDeclared { .. }
                | TransportFinding::AssertedWithoutJustification { .. }
        )
    });
    if unknown {
        OracleStatus::Underdetermined
    } else {
        OracleStatus::Valid
    }
}

/// Blueprint 42.12.
#[derive(Debug, Clone, Copy, Default)]
pub struct CausalTransportLens;

impl CausalTransportLens {
    pub const ID: &'static str = "causal_transport";

    /// The assumptions a design must account for. Randomisation buys exchangeability and
    /// selection independence inside the trial; it buys nothing about the target population.
    pub fn required_assumptions(design: StudyDesign) -> Vec<Assumption> {
        match design {
            StudyDesign::Randomised => vec![
                Assumption::Consistency,
                Assumption::NoInterference,
                Assumption::Positivity,
                Assumption::Transportability,
            ],
            StudyDesign::Observational => Assumption::ALL.to_vec(),
        }
    }
}

impl Lens for CausalTransportLens {
    type Evidence = TransportQuestion;
    type Witness = TransportFinding;

    fn declaration(&self) -> LensDeclaration {
        LensDeclaration::new(
            LensId::new(Self::ID),
            "42.12",
            "which assumptions does transporting this effect to the target population require, \
             and which of them has anyone actually examined?",
            vec![
                EvidenceRequirement::new(
                    "transport.design",
                    "how the source evidence was produced",
                ),
                EvidenceRequirement::new(
                    "transport.assumptions",
                    "each required assumption and its examination status",
                ),
                EvidenceRequirement::new(
                    "transport.strata",
                    "source support for each target covariate stratum",
                ),
                EvidenceRequirement::new(
                    "transport.identification",
                    "the identification strategy, when the design is observational",
                ),
            ],
            vec![ScopePrecondition::new(
                "target_population",
                "an effect transports to somewhere; without a target the question is unposed",
            )],
            vec![
                RefusalReason::ScopePreconditionUnmet,
                RefusalReason::WouldRequireInterventionalClaim,
            ],
        )
        .expect("42.12 declaration is well formed")
    }

    fn answer(
        &self,
        _scope: &ScopeKey,
        question: &TransportQuestion,
    ) -> LensOutcome<TransportFinding> {
        if question.design == StudyDesign::Observational && question.identification.is_none() {
            return LensOutcome::Refused(Refusal::new(
                RefusalReason::WouldRequireInterventionalClaim,
                format!(
                    "`{}` is an interventional claim over observational evidence with no declared \
                     identification strategy",
                    question.effect
                ),
            ));
        }

        let required = Self::required_assumptions(question.design);
        let mut findings = Vec::new();

        for assumption in &required {
            match question
                .assumptions
                .iter()
                .find(|record| record.assumption == *assumption)
            {
                None => findings.push(TransportFinding::AssumptionNotDeclared {
                    assumption: *assumption,
                }),
                Some(record) => match &record.status {
                    AssumptionStatus::Justified { .. } => {}
                    AssumptionStatus::Asserted { by } => {
                        findings.push(TransportFinding::AssertedWithoutJustification {
                            assumption: *assumption,
                            by: by.clone(),
                        })
                    }
                    AssumptionStatus::Violated { witness } => {
                        findings.push(TransportFinding::ViolatedAssumption {
                            assumption: *assumption,
                            witness: witness.clone(),
                        })
                    }
                    AssumptionStatus::Unexamined => {
                        findings.push(TransportFinding::UnexaminedAssumption {
                            assumption: *assumption,
                        })
                    }
                },
            }
        }

        for stratum in &question.strata {
            if stratum.source_units == 0 && stratum.target_units > 0 {
                findings.push(TransportFinding::PositivityViolation {
                    stratum: stratum.stratum.clone(),
                    target_units: stratum.target_units,
                });
            }
        }

        let eligible = required.len() + question.strata.len();
        let coverage = Coverage::complete(Self::ID, eligible, eligible)
            .expect("every required assumption and declared stratum is examined");
        LensOutcome::Answered {
            witnesses: findings,
            coverage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::{run, ReportOutcome};

    fn scope() -> ScopeKey {
        ScopeKey::new().exact("target_population", "community-oncology")
    }

    fn fully_justified(design: StudyDesign) -> TransportQuestion {
        let mut question = TransportQuestion::new(
            "pembrolizumab effect",
            ScopeKey::new().exact("population", "trial"),
            ScopeKey::new().exact("population", "community"),
            design,
        );
        if design == StudyDesign::Observational {
            question = question.with_identification(IdentificationStrategy::Backdoor);
        }
        for assumption in CausalTransportLens::required_assumptions(design) {
            question = question.assuming(
                assumption,
                AssumptionStatus::Justified {
                    by: "audit-2026-01".into(),
                },
            );
        }
        question
    }

    fn findings_of(question: &TransportQuestion) -> Vec<TransportFinding> {
        match CausalTransportLens.answer(&scope(), question) {
            LensOutcome::Answered { witnesses, .. } => witnesses,
            other => panic!("expected an answer, got {:?}", other.kind_str()),
        }
    }

    impl LensOutcome<TransportFinding> {
        fn kind_str(&self) -> &'static str {
            match self {
                LensOutcome::Answered { .. } => "answered",
                LensOutcome::Refused(_) => "refused",
                LensOutcome::EvidenceAbsent(_) => "evidence_absent",
            }
        }
    }

    #[test]
    fn an_unexamined_assumption_is_never_a_satisfied_one() {
        let mut question = fully_justified(StudyDesign::Randomised);
        question
            .assumptions
            .iter_mut()
            .find(|r| r.assumption == Assumption::Transportability)
            .unwrap()
            .status = AssumptionStatus::Unexamined;
        let found = findings_of(&question);
        assert_eq!(found.len(), 1);
        assert_eq!(transport_status(&found), OracleStatus::Underdetermined);
        assert_ne!(transport_status(&found), OracleStatus::Valid);
    }

    #[test]
    fn an_unexamined_assumption_is_not_reported_as_a_violation_either() {
        let mut question = fully_justified(StudyDesign::Randomised);
        question
            .assumptions
            .iter_mut()
            .find(|r| r.assumption == Assumption::Consistency)
            .unwrap()
            .status = AssumptionStatus::Unexamined;
        assert_eq!(
            transport_status(&findings_of(&question)),
            OracleStatus::Underdetermined
        );
    }

    #[test]
    fn an_assumption_asserted_by_a_person_is_not_a_justified_one() {
        let mut question = fully_justified(StudyDesign::Randomised);
        question
            .assumptions
            .iter_mut()
            .find(|r| r.assumption == Assumption::NoInterference)
            .unwrap()
            .status = AssumptionStatus::Asserted {
            by: "the investigator".into(),
        };
        let found = findings_of(&question);
        assert_eq!(found[0].kind(), "asserted_without_justification");
        assert_eq!(transport_status(&found), OracleStatus::Underdetermined);
    }

    #[test]
    fn an_assumption_nobody_listed_is_reported_rather_than_assumed_away() {
        let mut question = fully_justified(StudyDesign::Randomised);
        question
            .assumptions
            .retain(|r| r.assumption != Assumption::Positivity);
        let found = findings_of(&question);
        assert_eq!(found[0].kind(), "assumption_not_declared");
        assert_eq!(transport_status(&found), OracleStatus::Underdetermined);
    }

    #[test]
    fn a_target_stratum_with_no_source_support_yields_a_counted_witness() {
        let question = fully_justified(StudyDesign::Randomised).with_strata(vec![StratumSupport {
            stratum: "age>=75".into(),
            target_units: 41,
            source_units: 0,
        }]);
        let found = findings_of(&question);
        let violation = found
            .iter()
            .find(|f| f.kind() == "positivity_violation")
            .expect("positivity violation found");
        assert!(violation.sentence().contains("age>=75"));
        assert!(violation.sentence().contains("41"));
        assert_eq!(transport_status(&found), OracleStatus::Invalid);
    }

    #[test]
    fn a_supported_stratum_raises_nothing() {
        let question = fully_justified(StudyDesign::Randomised).with_strata(vec![StratumSupport {
            stratum: "age>=75".into(),
            target_units: 41,
            source_units: 12,
        }]);
        assert!(findings_of(&question).is_empty());
    }

    #[test]
    fn an_observational_design_without_an_identification_strategy_is_refused() {
        let mut question = fully_justified(StudyDesign::Observational);
        question.identification = None;
        let report = run(&CausalTransportLens, &scope(), &question).unwrap();
        match report.outcome() {
            ReportOutcome::Refused(refusal) => {
                assert_eq!(
                    refusal.reason,
                    RefusalReason::WouldRequireInterventionalClaim
                );
            }
            other => panic!("expected refusal, got {other:?}"),
        }
    }

    #[test]
    fn an_observational_design_needs_every_assumption_a_randomised_one_does_and_more() {
        let randomised = CausalTransportLens::required_assumptions(StudyDesign::Randomised);
        let observational = CausalTransportLens::required_assumptions(StudyDesign::Observational);
        assert!(randomised.iter().all(|a| observational.contains(a)));
        assert!(observational.len() > randomised.len());
    }

    #[test]
    fn a_fully_justified_transport_is_valid() {
        let found = findings_of(&fully_justified(StudyDesign::Randomised));
        assert!(found.is_empty());
        assert_eq!(transport_status(&found), OracleStatus::Valid);
    }

    #[test]
    fn the_lens_refuses_when_no_target_population_is_bound() {
        let report = run(
            &CausalTransportLens,
            &ScopeKey::new(),
            &fully_justified(StudyDesign::Randomised),
        )
        .unwrap();
        assert_eq!(report.outcome().as_str(), "refused");
    }
}
