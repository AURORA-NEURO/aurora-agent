//! Blueprint 36.11 — dual-use biosecurity and capability release.
//!
//! 36.11's purpose sentence: "evaluate and release biological capabilities according to plausible
//! misuse, not only benchmark performance."
//!
//! # There is no second release gate here
//!
//! `bioprism-safety` implements 13.26 in full: nine risk dimensions where unrated blocks the gate,
//! a two-high-blocks decision rule written in the open because 13.26 states no threshold, the
//! promotion check that refuses an unmeasured safety delta, and the withholding rule that permits
//! suppressing exploit detail and refuses suppressing the existence of a finding.
//! [`refer`] **calls** that gate. It does not reimplement any part of it, and
//! [`ReleaseReferral::decision`] is `bioprism_safety::release::GateDecision` unchanged, so a
//! caller reading a referral is reading section 13's verdict rather than a §36 paraphrase of it.
//!
//! # What §36 adds that §13 does not have
//!
//! One thing, and it is the whole module. 13.26's gate refuses an assessment with an *unrated
//! dimension*; it has nothing to say about a task nobody ever asked the misuse question about.
//! [`SurfaceAssessment`] has two states, and `Assessed` with an empty surface set — "we looked and
//! found none" — is a different value from [`SurfaceAssessment::NotAssessed`]. A referral cannot
//! be built from the second. This is the workspace's standing rule that unmeasured is not zero,
//! applied to misuse rather than to capability.
//!
//! # The correspondence this crate refuses to invent
//!
//! 36.11's Scope names six misuse surfaces. 13.26's `SensitiveCategory` names six sensitive
//! categories. They are different lists, drawn up for different purposes, and **no module of the
//! blueprint relates one to the other**. A mapping here would look like transcription and would be
//! this crate's invention, so [`refer`] requires the caller to have stated both and fails with
//! [`crate::BioethicsError::SensitiveCategoryUnstated`] when the risk assessment carries no
//! category.
//!
//! # What 36.11 names and never specifies
//!
//! * **"plausible misuse"** — no scale, no likelihood, no time horizon. [`SurfaceAssessment`]
//!   records which surfaces a named assessor identified and attaches no score to any of them.
//! * **"task risk classification"** — the classes are never enumerated. The six here are 36.11's
//!   own Scope bullets, which is the closest thing the module offers and is not the same thing.
//! * **"rate and tool limits"** — no rate, no limit, and nothing in this process could apply one.
//! * **"restricted packs"** — no restriction mechanism. `bioprism-registry` owns trust tiers over
//!   pack bytes; this crate cannot set or read one.
//! * **"expert safety review"** — no expertise criterion, no independence criterion, no quorum.
//! * **"capability withholding"** — 36.11 states no criterion for what may be withheld. 13.26 does
//!   state one, and [`ReleaseReferral::withhold`] defers to it rather than adding a second.
//!
//! # Not implemented
//!
//! No content classification of any kind. Nothing here reads a sequence, a prompt, a pack or a
//! model output and decides what it is about; a [`MisuseSurface`] is a label a named human applied.
//! No rate limiter, no quota, no pack restriction, no reviewer registry, no disclosure timeline and
//! no clock.

use crate::error::BioethicsError;
use bioprism_safety::release::{
    withhold, GateDecision, ReleaseGate, RiskAssessment, SensitiveCategory, WithholdScope,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The six misuse surfaces of 36.11's Scope, transcribed.
///
/// Closed, because a surface outside this list is one the module never contemplated and the honest
/// response is to amend the list visibly rather than to admit a free-text label that nothing can
/// aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MisuseSurface {
    SequenceDesign,
    PathogenRelevantAnalysis,
    ExperimentalExecutionAutomation,
    ScreeningEvasion,
    ToxinOrVirulenceOptimisation,
    SensitiveLiteratureSynthesis,
}

impl MisuseSurface {
    pub const ALL: [MisuseSurface; 6] = [
        MisuseSurface::SequenceDesign,
        MisuseSurface::PathogenRelevantAnalysis,
        MisuseSurface::ExperimentalExecutionAutomation,
        MisuseSurface::ScreeningEvasion,
        MisuseSurface::ToxinOrVirulenceOptimisation,
        MisuseSurface::SensitiveLiteratureSynthesis,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            MisuseSurface::SequenceDesign => "sequence_design",
            MisuseSurface::PathogenRelevantAnalysis => "pathogen_relevant_analysis",
            MisuseSurface::ExperimentalExecutionAutomation => "experimental_execution_automation",
            MisuseSurface::ScreeningEvasion => "screening_evasion",
            MisuseSurface::ToxinOrVirulenceOptimisation => "toxin_or_virulence_optimisation",
            MisuseSurface::SensitiveLiteratureSynthesis => "sensitive_literature_synthesis",
        }
    }

    /// The blueprint's own words for the surface. No elaboration, because elaborating would mean
    /// adding biology the module does not state.
    pub const fn describe(self) -> &'static str {
        match self {
            MisuseSurface::SequenceDesign => "sequence design",
            MisuseSurface::PathogenRelevantAnalysis => "pathogen-relevant analysis",
            MisuseSurface::ExperimentalExecutionAutomation => {
                "automation of experimental execution"
            }
            MisuseSurface::ScreeningEvasion => "screening evasion",
            MisuseSurface::ToxinOrVirulenceOptimisation => "toxin or virulence optimization",
            MisuseSurface::SensitiveLiteratureSynthesis => "sensitive literature synthesis",
        }
    }
}

