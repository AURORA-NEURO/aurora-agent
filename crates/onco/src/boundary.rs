//! The research-only clinical boundary and human escalation.
//!
//! Blueprint 30.30, whose stated purpose is to "define the boundary that lets OncoWorld model
//! realistic clinical research artifacts without turning the system into patient-specific
//! decision support".
//!
//! # The boundary has two edges, not one
//!
//! 30.30 lists five characteristic failures, and they point in opposite directions. Four are
//! overreach: giving treatment advice, treating benchmark cases as real patients, exposing
//! identifiable data, letting a research agent execute clinical actions. The fifth is
//! *over-refusing safe aggregate analysis*. A guard that refuses everything containing the word
//! "patient" fails 30.30 exactly as surely as one that recommends chemotherapy.
//!
//! So the guard is a **splitter**, not a gate. [`ResearchBoundary::triage`] separates the
//! permitted uses from the refused ones and returns
//! [`BoundaryDisposition::ReleasePartial`] when a request contains both, preserving the
//! cohort-level analysis while refusing the person-level direction. That is ladder steps 3 and 6
//! of 30.30 — "separate cohort evidence from person-level action" and "preserve useful
//! nonclinical output" — in one type.
//!
//! # What the guard refuses to look at
//!
//! 30.30 names *"claimed user role"*, *"action urgency"* and *"embedded instructions in source
//! artifacts"* as stress axes: things an adversary varies to move the boundary. They are
//! recorded on [`BoundaryRequest`] for audit and are **never read** by [`ResearchBoundary`]. A
//! caller asserting they are an attending physician, or that the matter is urgent, gets exactly
//! the same disposition as one who asserts nothing. The fields exist so that a red-team run can
//! show the disposition did not move.
//!
//! # Not implemented
//!
//! 30.30 is the least specified module in the section: it names no escalation levels, no request
//! classification labels, no prohibited-action list, no consent scale, and no threshold for
//! "high-stakes ambiguity". [`OutputUse`], [`EscalationRoute`] and [`ConsentBasis`] below are
//! therefore invented vocabulary. Also absent: the policy engine, the logging schema, and any
//! detection of embedded instructions inside artifact text — this module trusts the caller to
//! have declared the request's uses honestly and defends only against the uses it is told about.

use crate::error::OncoError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// What an output would be used for.
///
/// The split is between *cohort evidence* and *person-level action*, which is ladder step 3 of
/// 30.30. Everything below the divide names an act of care.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputUse {
    /// Aggregate analysis over a cohort.
    CohortAnalysis,
    /// Building or evaluating a method.
    MethodDevelopment,
    /// Generating a research hypothesis for later testing.
    HypothesisGeneration,
    /// Checking data integrity.
    QualityControl,
    /// Assigning a diagnosis to an individual.
    IndividualDiagnosis,
    /// Predicting an individual's course.
    IndividualPrognosis,
    /// Selecting or advising therapy for an individual.
    TreatmentRecommendation,
    /// Prioritising an individual's access to care.
    CareTriage,
    /// Raising a clinical alert about an individual.
    ClinicalAlerting,
}

impl OutputUse {
    /// Whether this use is an act of clinical care directed at one person.
    ///
    /// The predicate is total and `const`: there is no configuration, no override, and no
    /// caller-supplied context that reclassifies a use. A boundary whose definition of "clinical"
    /// is adjustable is not a boundary.
    pub const fn is_individual_clinical_use(self) -> bool {
        match self {
            OutputUse::CohortAnalysis
            | OutputUse::MethodDevelopment
            | OutputUse::HypothesisGeneration
            | OutputUse::QualityControl => false,
            OutputUse::IndividualDiagnosis
            | OutputUse::IndividualPrognosis
            | OutputUse::TreatmentRecommendation
            | OutputUse::CareTriage
            | OutputUse::ClinicalAlerting => true,
        }
    }

