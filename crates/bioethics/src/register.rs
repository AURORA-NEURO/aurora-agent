//! The §36 remainder as data: forty-two required controls, six of them enforced.
//!
//! Each of the seven modules in scope ends its distinguishing content with a "Required controls"
//! block of exactly six bullets. Those forty-two bullets are transcribed here verbatim, including
//! the six belonging to the two modules this crate does not implement — a register that listed only
//! the controls someone got round to building would flatter itself.
//!
//! # The two numbers, and why they are never one number
//!
//! [`SafeguardRegister::counts`] reports declared and enforced separately and offers no total, no
//! ratio and no coverage percentage. Combining them requires deciding what a declaration is worth;
//! any answer makes one of the two numbers wrong. `bioprism-safety` reports the same shape over
//! section 13 for the same reason.
//!
//! # The finding
//!
//! Six of the forty-two are enforced, and **every one of them defends a claim this crate makes
//! about itself** — that it did not act on the physical world, did not refer an unassessed
//! capability, did not attribute a finding to a group, did not publish a representation summary
//! that dropped its unmeasured strata, did not verify a module on absent evidence, and did not
//! exempt a study from institutional review. Not one defends a perimeter, because a perimeter
//! control needs something this library does not have: a process boundary, a network, an
//! instrument, or a person. `bioprism-safety` reached the identical conclusion from the opposite
//! direction — a threat model rather than a control list — which is worth stating because two
//! independent routes to the same finding is the strongest evidence available here that the
//! finding is about the architecture and not about either author's taste.

use crate::safeguard::{
    BlueprintModule, ControlSurface, DeclaredSafeguard, Impossibility, Safeguard,
};
use serde::Serialize;

/// Declared and enforced counts, side by side and never summed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RegisterCounts {
    pub declared: usize,
    pub enforced: usize,
}

/// The §36 required-control lists, in blueprint order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SafeguardRegister {
    entries: Vec<Safeguard>,
}

impl SafeguardRegister {
    pub fn entries(&self) -> &[Safeguard] {
        &self.entries
    }

    pub fn for_module(&self, module: BlueprintModule) -> Vec<&Safeguard> {
        self.entries
            .iter()
            .filter(|entry| entry.module() == module)
            .collect()
    }

    pub fn enforced(&self) -> Vec<&Safeguard> {
        self.entries
            .iter()
            .filter(|entry| entry.is_enforced())
            .collect()
    }

    pub fn declared(&self) -> Vec<&Safeguard> {
        self.entries
            .iter()
            .filter(|entry| !entry.is_enforced())
            .collect()
    }

    /// The two numbers. There is deliberately no `total` and no `share`.
    pub fn counts(&self) -> RegisterCounts {
        RegisterCounts {
            declared: self.declared().len(),
            enforced: self.enforced().len(),
        }
    }

    /// The controls belonging to modules this crate read and did not implement.
    ///
    /// They are in the register so that the position is visible; they are all declared, and
    /// [`BlueprintModule::deferred_to`] says where the workspace's actual position lives.
    pub fn deferred(&self) -> Vec<&Safeguard> {
        self.entries
            .iter()
            .filter(|entry| !entry.module().is_implemented_here())
            .collect()
    }
}

fn declared(name: &str, module: BlueprintModule, surface: ControlSurface) -> Safeguard {
    let declared_in = match module.module_id() {
        Some(id) => format!("{id} required controls"),
        None => format!("§36 {} — required controls", module.title()),
    };
    Safeguard::Declared(DeclaredSafeguard::new(name, module, surface, declared_in))
}

fn enforced(name: &str, module: BlueprintModule, by: Impossibility) -> Safeguard {
    let declared_in = match module.module_id() {
        Some(id) => format!("{id} required controls"),
        None => format!("§36 {} — required controls", module.title()),
    };
    let safeguard = DeclaredSafeguard::new(name, module, ControlSurface::Claim, declared_in)
        .enforce(by)
        .expect("a claim-surface safeguard can be enforced");
    Safeguard::Enforced(safeguard)
}

