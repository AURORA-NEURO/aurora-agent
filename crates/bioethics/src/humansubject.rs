//! Blueprint 36.22 — research ethics, IRB and human-subject boundaries.
//!
//! 36.22's purpose sentence: "clarify when benchmark construction or prospective evaluation may
//! constitute human-subject research and require institutional review."
//!
//! # The determination is asymmetric, and the type says so
//!
//! [`Determination`] has two variants: [`Determination::ReviewRequired`] and
//! [`Determination::Undetermined`]. There is no `Exempt`, no `MinimalRisk`, no
//! `NotHumanSubjectResearch` and no boolean anywhere that [`screen`] could set to mean "no review
//! needed". Software can raise the flag; only an institution can lower it, and a library that
//! could emit an exemption would be issuing a determination it has no standing to make.
//!
//! This is registered as
//! [`crate::safeguard::Impossibility::NoScreeningResultRecordsAnInstitutionalExemption`] and it is
//! the whole reason the module exists as code rather than as a checklist.
//!
//! [`Determination::Undetermined`] carries [`UndeterminedReason`], which has one variant: the
//! study declared no engagement at all. A study with no declared engagement is not a study outside
//! the scope of human-subject review — it is a study nobody described, and the honest output is
//! silence rather than clearance.
//!
//! # Where an exemption *can* live
//!
//! [`InstitutionalDetermination`] transcribes what a named body decided, including
//! [`RecordedOutcome::DeterminedNotHumanSubjectResearch`]. The distinction is total: that value
//! can only be constructed by [`InstitutionalDetermination::record`], which demands a body and a
//! reference, and [`screen`] cannot produce one because its return type has no variant for it.
//! Nothing here authenticates the body or resolves the reference — this is a transcription, the
//! same standing `bioprism-stewardship` gives a site attestation.
//!
//! # Consent is `bioprism-policy`'s, and is not modelled twice
//!
//! [`StudyDescription::check_consent`] takes a `bioprism_policy::Consent` and calls its `check`
//! for each declared purpose, carrying policy's own refusal sentence into
//! [`crate::BioethicsError::PurposeOutsideConsent`]. There is no consent type here, no purpose
//! enumeration here, no expiry logic here and no withdrawal logic here. 36.04 and 36.18 are
//! policy's, and a second closed purpose list in this workspace would drift from the first within
//! a release.
//!
//! # Returning individual findings is the research boundary's question
//!
//! 36.22 requires a "return-of-results policy" and states none.
//! [`StudyDescription::check_return_of_results`] does not decide whether results should be
//! returned — that is an institutional decision this crate has no standing in. It decides one
//! narrower thing: whether *this platform* may be the thing that produces an individual finding
//! for a participant. It asks `bioprism_onco::ResearchBoundary`, which refuses, and the refusal
//! that comes back is onco's own. Aggregate return and no return are not this crate's business
//! and pass through untouched.
//!
//! # What 36.22 names and never specifies
//!
//! * **"institutional determination"** — who determines, on what basis, in what form, with what
//!   validity period. None of it stated. [`InstitutionalDetermination`] checks that a body and a
//!   reference are present and attributable, and nothing else.
//! * **"consent or waiver"** — no waiver criterion. [`RecordedOutcome::WaiverGranted`] is a
//!   transcription of somebody else's decision, not a decision procedure.
//! * **"minimal-risk assessment"** — no risk scale anywhere in §36. Not modelled.
//! * **"data-security plan"** — no required contents. Perimeter, declared only.
//! * **"publication and return-of-results policy"** — no policy given.
//! * **"ongoing review"** — no period, and there is no clock in this workspace to measure one
//!   against. The same gap `bioprism-stewardship` records for §14's "periodic review".
//!
//! # Not implemented
//!
//! No participant registry, no recruitment, no enrolment, no identity system, no risk model, no
//! document store, no reminder, no clock. AGENTS.md's boundary paragraph is load-bearing here: this
//! workspace does not enroll participants, and nothing in this module is a step towards doing so.

use crate::error::BioethicsError;
use bioprism_onco::{OutputUse, ResearchBoundary};
use bioprism_policy::{Consent, Purpose, PurposeSet};
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The six engagements of 36.22's Scope, transcribed.
///
/// Every one of them is a trigger. 36.22 offers no engagement that is listed as *not* implicating
/// review, so there is no non-triggering variant to add.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngagementKind {
    IdentifiableOrCodedData,
    InteractionWithParticipants,
    ProspectiveDataCollection,
    ExpertPerformanceStudy,
    ClinicalWorkflowObservation,
    SecondaryResearch,
}

