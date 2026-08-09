//! Declared safeguards and enforced ones, as two types that cannot be confused.
//!
//! This module is the spine of the crate. Every §36 module in scope names a list of "required
//! controls", and the natural implementation records each one as a string with a boolean beside
//! it. Under that encoding a control that a person performs, a control that a runtime applies and
//! a control that a Rust type makes unbreakable all look the same, and the register becomes an
//! inventory of intentions.
//!
//! So there are two types. [`DeclaredSafeguard`] is a control §36 names and nothing in this
//! process applies. [`EnforcedSafeguard`] is a control whose violation is *not representable* in
//! this crate's types, and the only way to obtain one is
//! [`DeclaredSafeguard::enforce`], which demands an [`Impossibility`] — a named, checkable fact
//! about what this crate's public API cannot express. There is no `From<DeclaredSafeguard>`, no
//! `promote`, no builder and no `Deserialize`.
//!
//! # Why enforcement does not survive serialization
//!
//! [`Safeguard`] serializes both states, and decoding one that claims enforcement fails with
//! [`BioethicsError::EnforcementNotTransportable`]. Enforcement here is a statement about the
//! compiled crate: *no value of this shape exists*. A JSON document cannot witness that, and
//! quietly demoting the record to declared on the way in would be a second lie in place of the
//! first. Failing is the honest decode.
//!
//! # Why a perimeter control can never be enforced
//!
//! [`ControlSurface`] splits the §36 control lists in two. A **claim** control governs what this
//! workspace asserts about itself; a type here can make one unbreakable. A **perimeter** control
//! needs a runtime, a network, an instrument or a person to have any effect at all; nothing in a
//! single-process library with no I/O can apply one. [`DeclaredSafeguard::enforce`] refuses every
//! perimeter control with [`BioethicsError::PerimeterCannotBeEnforced`], and a test in
//! `tests/safeguard.rs` fails if the shipped register ever contains an enforced perimeter entry.
//! `bioprism-safety` holds exactly this line over section 13's threats, and
//! `bioprism_sdk::sandbox::Enforcement` holds it with a single variant; this is the same rule in
//! the currency of §36's control lists.
//!
//! # This is not a second threat model
//!
//! A [`Safeguard`] carries a blueprint module and a control surface. It carries no asset, no
//! adversary, no capability and no attack class, because `bioprism-safety` owns that vocabulary
//! and a second copy of it would drift. The register here is indexed by *which §36 module asked
//! for the control*, which is the question section 13's model cannot answer and this one can.
//!
//! # What is not modelled
//!
//! No control effectiveness, no residual-risk score, no priority and no coverage percentage over
//! the control list. Counting declared and enforced together would require deciding what a
//! declaration is worth, and every answer to that is wrong in one direction.

use crate::error::BioethicsError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The seven §36 modules that were still uncovered when this crate was written.
///
/// Five are implemented here and carry their blueprint id. Two are not, and
/// [`BlueprintModule::module_id`] returns `None` for them: `tools/coverage.sh` counts any `NN.MM`
/// token found under `crates/`, so writing their ids into this crate would move a coverage number
/// without moving a capability. They are named by title instead, and the crate root says why each
/// was left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintModule {
    /// Implemented by [`crate::action`].
    PhysicalExperimentAndWetLabActionBoundaries,
    /// Implemented by [`crate::dualuse`].
    DualUseBiosecurityAndCapabilityRelease,
    /// Implemented by [`crate::representation`].
    FairnessRepresentationAndGlobalResourceContext,
    /// Implemented by [`crate::validation`].
    QualityManagementValidationAndReleaseGates,
    /// Implemented by [`crate::humansubject`].
    ResearchEthicsIrbAndHumanSubjectBoundaries,
    /// Not implemented. Every item in its scope and control lists is runtime infrastructure that
    /// `bioprism-safety` and `bioprism-sdk` have already taken this workspace's position on.
    SandboxingUntrustedCodeAndResearchArtifacts,
    /// Not implemented. A programme: an independent team, a severity scale, a remediation workflow
    /// and a disclosure policy. `bioprism-safety` owns the corpus and the disclosure ladder.
    SecurityPrivacySafetyRedTeamProgram,
}