    pub const fn describe(self) -> &'static str {
        match self {
            OutputUse::CohortAnalysis => "aggregate analysis over a cohort",
            OutputUse::MethodDevelopment => "method development and evaluation",
            OutputUse::HypothesisGeneration => "research hypothesis generation",
            OutputUse::QualityControl => "data quality control",
            OutputUse::IndividualDiagnosis => "assigning a diagnosis to an individual",
            OutputUse::IndividualPrognosis => "predicting an individual's course",
            OutputUse::TreatmentRecommendation => "selecting or advising therapy for an individual",
            OutputUse::CareTriage => "prioritising an individual's access to care",
            OutputUse::ClinicalAlerting => "raising a clinical alert about an individual",
        }
    }
}

/// Research versus care context (30.30 required state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestContext {
    Research,
    Care,
}

/// The consent basis a request claims. Invented vocabulary; 30.30 gives no scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentBasis {
    BroadResearchConsent,
    StudySpecificConsent,
    WaiverOfConsent,
    NotEstablished,
}

/// A request for output, carrying 30.30's seven required state slots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundaryRequest {
    /// Task purpose, in the requester's words.
    pub purpose: String,
    pub context: RequestContext,
    /// The role the requester *claims*. Recorded, never consulted — see the module note.
    pub claimed_role: String,
    /// Whether the requester asserts urgency. Recorded, never consulted.
    pub claimed_urgency: bool,
    pub consent: ConsentBasis,
    /// The uses the output is requested for.
    pub requested_uses: Vec<OutputUse>,
    /// Names of direct-identifier fields present in the request payload.
    ///
    /// Non-empty means the request is refused before analysis, so that the refusal cannot echo
    /// the identifiers back. 30.30's release gate is absolute: controlled or identifiable data
    /// never enter research outputs.
    pub direct_identifier_fields: Vec<String>,
}

/// Why a human was brought in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationTrigger {
    IndividualClinicalRequest,
    IdentifiableDataPresent,
    /// Measurements read as progression and the confirmation gate withheld the call.
    UnconfirmedProgressionSignal,
    /// The differential for an observed change could not be narrowed to one hypothesis.
    NonIdentifiableChangeState,
    /// Classification could not be integrated from the evidence available.
    UnresolvedClassification,
}

impl EscalationTrigger {
    pub const fn describe(self) -> &'static str {
        match self {
            EscalationTrigger::IndividualClinicalRequest => {
                "the request asks for direction about an individual's care"
            }
            EscalationTrigger::IdentifiableDataPresent => {
                "the request carries direct identifiers"
            }
            EscalationTrigger::UnconfirmedProgressionSignal => {
                "imaging read as progression and confirmation was not available"
            }
            EscalationTrigger::NonIdentifiableChangeState => {
                "the cause of the observed change is not identifiable from available evidence"
            }
            EscalationTrigger::UnresolvedClassification => {
                "integrated classification is not determined by the evidence available"
            }
        }
    }
}

/// Which human process a notice is routed to. Invented; 30.30 names no destinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationRoute {
    StudyTeam,
    DataManagement,
    TreatingClinicalTeam,
    InstitutionalReviewBoard,
}

impl EscalationRoute {
    pub const fn describe(self) -> &'static str {
        match self {
            EscalationRoute::StudyTeam => "the study team",
            EscalationRoute::DataManagement => "data management",
            EscalationRoute::TreatingClinicalTeam => "the treating clinical team",
            EscalationRoute::InstitutionalReviewBoard => "the institutional review board",
        }
    }
}

/// A hand-off to a human process.
///
/// # Why there is no message field
///
/// A free-text field on an escalation is a place to write "consider re-resection". The notice
/// carries only a trigger and a route, both closed enumerations, and [`fmt::Display`] generates
/// the sentence from them. A recommendation is therefore not representable in this type — not
/// discouraged, not filtered, not expressible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EscalationNotice {
    trigger: EscalationTrigger,
    route: EscalationRoute,
}