/// The seven modules' required-control lists, transcribed.
///
/// The `ControlSurface` on each entry is this crate's classification, not the blueprint's; §36
/// lists controls without saying which need a runtime. The rule applied is the one at
/// [`ControlSurface`]: a control is a perimeter control when it has no effect unless something
/// outside this process does the work.
///
/// Two groups are worth knowing about before reading the list:
///
/// * The twelve entries belonging to *Sandboxing Untrusted Code and Research Artifacts* and to the
///   *Security, Privacy, and Safety Red-Team Program* are every one of them perimeter controls, and
///   that uniformity is why this crate implements neither module.
/// * The six entries of *Quality Management, Validation, and Release Gates* are the blueprint's own
///   "Required controls" block, which for that module alone lists six measurements rather than six
///   controls. They are transcribed as written; the crate root says why the substitution matters.
pub fn section_36_remainder() -> SafeguardRegister {
    use BlueprintModule as M;
    use ControlSurface::{Claim, Perimeter};

    let entries = vec![
        declared(
            "escape tests",
            M::SandboxingUntrustedCodeAndResearchArtifacts,
            Perimeter,
        ),
        declared(
            "secret canaries",
            M::SandboxingUntrustedCodeAndResearchArtifacts,
            Perimeter,
        ),
        declared(
            "filesystem and network policy",
            M::SandboxingUntrustedCodeAndResearchArtifacts,
            Perimeter,
        ),
        declared(
            "supply-chain scans",
            M::SandboxingUntrustedCodeAndResearchArtifacts,
            Perimeter,
        ),
        declared(
            "malware and macro controls",
            M::SandboxingUntrustedCodeAndResearchArtifacts,
            Perimeter,
        ),
        declared(
            "quarantine",
            M::SandboxingUntrustedCodeAndResearchArtifacts,
            Perimeter,
        ),
        enforced(
            "no physical execution in public MVP",
            M::PhysicalExperimentAndWetLabActionBoundaries,
            Impossibility::NoValueRepresentsAPerformedPhysicalAction,
        ),
        declared(
            "human approval",
            M::PhysicalExperimentAndWetLabActionBoundaries,
            Perimeter,
        ),
        declared(
            "institutional safety review",
            M::PhysicalExperimentAndWetLabActionBoundaries,
            Perimeter,
        ),
        declared(
            "restricted capability tiers",
            M::PhysicalExperimentAndWetLabActionBoundaries,
            Perimeter,
        ),
        declared(
            "sandbox simulation",
            M::PhysicalExperimentAndWetLabActionBoundaries,
            Perimeter,
        ),
        declared(
            "full audit",
            M::PhysicalExperimentAndWetLabActionBoundaries,
            Perimeter,
        ),
        enforced(
            "task risk classification",
            M::DualUseBiosecurityAndCapabilityRelease,
            Impossibility::NoReleaseReferralExistsForAnUnassessedTask,
        ),
        declared(
            "restricted packs",
            M::DualUseBiosecurityAndCapabilityRelease,
            Perimeter,
        ),
        declared(
            "expert safety review",
            M::DualUseBiosecurityAndCapabilityRelease,
            Perimeter,
        ),
        declared(
            "rate and tool limits",
            M::DualUseBiosecurityAndCapabilityRelease,
            Perimeter,
        ),
        declared(
            "responsible disclosure",
            M::DualUseBiosecurityAndCapabilityRelease,
            Perimeter,
        ),
        declared(
            "capability withholding",
            M::DualUseBiosecurityAndCapabilityRelease,
            Claim,
        ),
        declared(
            "worst-group calibration",
            M::FairnessRepresentationAndGlobalResourceContext,
            Claim,
        ),
        enforced(
            "coverage and abstention",
            M::FairnessRepresentationAndGlobalResourceContext,
            Impossibility::NoRepresentationSummaryOmitsItsUnmeasuredStrata,
        ),
        declared(
            "resource-constrained architecture",
            M::FairnessRepresentationAndGlobalResourceContext,
            Perimeter,
        ),
        enforced(
            "bias-source analysis",
            M::FairnessRepresentationAndGlobalResourceContext,
            Impossibility::NoFindingIsAttributedToAGroupRatherThanAContext,
        ),
        declared(
            "small-group protection",
            M::FairnessRepresentationAndGlobalResourceContext,
            Claim,
        ),
        declared(
            "community review",
            M::FairnessRepresentationAndGlobalResourceContext,
            Perimeter,
        ),
        declared(
            "attack library",
            M::SecurityPrivacySafetyRedTeamProgram,
            Perimeter,
        ),
        declared(
            "independent team",
            M::SecurityPrivacySafetyRedTeamProgram,
            Perimeter,
        ),
        declared(
            "canary assets",
            M::SecurityPrivacySafetyRedTeamProgram,
            Perimeter,
        ),
        declared(
            "severity and remediation",
            M::SecurityPrivacySafetyRedTeamProgram,
            Perimeter,
        ),
        declared(
            "regression packs",
            M::SecurityPrivacySafetyRedTeamProgram,
            Perimeter,
        ),
        declared(
            "public disclosure policy",
            M::SecurityPrivacySafetyRedTeamProgram,
            Perimeter,
        ),
        declared(
            "gate pass rate",
            M::QualityManagementValidationAndReleaseGates,
            Claim,
        ),
        declared(
            "open risk debt",
            M::QualityManagementValidationAndReleaseGates,
            Claim,
        ),
        enforced(
            "validation coverage",
            M::QualityManagementValidationAndReleaseGates,
            Impossibility::NoVerifiedModuleExistsWithUnmetEvidence,
        ),
        declared(
            "post-release defect",
            M::QualityManagementValidationAndReleaseGates,
            Perimeter,
        ),
        declared(
            "reproducibility",
            M::QualityManagementValidationAndReleaseGates,
            Claim,
        ),
        declared(
            "audit findings",
            M::QualityManagementValidationAndReleaseGates,
            Perimeter,
        ),
        enforced(
            "institutional determination",
            M::ResearchEthicsIrbAndHumanSubjectBoundaries,
            Impossibility::NoScreeningResultRecordsAnInstitutionalExemption,
        ),
        declared(
            "consent or waiver",
            M::ResearchEthicsIrbAndHumanSubjectBoundaries,
            Claim,
        ),
        declared(
            "minimal-risk assessment",
            M::ResearchEthicsIrbAndHumanSubjectBoundaries,
            Perimeter,
        ),
        declared(
            "data-security plan",
            M::ResearchEthicsIrbAndHumanSubjectBoundaries,
            Perimeter,
        ),
        declared(
            "publication and return-of-results policy",
            M::ResearchEthicsIrbAndHumanSubjectBoundaries,
            Claim,
        ),
        declared(
            "ongoing review",
            M::ResearchEthicsIrbAndHumanSubjectBoundaries,
            Perimeter,
        ),
    ];

    SafeguardRegister { entries }
}