impl BlueprintModule {
    /// Every module whose controls appear in the shipped register, in blueprint order.
    pub const ALL: [BlueprintModule; 7] = [
        BlueprintModule::SandboxingUntrustedCodeAndResearchArtifacts,
        BlueprintModule::PhysicalExperimentAndWetLabActionBoundaries,
        BlueprintModule::DualUseBiosecurityAndCapabilityRelease,
        BlueprintModule::FairnessRepresentationAndGlobalResourceContext,
        BlueprintModule::SecurityPrivacySafetyRedTeamProgram,
        BlueprintModule::QualityManagementValidationAndReleaseGates,
        BlueprintModule::ResearchEthicsIrbAndHumanSubjectBoundaries,
    ];

    /// The dotted blueprint id, for the five modules this crate implements.
    ///
    /// `None` is not "unknown". It means the module was read, classified and deliberately left
    /// uncited, which is a position rather than an omission.
    pub const fn module_id(self) -> Option<&'static str> {
        match self {
            BlueprintModule::PhysicalExperimentAndWetLabActionBoundaries => Some("36.10"),
            BlueprintModule::DualUseBiosecurityAndCapabilityRelease => Some("36.11"),
            BlueprintModule::FairnessRepresentationAndGlobalResourceContext => Some("36.13"),
            BlueprintModule::QualityManagementValidationAndReleaseGates => Some("36.21"),
            BlueprintModule::ResearchEthicsIrbAndHumanSubjectBoundaries => Some("36.22"),
            BlueprintModule::SandboxingUntrustedCodeAndResearchArtifacts
            | BlueprintModule::SecurityPrivacySafetyRedTeamProgram => None,
        }
    }

    /// The module's blueprint title.
    pub const fn title(self) -> &'static str {
        match self {
            BlueprintModule::PhysicalExperimentAndWetLabActionBoundaries => {
                "Physical Experiment and Wet-Lab Action Boundaries"
            }
            BlueprintModule::DualUseBiosecurityAndCapabilityRelease => {
                "Dual-Use Biosecurity and Capability Release"
            }
            BlueprintModule::FairnessRepresentationAndGlobalResourceContext => {
                "Fairness, Representation, and Global Resource Context"
            }
            BlueprintModule::QualityManagementValidationAndReleaseGates => {
                "Quality Management, Validation, and Release Gates"
            }
            BlueprintModule::ResearchEthicsIrbAndHumanSubjectBoundaries => {
                "Research Ethics, IRB, and Human-Subject Boundaries"
            }
            BlueprintModule::SandboxingUntrustedCodeAndResearchArtifacts => {
                "Sandboxing Untrusted Code and Research Artifacts"
            }
            BlueprintModule::SecurityPrivacySafetyRedTeamProgram => {
                "Security, Privacy, and Safety Red-Team Program"
            }
        }
    }

    /// Whether this crate implements the module, as opposed to holding its controls as declared.
    pub const fn is_implemented_here(self) -> bool {
        self.module_id().is_some()
    }

    /// Where a reader should go for the workspace's position on a module this crate left out.
    pub const fn deferred_to(self) -> Option<&'static str> {
        match self {
            BlueprintModule::SandboxingUntrustedCodeAndResearchArtifacts => Some(
                "bioprism-safety (sandbox, untrusted code, supply chain, quarantine) and \
                      bioprism-sdk (isolation class, declared-only enforcement)",
            ),
            BlueprintModule::SecurityPrivacySafetyRedTeamProgram => Some(
                "bioprism-safety (red-team corpus, vulnerability ladder, responsible disclosure)",
            ),
            _ => None,
        }
    }
}