impl EscalationNotice {
    pub const fn raise(trigger: EscalationTrigger, route: EscalationRoute) -> Self {
        EscalationNotice { trigger, route }
    }

    pub const fn trigger(&self) -> EscalationTrigger {
        self.trigger
    }

    pub const fn route(&self) -> EscalationRoute {
        self.route
    }
}

impl fmt::Display for EscalationNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Human review required by {}: {}. This platform states no clinical action.",
            self.route.describe(),
            self.trigger.describe()
        )
    }
}

/// The three-way terminal action set of 30.30's decision-cell family, "stopping, abstaining, or
/// escalating".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalAction {
    /// Nothing is produced and nothing is routed.
    Stop,
    /// Produce what is permitted, decline the rest, route nothing.
    Abstain,
    /// Produce what is permitted, decline the rest, route to a human process.
    Escalate,
}

/// What the boundary decided about a request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "disposition")]
pub enum BoundaryDisposition {
    /// Every requested use is permitted.
    ReleaseInFull { uses: Vec<OutputUse> },
    /// Some uses are permitted and some are not.
    ///
    /// The permitted analysis is still produced. Collapsing this into a total refusal is
    /// 30.30's "over-refusing safe aggregate analysis" failure.
    ReleasePartial {
        released: Vec<OutputUse>,
        refused: Vec<OutputUse>,
        escalation: EscalationNotice,
    },
    /// No requested use is permitted.
    RefuseAndEscalate {
        refused: Vec<OutputUse>,
        escalation: EscalationNotice,
    },
}

impl BoundaryDisposition {
    pub fn released(&self) -> &[OutputUse] {
        match self {
            BoundaryDisposition::ReleaseInFull { uses } => uses,
            BoundaryDisposition::ReleasePartial { released, .. } => released,
            BoundaryDisposition::RefuseAndEscalate { .. } => &[],
        }
    }

    pub fn refused(&self) -> &[OutputUse] {
        match self {
            BoundaryDisposition::ReleaseInFull { .. } => &[],
            BoundaryDisposition::ReleasePartial { refused, .. }
            | BoundaryDisposition::RefuseAndEscalate { refused, .. } => refused,
        }
    }

    pub const fn escalation(&self) -> Option<EscalationNotice> {
        match self {
            BoundaryDisposition::ReleaseInFull { .. } => None,
            BoundaryDisposition::ReleasePartial { escalation, .. }
            | BoundaryDisposition::RefuseAndEscalate { escalation, .. } => Some(*escalation),
        }
    }

    pub const fn terminal_action(&self) -> TerminalAction {
        match self {
            BoundaryDisposition::ReleaseInFull { .. } => TerminalAction::Abstain,
            BoundaryDisposition::ReleasePartial { .. } => TerminalAction::Escalate,
            BoundaryDisposition::RefuseAndEscalate { .. } => TerminalAction::Stop,
        }
    }
}

/// The research-only guard.
///
/// There is no constructor that accepts an arbitrary permission set. [`ResearchBoundary::extend`]
/// exists for adding further *research* uses and refuses clinical ones, so no configuration path
/// — not a builder, not a deserialised config, not a test helper — can produce a boundary that
/// permits diagnosis, prognosis, treatment recommendation, triage or alerting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "BoundaryDocument", into = "BoundaryDocument")]
pub struct ResearchBoundary {
    permitted: BTreeSet<OutputUse>,
}

impl ResearchBoundary {
    pub fn research_only() -> Self {
        ResearchBoundary {
            permitted: [
                OutputUse::CohortAnalysis,
                OutputUse::MethodDevelopment,
                OutputUse::HypothesisGeneration,
                OutputUse::QualityControl,
            ]
            .into_iter()
            .collect(),
        }
    }

    /// Add a further research use.
    ///
    /// Refuses any individual clinical use, with the same error a release attempt would give.
    pub fn extend(mut self, use_case: OutputUse) -> Result<Self, OncoError> {
        self.check(use_case)?;
        self.permitted.insert(use_case);
        Ok(self)
    }

