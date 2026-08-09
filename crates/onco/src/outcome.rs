//! Outcomes, survival, censoring and longitudinal estimands.
//!
//! Blueprint 30.26.
//!
//! # Censoring is not an event
//!
//! A subject lost to follow-up did not have the event. Recording the loss as an event
//! systematically shortens observed survival, and the bias is worst exactly where follow-up is
//! worst — sicker subjects, poorer sites, longer studies — so it does not average out. Here
//! [`CensoringReason`] and [`EventKind`] are separate types reachable only through
//! [`AnalysisOutcome`], and no function in this module maps a loss to an event.
//!
//! The converse is enforced too: [`TerminalFact::Progression`] carries a
//! [`ConfirmedProgression`] token, so a not-evaluable scan cannot become a progression event.
//!
//! # Which estimand does an endpoint answer
//!
//! 30.26 requires that the target estimand be written *before* modelling, and names mixing
//! overall and progression-free endpoints as a failure. [`FollowUp::analyse`] therefore takes an
//! [`Estimand`] rather than an [`EndpointKind`]: the caller must have stated the population, the
//! intercurrent-event strategies and the censoring assumption before any number comes back.
//!
//! The sharpest consequence is that the same death is a different thing under different
//! endpoints. Under progression-free survival death is an event, handled by a composite-variable
//! strategy. Under time to progression it is a competing risk, censored under a hypothetical
//! strategy, and that censoring is *not* non-informative — subjects who die were not a random
//! sample of those at risk. [`EndpointKind::default_estimand`] says so in the type.
//!
//! # Not implemented
//!
//! Any estimator. This module produces per-subject analysis records — at-risk time, event or
//! censoring, declared estimand, bias flags — and stops there. Kaplan-Meier, Cox models,
//! competing-risk cumulative incidence, and the coverage and transport metrics 30.26 asks for
//! belong to whatever consumes these records. Two of the six biases 30.26 names, landmark-time
//! change and follow-up intensity, are cohort-level properties and are not detectable from a
//! single subject; they are absent from [`AnalysedFollowUp::bias_flags`] for that reason.

use crate::error::OncoError;
use crate::response::{ConfirmedProgression, ResponseAssessment};
use crate::worldline::{AcquisitionTime, SubjectRef};
use serde::{Deserialize, Serialize};

/// Time-to-event endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointKind {
    OverallSurvival,
    ProgressionFreeSurvival,
    TimeToProgression,
    TimeToTreatmentFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathCause {
    DiseaseRelated,
    TreatmentRelated,
    Other,
    Unknown,
}

/// How follow-up ended, as observed — before any endpoint interprets it.
///
/// This is the raw fact. It is deliberately endpoint-agnostic, because the same fact means
/// different things under different endpoints and baking one meaning in here is how endpoints
/// get mixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "terminal")]
pub enum TerminalFact {
    Death {
        cause: DeathCause,
    },
    /// Requires the confirmation token from [`crate::response`].
    Progression(ConfirmedProgression),
    /// Contact was lost. Note the variant's home: it sits among terminal *facts*, and every
    /// endpoint maps it to censoring. There is no path from here to [`EventKind`].
    LostToFollowUp,
    WithdrewConsent,
    StartedSubsequentTherapy,
    /// The database closed while the subject was still under observation.
    AdministrativeCutoff,
    EventFreeAtLastContact,
}

/// What an endpoint counts as having happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Death,
    ConfirmedProgression,
    /// The composite of progression-free survival.
    ProgressionOrDeath,
    TreatmentFailure,
}

/// Why observation stopped without the endpoint's event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CensoringReason {
    AdministrativeCutoff,
    LostToFollowUp,
    WithdrewConsent,
    EventFreeAtLastContact,
    /// Death under an endpoint for which death is a competing risk, not an event.
    CompetingDeath,
    SubsequentTherapy,
}

impl CensoringReason {
    /// Whether this reason plausibly depends on the subject's prognosis.
    ///
    /// 30.26 forbids treating censoring as random without examination. Administrative cutoff is
    /// imposed by the study calendar and is independent of the subject; loss to follow-up,
    /// withdrawal, competing death and treatment switching are not, and every downstream
    /// estimator that assumes non-informative censoring is biased in their presence.
    pub const fn is_potentially_informative(self) -> bool {
        match self {
            CensoringReason::AdministrativeCutoff | CensoringReason::EventFreeAtLastContact => {
                false
            }
            CensoringReason::LostToFollowUp
            | CensoringReason::WithdrewConsent
            | CensoringReason::CompetingDeath
            | CensoringReason::SubsequentTherapy => true,
        }
    }
}