impl fmt::Display for BlueprintModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.module_id() {
            Some(id) => write!(f, "{id} {}", self.title()),
            None => write!(f, "§36 {}", self.title()),
        }
    }
}

/// What a control acts on.
///
/// The split decides which controls could ever be enforced by a type. It is this crate's, not the
/// blueprint's: §36 lists controls without saying which of them run inside a process, and the
/// derivation is written here so a reader disagreeing with a placement can see the rule they are
/// disagreeing with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlSurface {
    /// Needs a runtime, a network, an instrument or a person. Nothing in this library applies one.
    Perimeter,
    /// Governs what this workspace asserts about itself. A type here can make one unbreakable.
    Claim,
}

impl ControlSurface {
    pub const fn as_str(self) -> &'static str {
        match self {
            ControlSurface::Perimeter => "perimeter",
            ControlSurface::Claim => "claim",
        }
    }
}

impl fmt::Display for ControlSurface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A fact about this crate's public API that makes a violation unrepresentable.
///
/// Closed, and every variant is checkable by reading the module it names. This is the analogue of
/// `bioprism_safety::threat::Unrepresentable`: a value here is the *only* thing that can turn a
/// declaration into an enforcement, so adding a variant is a visible change to a public enum
/// rather than a quiet change to a boolean.
///
/// Deliberately absent: any variant naming a sandbox, a monitor, a reviewer, an approval workflow
/// or a control plane. None of those runs in this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Impossibility {
    /// [`crate::action`] has no type representing a performed physical action and no function that
    /// takes a plan and does anything. The furthest a physical step can travel is
    /// [`crate::action::PhysicalReferral`], which is addressed to a process outside the workspace.
    NoValueRepresentsAPerformedPhysicalAction,
    /// [`crate::dualuse::ReleaseReferral`] has private fields and one constructor, which refuses a
    /// task whose misuse surfaces were never assessed.
    NoReleaseReferralExistsForAnUnassessedTask,
    /// [`crate::representation::Attribution`] has no variant assigning a finding to a group. A
    /// finding is attributable to a context or to nothing.
    NoFindingIsAttributedToAGroupRatherThanAContext,
    /// [`crate::representation::RepresentationSummary`] has private fields and one constructor,
    /// and every summary carries its unmeasured strata alongside its worst measured one.
    NoRepresentationSummaryOmitsItsUnmeasuredStrata,
    /// [`crate::validation::VerifiedModule`] has private fields, one constructor and no
    /// `Deserialize`; the constructor refuses a dossier with any evidence kind absent.
    NoVerifiedModuleExistsWithUnmetEvidence,
    /// [`crate::humansubject::Determination`] has no variant meaning exempt or
    /// not-human-subject-research. Screening can raise the flag and can never lower it.
    NoScreeningResultRecordsAnInstitutionalExemption,
}