impl EngagementKind {
    pub const ALL: [EngagementKind; 6] = [
        EngagementKind::IdentifiableOrCodedData,
        EngagementKind::InteractionWithParticipants,
        EngagementKind::ProspectiveDataCollection,
        EngagementKind::ExpertPerformanceStudy,
        EngagementKind::ClinicalWorkflowObservation,
        EngagementKind::SecondaryResearch,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            EngagementKind::IdentifiableOrCodedData => "identifiable_or_coded_data",
            EngagementKind::InteractionWithParticipants => "interaction_with_participants",
            EngagementKind::ProspectiveDataCollection => "prospective_data_collection",
            EngagementKind::ExpertPerformanceStudy => "expert_performance_study",
            EngagementKind::ClinicalWorkflowObservation => "clinical_workflow_observation",
            EngagementKind::SecondaryResearch => "secondary_research",
        }
    }

    /// The blueprint's own words. Not elaborated.
    pub const fn describe(self) -> &'static str {
        match self {
            EngagementKind::IdentifiableOrCodedData => "use of identifiable or coded data",
            EngagementKind::InteractionWithParticipants => "interaction with research participants",
            EngagementKind::ProspectiveDataCollection => "prospective data collection",
            EngagementKind::ExpertPerformanceStudy => "expert performance studies",
            EngagementKind::ClinicalWorkflowObservation => "clinical workflow observation",
            EngagementKind::SecondaryResearch => "secondary research",
        }
    }
}

impl fmt::Display for EngagementKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a study proposes to give back to participants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnOfResults {
    /// Nothing goes back.
    NotReturned,
    /// Cohort-level findings go back.
    AggregateToParticipants,
    /// Person-level findings go back. This is the one that meets the research boundary.
    IndividualFindings,
}

impl ReturnOfResults {
    pub const fn as_str(self) -> &'static str {
        match self {
            ReturnOfResults::NotReturned => "not_returned",
            ReturnOfResults::AggregateToParticipants => "aggregate_to_participants",
            ReturnOfResults::IndividualFindings => "individual_findings",
        }
    }
}

impl fmt::Display for ReturnOfResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a study says it will do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyDescription {
    pub subject: String,
    pub engagements: BTreeSet<EngagementKind>,
    /// The purposes the study declares, in `bioprism-policy`'s closed enumeration. This crate has
    /// no purpose vocabulary of its own.
    pub declared_purposes: PurposeSet,
    pub return_of_results: ReturnOfResults,
}

impl StudyDescription {
    pub fn new(subject: impl Into<String>, declared_purposes: PurposeSet) -> Self {
        StudyDescription {
            subject: subject.into(),
            engagements: BTreeSet::new(),
            declared_purposes,
            return_of_results: ReturnOfResults::NotReturned,
        }
    }

    pub fn engaging(mut self, engagement: EngagementKind) -> Self {
        self.engagements.insert(engagement);
        self
    }

    pub fn returning(mut self, return_of_results: ReturnOfResults) -> Self {
        self.return_of_results = return_of_results;
        self
    }

    /// Runs `bioprism-policy`'s consent check for every declared purpose.
    ///
    /// The first refusal wins and carries policy's sentence. Purposes are iterated in the closed
    /// enumeration's order so the answer does not depend on insertion order.
    pub fn check_consent(&self, consent: &Consent, at: Timestamp) -> Result<(), BioethicsError> {
        for purpose in Purpose::ALL {
            if !self.declared_purposes.admits(purpose) {
                continue;
            }
            if let Err(refusal) = consent.check(purpose, at) {
                return Err(BioethicsError::PurposeOutsideConsent {
                    study: self.subject.clone(),
                    purpose: purpose.to_string(),
                    refusal: refusal.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Asks `bioprism-onco` whether this platform may produce the results the study returns.
    ///
    /// Only [`ReturnOfResults::IndividualFindings`] reaches the boundary, as
    /// `bioprism_onco::OutputUse::IndividualPrognosis`. The other two are not questions this crate
    /// has an opinion about.
    pub fn check_return_of_results(
        &self,
        boundary: &ResearchBoundary,
    ) -> Result<(), BioethicsError> {
        match self.return_of_results {
            ReturnOfResults::NotReturned | ReturnOfResults::AggregateToParticipants => Ok(()),
            ReturnOfResults::IndividualFindings => {
                boundary.check(OutputUse::IndividualPrognosis)?;
                Ok(())
            }
        }
    }
}

/// Why a screening produced no determination.
///
/// One variant, for the same reason `bioprism_sdk::sandbox::Enforcement` has one: there is no
/// second reason that is not an exemption in disguise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UndeterminedReason {
    /// The study declared no engagement, so there is nothing to screen. Not a clearance.
    NoEngagementWasDeclared,
}

impl UndeterminedReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            UndeterminedReason::NoEngagementWasDeclared => "no_engagement_was_declared",
        }
    }
}

impl fmt::Display for UndeterminedReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What screening a study description can conclude.
///
/// Two variants, neither of which is an exemption. See the module header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "determination", rename_all = "snake_case")]
pub enum Determination {
    /// At least one declared engagement triggers institutional review. The triggers are listed so
    /// the referral says what it is about.
    ReviewRequired { triggers: Vec<EngagementKind> },
    /// Nothing was concluded. Not a clearance.
    Undetermined { reason: UndeterminedReason },
}

impl Determination {
    pub const fn requires_review(&self) -> bool {
        matches!(self, Determination::ReviewRequired { .. })
    }