/// The endpoint's verdict on one subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum AnalysisOutcome {
    Event(EventKind),
    Censored(CensoringReason),
}

impl AnalysisOutcome {
    pub const fn is_event(&self) -> bool {
        matches!(self, AnalysisOutcome::Event(_))
    }

    pub const fn censoring_reason(&self) -> Option<CensoringReason> {
        match self {
            AnalysisOutcome::Censored(reason) => Some(*reason),
            AnalysisOutcome::Event(_) => None,
        }
    }
}

/// Analysis population.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Population {
    IntentionToTreat,
    PerProtocol,
    EvaluableForResponse,
}

/// How an intercurrent event is handled, in the ICH E9(R1) vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntercurrentEventStrategy {
    Treatment,
    CompositeVariable,
    Hypothetical,
    PrincipalStratum,
    WhileOnTreatment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntercurrentEvent {
    Death,
    SubsequentAnticancerTherapy,
    LossToFollowUp,
    ConsentWithdrawal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryMeasure {
    MedianTimeToEvent,
    HazardRatio,
    LandmarkProbability { days: i64 },
}

/// What is assumed about the censoring mechanism.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "censoring")]
pub enum CensoringAssumption {
    /// Censoring is independent of prognosis given covariates.
    NoninformativeAssumed,
    /// At least one censoring mechanism in this endpoint depends on prognosis.
    ///
    /// `concern` is prose rather than a code because 30.26 enumerates no taxonomy of censoring
    /// mechanisms; what matters structurally is that the variant exists and that an endpoint
    /// cannot silently claim the non-informative case.
    PotentiallyInformative { concern: String },
}

/// The target of estimation, stated before modelling (30.26).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Estimand {
    pub endpoint: EndpointKind,
    pub population: Population,
    /// Plain-language statement of the variable being estimated.
    pub variable: String,
    pub summary_measure: SummaryMeasure,
    pub intercurrent_event_strategies: Vec<(IntercurrentEvent, IntercurrentEventStrategy)>,
    pub censoring_assumption: CensoringAssumption,
}

impl EndpointKind {
    /// The estimand this endpoint actually answers under its conventional handling.
    ///
    /// Read the differences between overall survival, progression-free survival and time to
    /// progression: they are the same follow-up viewed under three different treatments of
    /// death, and only one of the three can claim non-informative censoring.
    pub fn default_estimand(self, population: Population) -> Estimand {
        match self {
            EndpointKind::OverallSurvival => Estimand {
                endpoint: self,
                population,
                variable: "time from risk-set entry to death from any cause".into(),
                summary_measure: SummaryMeasure::MedianTimeToEvent,
                intercurrent_event_strategies: vec![
                    (
                        IntercurrentEvent::SubsequentAnticancerTherapy,
                        IntercurrentEventStrategy::Treatment,
                    ),
                    (
                        IntercurrentEvent::LossToFollowUp,
                        IntercurrentEventStrategy::Hypothetical,
                    ),
                ],
                censoring_assumption: CensoringAssumption::NoninformativeAssumed,
            },
            EndpointKind::ProgressionFreeSurvival => Estimand {
                endpoint: self,
                population,
                variable: "time from risk-set entry to confirmed progression or death, whichever first"
                    .into(),
                summary_measure: SummaryMeasure::MedianTimeToEvent,
                intercurrent_event_strategies: vec![
                    (IntercurrentEvent::Death, IntercurrentEventStrategy::CompositeVariable),
                    (
                        IntercurrentEvent::SubsequentAnticancerTherapy,
                        IntercurrentEventStrategy::Hypothetical,
                    ),
                    (
                        IntercurrentEvent::LossToFollowUp,
                        IntercurrentEventStrategy::Hypothetical,
                    ),
                ],
                censoring_assumption: CensoringAssumption::PotentiallyInformative {
                    concern: "assessment is scan-driven, so censoring depends on imaging frequency \
                              and on whether a subject was well enough to be scanned"
                        .into(),
                },
            },
            EndpointKind::TimeToProgression => Estimand {
                endpoint: self,
                population,
                variable: "time from risk-set entry to confirmed progression, with death as a \
                           competing risk"
                    .into(),
                summary_measure: SummaryMeasure::MedianTimeToEvent,
                intercurrent_event_strategies: vec![
                    (IntercurrentEvent::Death, IntercurrentEventStrategy::Hypothetical),
                    (
                        IntercurrentEvent::SubsequentAnticancerTherapy,
                        IntercurrentEventStrategy::Hypothetical,
                    ),
                ],
                censoring_assumption: CensoringAssumption::PotentiallyInformative {
                    concern: "death is censored rather than counted, and subjects who die are not \
                              a random sample of those at risk of progression"
                        .into(),
                },
            },
            EndpointKind::TimeToTreatmentFailure => Estimand {
                endpoint: self,
                population,
                variable: "time from risk-set entry to progression, death, or discontinuation for \
                           any reason"
                    .into(),
                summary_measure: SummaryMeasure::MedianTimeToEvent,
                intercurrent_event_strategies: vec![
                    (IntercurrentEvent::Death, IntercurrentEventStrategy::CompositeVariable),
                    (
                        IntercurrentEvent::ConsentWithdrawal,
                        IntercurrentEventStrategy::CompositeVariable,
                    ),
                ],
                censoring_assumption: CensoringAssumption::PotentiallyInformative {
                    concern: "discontinuation is counted as failure, so the endpoint mixes efficacy \
                              with tolerability and clinician preference"
                        .into(),
                },
            },
        }
    }