impl Impossibility {
    pub const ALL: [Impossibility; 6] = [
        Impossibility::NoValueRepresentsAPerformedPhysicalAction,
        Impossibility::NoReleaseReferralExistsForAnUnassessedTask,
        Impossibility::NoFindingIsAttributedToAGroupRatherThanAContext,
        Impossibility::NoRepresentationSummaryOmitsItsUnmeasuredStrata,
        Impossibility::NoVerifiedModuleExistsWithUnmetEvidence,
        Impossibility::NoScreeningResultRecordsAnInstitutionalExemption,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Impossibility::NoValueRepresentsAPerformedPhysicalAction => {
                "no_value_represents_a_performed_physical_action"
            }
            Impossibility::NoReleaseReferralExistsForAnUnassessedTask => {
                "no_release_referral_exists_for_an_unassessed_task"
            }
            Impossibility::NoFindingIsAttributedToAGroupRatherThanAContext => {
                "no_finding_is_attributed_to_a_group_rather_than_a_context"
            }
            Impossibility::NoRepresentationSummaryOmitsItsUnmeasuredStrata => {
                "no_representation_summary_omits_its_unmeasured_strata"
            }
            Impossibility::NoVerifiedModuleExistsWithUnmetEvidence => {
                "no_verified_module_exists_with_unmet_evidence"
            }
            Impossibility::NoScreeningResultRecordsAnInstitutionalExemption => {
                "no_screening_result_records_an_institutional_exemption"
            }
        }
    }

    /// Which module of this crate a reader must read to check the claim.
    pub const fn checkable_in(self) -> &'static str {
        match self {
            Impossibility::NoValueRepresentsAPerformedPhysicalAction => "action",
            Impossibility::NoReleaseReferralExistsForAnUnassessedTask => "dualuse",
            Impossibility::NoFindingIsAttributedToAGroupRatherThanAContext
            | Impossibility::NoRepresentationSummaryOmitsItsUnmeasuredStrata => "representation",
            Impossibility::NoVerifiedModuleExistsWithUnmetEvidence => "validation",
            Impossibility::NoScreeningResultRecordsAnInstitutionalExemption => "humansubject",
        }
    }
}

impl fmt::Display for Impossibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rust-type-system({})", self.as_str())
    }
}

/// A control §36 names and nothing in this process applies.
///
/// The honest state of most of §36 as far as this workspace is concerned.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeclaredSafeguard {
    name: String,
    module: BlueprintModule,
    surface: ControlSurface,
    declared_in: String,
}