impl fmt::Display for MisuseSurface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether anybody asked the misuse question, and what they found.
///
/// The two states are not orderable and one is not a special case of the other. An empty
/// [`SurfaceAssessment::Assessed`] is a finding; [`SurfaceAssessment::NotAssessed`] is the absence
/// of one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "assessment", rename_all = "snake_case")]
pub enum SurfaceAssessment {
    /// Nobody asked. Not a synonym for "no surface".
    NotAssessed,
    /// A named assessor looked and recorded what they found, possibly nothing.
    Assessed {
        surfaces: BTreeSet<MisuseSurface>,
        /// Who assessed. Nothing authenticates this; it exists so a referral is attributable.
        assessor: String,
    },
}

impl SurfaceAssessment {
    /// Records an assessment. An empty iterator produces an assessed-and-empty value, which is the
    /// point of the type.
    pub fn assessed<I: IntoIterator<Item = MisuseSurface>>(
        assessor: impl Into<String>,
        surfaces: I,
    ) -> Self {
        SurfaceAssessment::Assessed {
            surfaces: surfaces.into_iter().collect(),
            assessor: assessor.into(),
        }
    }

    pub const fn was_assessed(&self) -> bool {
        matches!(self, SurfaceAssessment::Assessed { .. })
    }

    /// The surfaces found, or `None` when nobody looked. Deliberately not an empty set for the
    /// unassessed case: collapsing the two is the failure this type prevents.
    pub fn surfaces(&self) -> Option<&BTreeSet<MisuseSurface>> {
        match self {
            SurfaceAssessment::NotAssessed => None,
            SurfaceAssessment::Assessed { surfaces, .. } => Some(surfaces),
        }
    }
}

/// A capability someone proposes to release, and the misuse question's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRelease {
    pub subject: String,
    pub assessment: SurfaceAssessment,
}

impl CapabilityRelease {
    pub fn new(subject: impl Into<String>, assessment: SurfaceAssessment) -> Self {
        CapabilityRelease {
            subject: subject.into(),
            assessment,
        }
    }
}

/// A capability release that reached section 13's gate, and what the gate said.
///
/// # Why there is no `Deserialize`
///
/// Possessing one is evidence that the misuse question was asked and that
/// `bioprism_safety::release::ReleaseGate` ran on a fully rated assessment. A decoded value would
/// have neither property, and the gate's own refusals — an unrated dimension in particular — would
/// be bypassed by a config file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReleaseReferral {
    subject: String,
    surfaces: BTreeSet<MisuseSurface>,
    assessor: String,
    category: SensitiveCategory,
    decision: GateDecision,
}

impl ReleaseReferral {
    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub fn surfaces(&self) -> &BTreeSet<MisuseSurface> {
        &self.surfaces
    }

    pub fn assessor(&self) -> &str {
        &self.assessor
    }

    pub const fn category(&self) -> SensitiveCategory {
        self.category
    }

    /// Section 13's verdict, unaltered.
    pub const fn decision(&self) -> &GateDecision {
        &self.decision
    }

    /// Applies 13.26's withholding rule, which this crate does not re-decide.
    ///
    /// [`WithholdScope::Existence`] is refused by `bioprism-safety` with its own error, because a
    /// safety process that can delete the fact that a weakness exists is a reputation process
    /// wearing a safety process's badge. That sentence is 13.26's and the check is
    /// `bioprism-safety`'s; this method exists so a caller holding a referral does not have to
    /// reach past it to find the rule.
    pub fn withhold(
        &self,
        finding: &str,
        scope: WithholdScope,
    ) -> Result<WithholdScope, BioethicsError> {
        withhold(finding, scope).map_err(BioethicsError::from)
    }
}

/// The only constructor for a [`ReleaseReferral`].
///
/// Order matters. The misuse question is asked before the risk assessment is looked at, so a fully
/// rated assessment on an unassessed task fails on the thing 36.11 is about rather than passing on
/// the thing 13.26 is about.
pub fn refer(
    release: &CapabilityRelease,
    risk: &RiskAssessment,
) -> Result<ReleaseReferral, BioethicsError> {
    let (surfaces, assessor) = match &release.assessment {
        SurfaceAssessment::NotAssessed => {
            return Err(BioethicsError::MisuseSurfacesUnassessed {
                subject: release.subject.clone(),
            })
        }
        SurfaceAssessment::Assessed { surfaces, assessor } => (surfaces.clone(), assessor.clone()),
    };

    if risk.subject != release.subject {
        return Err(BioethicsError::AssessmentSubjectMismatch {
            release: release.subject.clone(),
            assessment: risk.subject.clone(),
        });
    }

    let Some(category) = risk.category else {
        return Err(BioethicsError::SensitiveCategoryUnstated {
            subject: release.subject.clone(),
        });
    };

    let decision = ReleaseGate.decide(risk)?;

    Ok(ReleaseReferral {
        subject: release.subject.clone(),
        surfaces,
        assessor,
        category,
        decision,
    })
}