    /// Interpret a terminal fact under this endpoint.
    ///
    /// Exhaustive by construction, and there is no arm anywhere below that sends
    /// [`TerminalFact::LostToFollowUp`] to [`AnalysisOutcome::Event`].
    pub const fn classify(self, terminal: &TerminalFact) -> AnalysisOutcome {
        match (self, terminal) {
            (EndpointKind::OverallSurvival, TerminalFact::Death { .. }) => {
                AnalysisOutcome::Event(EventKind::Death)
            }
            (EndpointKind::ProgressionFreeSurvival, TerminalFact::Death { .. })
            | (EndpointKind::ProgressionFreeSurvival, TerminalFact::Progression(_)) => {
                AnalysisOutcome::Event(EventKind::ProgressionOrDeath)
            }
            (EndpointKind::TimeToProgression, TerminalFact::Progression(_)) => {
                AnalysisOutcome::Event(EventKind::ConfirmedProgression)
            }
            (EndpointKind::TimeToProgression, TerminalFact::Death { .. }) => {
                AnalysisOutcome::Censored(CensoringReason::CompetingDeath)
            }
            (EndpointKind::TimeToTreatmentFailure, TerminalFact::Death { .. })
            | (EndpointKind::TimeToTreatmentFailure, TerminalFact::Progression(_))
            | (EndpointKind::TimeToTreatmentFailure, TerminalFact::StartedSubsequentTherapy)
            | (EndpointKind::TimeToTreatmentFailure, TerminalFact::WithdrewConsent) => {
                AnalysisOutcome::Event(EventKind::TreatmentFailure)
            }
            (_, TerminalFact::LostToFollowUp) => {
                AnalysisOutcome::Censored(CensoringReason::LostToFollowUp)
            }
            (_, TerminalFact::WithdrewConsent) => {
                AnalysisOutcome::Censored(CensoringReason::WithdrewConsent)
            }
            (_, TerminalFact::StartedSubsequentTherapy) => {
                AnalysisOutcome::Censored(CensoringReason::SubsequentTherapy)
            }
            (_, TerminalFact::AdministrativeCutoff) => {
                AnalysisOutcome::Censored(CensoringReason::AdministrativeCutoff)
            }
            (_, TerminalFact::EventFreeAtLastContact) | (_, TerminalFact::Progression(_)) => {
                AnalysisOutcome::Censored(CensoringReason::EventFreeAtLastContact)
            }
        }
    }
}

/// Biases 30.26 names, where they are detectable from one subject's record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisBias {
    LeftTruncation,
    InformativeLossToFollowUp,
    CompetingDeath,
    TreatmentSwitching,
}

/// One subject's follow-up.
///
/// Three instants, not one. The **index date** is when the clinical clock starts, typically
/// diagnosis. The **risk-set entry** is when the subject actually became observable — enrolment,
/// consent, arrival at a referral centre. They differ constantly, and 30.26's microbenchmark is
/// exactly this: delayed entry must be recognised rather than counting survival from diagnosis
/// as at-risk time, because the interval between them is time the subject was guaranteed to have
/// survived in order to enter at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "FollowUpDocument", into = "FollowUpDocument")]
pub struct FollowUp {
    subject: SubjectRef,
    index: AcquisitionTime,
    entry: AcquisitionTime,
    last_contact: AcquisitionTime,
    terminal: TerminalFact,
}