impl DeclaredSafeguard {
    /// Records a control as declared.
    ///
    /// `declared_in` is where a reader goes to check the declaration: a blueprint module's
    /// required-controls list, a sibling crate, a deployment runbook.
    pub fn new(
        name: impl Into<String>,
        module: BlueprintModule,
        surface: ControlSurface,
        declared_in: impl Into<String>,
    ) -> Self {
        DeclaredSafeguard {
            name: name.into(),
            module,
            surface,
            declared_in: declared_in.into(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn module(&self) -> BlueprintModule {
        self.module
    }

    pub const fn surface(&self) -> ControlSurface {
        self.surface
    }

    pub fn declared_in(&self) -> &str {
        &self.declared_in
    }

    /// The only path from a declaration to an enforcement.
    ///
    /// Refuses every perimeter control. The [`Impossibility`] argument is not decoration: it names
    /// the module whose types a reader must inspect to falsify the claim, and it is the reason
    /// this method cannot be called reflexively for a control that merely sounds strong.
    pub fn enforce(self, by: Impossibility) -> Result<EnforcedSafeguard, BioethicsError> {
        if self.surface == ControlSurface::Perimeter {
            return Err(BioethicsError::PerimeterCannotBeEnforced {
                safeguard: self.name,
                module: self.module,
                surface: ControlSurface::Perimeter,
            });
        }
        Ok(EnforcedSafeguard {
            name: self.name,
            module: self.module,
            by,
        })
    }

    /// Refuses, always.
    ///
    /// Relying on a declaration is the failure this whole module exists to make visible, so it is
    /// a `?` at the call site rather than a judgement call in a reviewer's head. The same shape as
    /// `bioprism_safety::threat::Threat::rely`.
    pub fn rely(&self) -> Result<(), BioethicsError> {
        Err(BioethicsError::UnenforcedReliance {
            safeguard: self.name.clone(),
            declared_in: self.declared_in.clone(),
        })
    }
}

/// A control whose violation is not representable in this crate's types.
///
/// No `Deserialize`. Possessing one is evidence that [`DeclaredSafeguard::enforce`] ran and named
/// an [`Impossibility`], which is exactly the property a decoded value would not have.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct EnforcedSafeguard {
    name: String,
    module: BlueprintModule,
    by: Impossibility,
}

impl EnforcedSafeguard {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn module(&self) -> BlueprintModule {
        self.module
    }

    pub const fn enforced_by(&self) -> Impossibility {
        self.by
    }

    /// Always a claim control. There is no field to hold anything else.
    pub const fn surface(&self) -> ControlSurface {
        ControlSurface::Claim
    }
}

/// One control from a §36 required-controls list, in one of its two states.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(into = "SafeguardDocument")]
pub enum Safeguard {
    Declared(DeclaredSafeguard),
    Enforced(EnforcedSafeguard),
}

impl Safeguard {
    pub fn name(&self) -> &str {
        match self {
            Safeguard::Declared(declared) => declared.name(),
            Safeguard::Enforced(enforced) => enforced.name(),
        }
    }

    pub const fn module(&self) -> BlueprintModule {
        match self {
            Safeguard::Declared(declared) => declared.module,
            Safeguard::Enforced(enforced) => enforced.module,
        }
    }

    pub const fn surface(&self) -> ControlSurface {
        match self {
            Safeguard::Declared(declared) => declared.surface,
            Safeguard::Enforced(_) => ControlSurface::Claim,
        }
    }

    pub const fn is_enforced(&self) -> bool {
        matches!(self, Safeguard::Enforced(_))
    }

    /// The impossibility backing an enforced safeguard, if this is one.
    pub const fn impossibility(&self) -> Option<Impossibility> {
        match self {
            Safeguard::Declared(_) => None,
            Safeguard::Enforced(enforced) => Some(enforced.by),
        }
    }

    /// Refuses for a declaration, succeeds for an enforcement.
    pub fn rely(&self) -> Result<(), BioethicsError> {
        match self {
            Safeguard::Declared(declared) => declared.rely(),
            Safeguard::Enforced(_) => Ok(()),
        }
    }
}

/// The wire shape of a [`Safeguard`].
///
/// Public because a caller reading a register from disk needs to name the type its decode failed
/// on. Decoding an enforced document is the failure — see [`Safeguard`]'s `TryFrom`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafeguardDocument {
    pub name: String,
    pub module: BlueprintModule,
    pub surface: ControlSurface,
    pub enforcement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_in: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impossibility: Option<Impossibility>,
}

/// The word a declared safeguard carries on the wire.
pub const DECLARED: &str = "declared";
/// The word an enforced safeguard serializes to, and which never decodes.
pub const ENFORCED: &str = "enforced";

impl From<Safeguard> for SafeguardDocument {
    fn from(safeguard: Safeguard) -> Self {
        match safeguard {
            Safeguard::Declared(declared) => SafeguardDocument {
                name: declared.name,
                module: declared.module,
                surface: declared.surface,
                enforcement: DECLARED.to_string(),
                declared_in: Some(declared.declared_in),
                impossibility: None,
            },
            Safeguard::Enforced(enforced) => SafeguardDocument {
                name: enforced.name,
                module: enforced.module,
                surface: ControlSurface::Claim,
                enforcement: ENFORCED.to_string(),
                declared_in: None,
                impossibility: Some(enforced.by),
            },
        }
    }
}

impl TryFrom<SafeguardDocument> for Safeguard {
    type Error = BioethicsError;

    fn try_from(document: SafeguardDocument) -> Result<Self, Self::Error> {
        match document.enforcement.as_str() {
            DECLARED => Ok(Safeguard::Declared(DeclaredSafeguard::new(
                document.name,
                document.module,
                document.surface,
                document.declared_in.unwrap_or_default(),
            ))),
            ENFORCED => Err(BioethicsError::EnforcementNotTransportable {
                safeguard: document.name,
            }),
            other => Err(BioethicsError::UnknownEnforcementState {
                safeguard: document.name,
                state: other.to_string(),
            }),
        }
    }
}

impl<'de> Deserialize<'de> for Safeguard {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let document = SafeguardDocument::deserialize(deserializer)?;
        Safeguard::try_from(document).map_err(serde::de::Error::custom)
    }
}