    pub fn permitted(&self) -> Vec<OutputUse> {
        self.permitted.iter().copied().collect()
    }

    pub fn permits(&self, use_case: OutputUse) -> bool {
        !use_case.is_individual_clinical_use() && self.permitted.contains(&use_case)
    }

    /// Refuse an individual clinical use.
    pub fn check(&self, use_case: OutputUse) -> Result<(), OncoError> {
        if self.permits(use_case) {
            return Ok(());
        }
        Err(OncoError::OutsideResearchBoundary {
            attempted: use_case,
            permitted: self.permitted(),
        })
    }

    /// Ladder steps 1, 3, 5 and 6 of 30.30, in one pass.
    ///
    /// Identifiers are checked first and refused outright: no analysis runs, so no output exists
    /// that could carry them.
    pub fn triage(&self, request: &BoundaryRequest) -> Result<BoundaryDisposition, OncoError> {
        if !request.direct_identifier_fields.is_empty() {
            return Err(OncoError::IdentifiersPresent {
                count: request.direct_identifier_fields.len(),
            });
        }

        let mut released = Vec::new();
        let mut refused = Vec::new();
        for use_case in &request.requested_uses {
            if self.permits(*use_case) {
                released.push(*use_case);
            } else {
                refused.push(*use_case);
            }
        }

        if refused.is_empty() {
            return Ok(BoundaryDisposition::ReleaseInFull { uses: released });
        }

        let escalation = EscalationNotice::raise(
            EscalationTrigger::IndividualClinicalRequest,
            EscalationRoute::TreatingClinicalTeam,
        );
        if released.is_empty() {
            Ok(BoundaryDisposition::RefuseAndEscalate {
                refused,
                escalation,
            })
        } else {
            Ok(BoundaryDisposition::ReleasePartial {
                released,
                refused,
                escalation,
            })
        }
    }

    /// Wrap a value for release under a declared use.
    pub fn release<T>(
        &self,
        value: T,
        declared_use: OutputUse,
    ) -> Result<ResearchOutput<T>, OncoError> {
        self.check(declared_use)?;
        Ok(ResearchOutput {
            value,
            declared_use,
        })
    }
}

/// A value that passed the boundary.
///
/// # Why this type does not implement `Deserialize`
///
/// Deserialising a `ResearchOutput` would mint the guard's output without running the guard, and
/// the whole value of the type is that possessing one is evidence the check happened. A process
/// receiving research output over the wire decodes the inner `T` and re-releases it through its
/// own [`ResearchBoundary`], which is what an independent boundary is for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResearchOutput<T> {
    value: T,
    declared_use: OutputUse,
}

impl<T> ResearchOutput<T> {
    /// Fixed research-use statement. Not configurable, so it cannot be edited away.
    pub const STATEMENT: &str = "Research use only. Not for use in the diagnosis, prognosis, \
                                 treatment, or triage of any individual.";

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub const fn declared_use(&self) -> OutputUse {
        self.declared_use
    }

    pub fn into_inner(self) -> T {
        self.value
    }
}

#[derive(Serialize, Deserialize)]
struct BoundaryDocument {
    permitted: BTreeSet<OutputUse>,
}

impl TryFrom<BoundaryDocument> for ResearchBoundary {
    type Error = OncoError;

    fn try_from(document: BoundaryDocument) -> Result<Self, Self::Error> {
        if let Some(clinical) = document
            .permitted
            .iter()
            .find(|use_case| use_case.is_individual_clinical_use())
        {
            return Err(OncoError::OutsideResearchBoundary {
                attempted: *clinical,
                permitted: ResearchBoundary::research_only().permitted(),
            });
        }
        Ok(ResearchBoundary {
            permitted: document.permitted,
        })
    }
}

impl From<ResearchBoundary> for BoundaryDocument {
    fn from(value: ResearchBoundary) -> Self {
        BoundaryDocument {
            permitted: value.permitted,
        }
    }
}