    pub fn triggers(&self) -> &[EngagementKind] {
        match self {
            Determination::ReviewRequired { triggers } => triggers,
            Determination::Undetermined { .. } => &[],
        }
    }
}

/// Screens a study description.
///
/// Every declared engagement is a trigger, so the rule is: any engagement means review is
/// required, and no engagement means nothing was determined. That is a shallow rule and it is the
/// deepest one 36.22 licenses — the module lists six engagements and does not grade them.
pub fn screen(study: &StudyDescription) -> Determination {
    if study.engagements.is_empty() {
        return Determination::Undetermined {
            reason: UndeterminedReason::NoEngagementWasDeclared,
        };
    }
    Determination::ReviewRequired {
        triggers: study.engagements.iter().copied().collect(),
    }
}

/// What a named institutional body decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordedOutcome {
    Approved,
    WaiverGranted,
    /// The one value in this crate that means "no review needed", and it is a transcription of
    /// somebody else's decision rather than an output of any function here.
    DeterminedNotHumanSubjectResearch,
}

impl RecordedOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            RecordedOutcome::Approved => "approved",
            RecordedOutcome::WaiverGranted => "waiver_granted",
            RecordedOutcome::DeterminedNotHumanSubjectResearch => {
                "determined_not_human_subject_research"
            }
        }
    }
}

impl fmt::Display for RecordedOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A transcription of an institutional determination.
///
/// Private fields and one constructor, so a determination with no attributable body cannot exist.
/// Nothing here authenticates the body, resolves the reference or checks that the reference says
/// what the outcome says it says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "InstitutionalDeterminationDocument")]
pub struct InstitutionalDetermination {
    study: String,
    body: String,
    reference: String,
    outcome: RecordedOutcome,
}

/// The wire shape of an [`InstitutionalDetermination`].
///
/// Unlike this crate's enforcement records, a determination *is* transportable: it describes an
/// event outside this process, so a decoded one is no weaker than a constructed one. It still goes
/// through the same emptiness checks, which is why the decode is fallible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstitutionalDeterminationDocument {
    pub study: String,
    pub body: String,
    pub reference: String,
    pub outcome: RecordedOutcome,
}

impl InstitutionalDetermination {
    /// Records what a body decided. Refuses a blank body or a blank reference.
    pub fn record(
        study: impl Into<String>,
        body: impl Into<String>,
        reference: impl Into<String>,
        outcome: RecordedOutcome,
    ) -> Result<Self, BioethicsError> {
        InstitutionalDetermination::try_from(InstitutionalDeterminationDocument {
            study: study.into(),
            body: body.into(),
            reference: reference.into(),
            outcome,
        })
    }

    pub fn study(&self) -> &str {
        &self.study
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn reference(&self) -> &str {
        &self.reference
    }

    pub const fn outcome(&self) -> RecordedOutcome {
        self.outcome
    }
}

impl TryFrom<InstitutionalDeterminationDocument> for InstitutionalDetermination {
    type Error = BioethicsError;

    fn try_from(document: InstitutionalDeterminationDocument) -> Result<Self, Self::Error> {
        if document.body.trim().is_empty() {
            return Err(BioethicsError::IncompleteInstitutionalDetermination {
                study: document.study,
                field: "body".to_string(),
            });
        }
        if document.reference.trim().is_empty() {
            return Err(BioethicsError::IncompleteInstitutionalDetermination {
                study: document.study,
                field: "reference".to_string(),
            });
        }
        Ok(InstitutionalDetermination {
            study: document.study,
            body: document.body,
            reference: document.reference,
            outcome: document.outcome,
        })
    }
}