impl FollowUp {
    pub fn new(
        subject: SubjectRef,
        index: AcquisitionTime,
        entry: AcquisitionTime,
        last_contact: AcquisitionTime,
        terminal: TerminalFact,
    ) -> Result<Self, OncoError> {
        if entry < index {
            return Err(OncoError::RiskSetEntryBeforeIndex {
                index: index.timestamp(),
                entry: entry.timestamp(),
            });
        }
        if last_contact < entry {
            return Err(OncoError::FollowUpEndsBeforeItStarts {
                entry: entry.timestamp(),
                last_contact: last_contact.timestamp(),
            });
        }
        Ok(FollowUp {
            subject,
            index,
            entry,
            last_contact,
            terminal,
        })
    }

    /// Record a progression endpoint from an assessment.
    ///
    /// The only way to build a [`TerminalFact::Progression`] from an assessment, and it refuses
    /// anything the confirmation gate did not pass — including not-evaluable, which is the call
    /// a post-treatment scan produces.
    pub fn from_assessment(
        subject: SubjectRef,
        index: AcquisitionTime,
        entry: AcquisitionTime,
        assessment: &ResponseAssessment,
    ) -> Result<Self, OncoError> {
        let Some(confirmed) = assessment.call.confirmed_progression() else {
            return Err(OncoError::ResponseCallIsNotProgression {
                call: assessment.call.label(),
            });
        };
        FollowUp::new(
            subject,
            index,
            entry,
            confirmed.confirmed_at(),
            TerminalFact::Progression(*confirmed),
        )
    }

    pub const fn subject(&self) -> &SubjectRef {
        &self.subject
    }

    pub const fn terminal(&self) -> TerminalFact {
        self.terminal
    }

    /// Whether the subject entered the risk set after the index date.
    pub fn is_left_truncated(&self) -> bool {
        self.entry > self.index
    }

    /// Days between index and risk-set entry.
    ///
    /// Survival during this interval was guaranteed by the fact of entering, so counting it as
    /// at-risk time manufactures the immortal-time bias 30.26 names.
    pub fn immortal_time_days(&self) -> i64 {
        self.index.days_until(self.entry)
    }

    /// Observed time at risk, measured from entry.
    pub fn at_risk_days(&self) -> i64 {
        self.entry.days_until(self.last_contact)
    }

    /// Analyse under a previously stated estimand.
    ///
    /// Takes the estimand rather than the endpoint so that the target of estimation exists
    /// before the number does, as 30.26 requires.
    pub fn analyse(&self, estimand: &Estimand) -> AnalysedFollowUp {
        let outcome = estimand.endpoint.classify(&self.terminal);
        let mut bias_flags = Vec::new();
        if self.is_left_truncated() {
            bias_flags.push(AnalysisBias::LeftTruncation);
        }
        match outcome.censoring_reason() {
            Some(CensoringReason::LostToFollowUp) => {
                bias_flags.push(AnalysisBias::InformativeLossToFollowUp);
            }
            Some(CensoringReason::CompetingDeath) => bias_flags.push(AnalysisBias::CompetingDeath),
            Some(CensoringReason::SubsequentTherapy) => {
                bias_flags.push(AnalysisBias::TreatmentSwitching);
            }
            _ => {}
        }

        AnalysedFollowUp {
            subject: self.subject.clone(),
            estimand: estimand.clone(),
            at_risk_days: self.at_risk_days(),
            immortal_time_days: self.immortal_time_days(),
            outcome,
            bias_flags,
        }
    }
}

/// One subject, reduced to what an estimator consumes, with its assumptions attached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysedFollowUp {
    pub subject: SubjectRef,
    /// Carried along so a record can never be pooled under an estimand it did not answer.
    pub estimand: Estimand,
    pub at_risk_days: i64,
    pub immortal_time_days: i64,
    pub outcome: AnalysisOutcome,
    pub bias_flags: Vec<AnalysisBias>,
}

#[derive(Serialize, Deserialize)]
struct FollowUpDocument {
    subject: SubjectRef,
    index: AcquisitionTime,
    entry: AcquisitionTime,
    last_contact: AcquisitionTime,
    terminal: TerminalFact,
}

impl TryFrom<FollowUpDocument> for FollowUp {
    type Error = OncoError;

    fn try_from(document: FollowUpDocument) -> Result<Self, Self::Error> {
        FollowUp::new(
            document.subject,
            document.index,
            document.entry,
            document.last_contact,
            document.terminal,
        )
    }
}

impl From<FollowUp> for FollowUpDocument {
    fn from(value: FollowUp) -> Self {
        FollowUpDocument {
            subject: value.subject,
            index: value.index,
            entry: value.entry,
            last_contact: value.last_contact,
            terminal: value.terminal,
        }
    }
}
