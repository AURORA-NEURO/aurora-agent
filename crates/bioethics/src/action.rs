//! Blueprint 36.10 — physical experiment and wet-lab action boundaries.
//!
//! 36.10's purpose sentence is the whole module: "keep initial releases in simulation, analysis,
//! and reviewed protocol planning; physical execution requires separate controls". Its first
//! required control is *no physical execution in public MVP*.
//!
//! # The control is implemented by an absence
//!
//! There is no `PerformedAction` type here, no `ActionResult`, no `execute`, no driver, no device
//! handle and no transport. The furthest a physical step travels is [`PhysicalReferral`], which
//! names a plan, its physical steps and the two humans who authorised it, and which nothing in
//! this workspace consumes. That absence is the enforcement, and it is registered as
//! [`crate::safeguard::Impossibility::NoValueRepresentsAPerformedPhysicalAction`] rather than as a
//! sentence in a document, so that adding an executor would require adding a public type and
//! deleting a register entry rather than quietly adding a method.
//!
//! This is the only honest reading available. A single-process library cannot *prevent* a caller
//! from driving a liquid handler; it can decline to be the thing that does it, and it can refuse
//! to hold a value asserting that it happened.
//!
//! # Why the plan is split rather than refused
//!
//! §36's shared policy-evaluation-cell list asks whether a platform "refuses or escalates a
//! prohibited action while completing safe portions". A boundary that refuses an entire plan
//! because one step touches a pipette destroys the simulation and analysis 36.10 explicitly wants
//! to keep. [`ActionPlan::partition`] is therefore a splitter, the same shape
//! `bioprism_onco::ResearchBoundary::triage` uses for outputs, and
//! [`PlanDisposition::in_silico_steps`] is non-empty whenever anything was salvageable.
//!
//! # Where the research boundary is consulted rather than re-decided
//!
//! A plan carries the `bioprism_onco::OutputUse` its results are for, and
//! [`ActionPlan::partition`] runs `bioprism_onco::ResearchBoundary::check` **first**, before it
//! looks at a single step. A physical action taken to produce individual clinical direction is
//! 30.30's compound failure — "letting a research agent execute clinical actions" — and the
//! refusal that comes back is `bioprism-onco`'s own, carried unaltered through
//! [`crate::BioethicsError::Onco`]. This crate does not decide what counts as clinical use and has
//! no vocabulary for it.
//!
//! # What 36.10 names and never specifies
//!
//! * **"restricted capability tiers"** — the tiers are never enumerated, anywhere in §36. There is
//!   no tier enum here. `bioprism_sdk::sandbox::IsolationClass` is the workspace's only ladder of
//!   this shape and it orders *requests*, not delivered properties, so it is not a capability tier
//!   either.
//! * **"public MVP"** — never defined, so the control is implemented as an unconditional absence
//!   rather than as a flag that some future configuration flips.
//! * **"human approval"** and **"institutional safety review"** — no approver role, no review
//!   criterion, no quorum and no expiry. [`Authorisation`] records two names and checks only that
//!   they are present and attributable, which is the same weak criterion `bioprism-stewardship`
//!   applies to a signed attestation and is weak for the same reason.
//! * **"sandbox simulation"** — a runtime control. Declared in the register, owned by nothing here.
//! * **"full audit"** — no schema, no retention, no clock. This crate emits no log.
//!
//! # Not implemented
//!
//! No scheduling, no protocol language, no instrument model, no reagent inventory, no specimen
//! ledger, no cost model, no clock and no identity system. An [`Authorisation`] is a transcription
//! of something that happened outside this process; nothing authenticates the names in it.

use crate::error::BioethicsError;
use bioprism_onco::{OutputUse, ResearchBoundary};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Whether an action changes the world or only a computation.
///
/// Two variants, and the mapping from [`ActionKind`] is total and `const`. There is no
/// configuration that reclassifies a kind, for the reason `bioprism_onco::OutputUse` gives about
/// clinical use: a boundary whose definition of "physical" is adjustable is not a boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    /// Consumes compute and nothing else.
    InSilico,
    /// Consumes material, drives an instrument, or changes the state of a living system.
    OnTheWorld,
}

impl Effect {
    pub const fn as_str(self) -> &'static str {
        match self {
            Effect::InSilico => "in_silico",
            Effect::OnTheWorld => "on_the_world",
        }
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a planned step does.
///
/// The six physical kinds are 36.10's Scope list, transcribed. The three in-silico kinds are the
/// three things 36.10's purpose sentence says initial releases *may* do — "simulation, analysis,
/// and reviewed protocol planning" — so both halves come from the module rather than from this
/// crate. Nothing else is enumerated: §36 gives no taxonomy of laboratory operations and this
/// crate does not invent one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Calling a remote laboratory's execution interface.
    RemoteLabApi,
    /// Driving an automated liquid handler.
    LiquidHandler,
    /// Consuming specimen material, which is finite and not recoverable.
    SampleConsumption,
    /// Cell culture or animal work.
    CellCultureOrAnimalWork,
    /// Handling chemicals.
    ChemicalHandling,
    /// Controlling an instrument.
    InstrumentControl,
    /// Running a simulation.
    Simulation,
    /// Analysing data already collected.
    Analysis,
    /// Writing a protocol for a human to review.
    ProtocolPlanning,
}

impl ActionKind {
    pub const ALL: [ActionKind; 9] = [
        ActionKind::RemoteLabApi,
        ActionKind::LiquidHandler,
        ActionKind::SampleConsumption,
        ActionKind::CellCultureOrAnimalWork,
        ActionKind::ChemicalHandling,
        ActionKind::InstrumentControl,
        ActionKind::Simulation,
        ActionKind::Analysis,
        ActionKind::ProtocolPlanning,
    ];

    /// Total, `const`, and not configurable.
    pub const fn effect(self) -> Effect {
        match self {
            ActionKind::RemoteLabApi
            | ActionKind::LiquidHandler
            | ActionKind::SampleConsumption
            | ActionKind::CellCultureOrAnimalWork
            | ActionKind::ChemicalHandling
            | ActionKind::InstrumentControl => Effect::OnTheWorld,
            ActionKind::Simulation | ActionKind::Analysis | ActionKind::ProtocolPlanning => {
                Effect::InSilico
            }
        }
    }

    pub const fn is_physical(self) -> bool {
        matches!(self.effect(), Effect::OnTheWorld)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            ActionKind::RemoteLabApi => "remote_lab_api",
            ActionKind::LiquidHandler => "liquid_handler",
            ActionKind::SampleConsumption => "sample_consumption",
            ActionKind::CellCultureOrAnimalWork => "cell_culture_or_animal_work",
            ActionKind::ChemicalHandling => "chemical_handling",
            ActionKind::InstrumentControl => "instrument_control",
            ActionKind::Simulation => "simulation",
            ActionKind::Analysis => "analysis",
            ActionKind::ProtocolPlanning => "protocol_planning",
        }
    }

    pub const fn describe(self) -> &'static str {
        match self {
            ActionKind::RemoteLabApi => "calling a remote laboratory execution interface",
            ActionKind::LiquidHandler => "driving an automated liquid handler",
            ActionKind::SampleConsumption => "consuming specimen material",
            ActionKind::CellCultureOrAnimalWork => "cell culture or animal work",
            ActionKind::ChemicalHandling => "handling chemicals",
            ActionKind::InstrumentControl => "controlling an instrument",
            ActionKind::Simulation => "running a simulation",
            ActionKind::Analysis => "analysing data already collected",
            ActionKind::ProtocolPlanning => "writing a protocol for a human to review",
        }
    }
}

impl fmt::Display for ActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One step of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedStep {
    pub kind: ActionKind,
    /// What the step is, in the planner's words. Recorded, never parsed.
    pub label: String,
}

impl PlannedStep {
    pub fn new(kind: ActionKind, label: impl Into<String>) -> Self {
        PlannedStep {
            kind,
            label: label.into(),
        }
    }

    pub const fn effect(&self) -> Effect {
        self.kind.effect()
    }
}

/// A plan and the use its results are intended for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionPlan {
    pub subject: String,
    pub steps: Vec<PlannedStep>,
    /// What the results will be used for, in `bioprism-onco`'s vocabulary. This crate holds no
    /// competing enumeration of uses.
    pub declared_use: OutputUse,
}

impl ActionPlan {
    pub fn new(subject: impl Into<String>, declared_use: OutputUse) -> Self {
        ActionPlan {
            subject: subject.into(),
            steps: Vec::new(),
            declared_use,
        }
    }

    pub fn with_step(mut self, step: PlannedStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Splits the plan into what may run and what may not, after the research boundary agrees.
    ///
    /// The boundary check comes first and refuses the whole plan, including its harmless
    /// simulation steps: when the *purpose* is individual clinical direction, the simulation is
    /// not a safe portion, it is the first half of the prohibited thing.
    pub fn partition(
        &self,
        boundary: &ResearchBoundary,
    ) -> Result<PlanDisposition, BioethicsError> {
        boundary.check(self.declared_use)?;

        let (physical, in_silico): (Vec<PlannedStep>, Vec<PlannedStep>) = self
            .steps
            .iter()
            .cloned()
            .partition(|step| step.kind.is_physical());

        Ok(PlanDisposition {
            subject: self.subject.clone(),
            in_silico,
            physical,
        })
    }
}

/// The split.
///
/// Both lists are always present, so a caller cannot read "no physical steps" out of a value that
/// simply never recorded any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanDisposition {
    subject: String,
    in_silico: Vec<PlannedStep>,
    physical: Vec<PlannedStep>,
}

impl PlanDisposition {
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// The steps that may run here. Empty is a legitimate answer and means nothing was salvageable.
    pub fn in_silico_steps(&self) -> &[PlannedStep] {
        &self.in_silico
    }

    /// The steps that may not run here, ever, under any configuration.
    pub fn physical_steps(&self) -> &[PlannedStep] {
        &self.physical
    }

    pub fn requires_physical_authorisation(&self) -> bool {
        !self.physical.is_empty()
    }
}

/// A transcription of the two human acts 36.10 requires before a physical step leaves this
/// workspace as a referral.
///
/// Nothing here is authenticated. The names are strings a caller supplied, exactly as
/// `bioprism-stewardship`'s site attestation is a record that names digests rather than a
/// signature over them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Authorisation {
    /// Who approved the physical steps.
    pub human_approver: Option<String>,
    /// Which institutional body reviewed them for safety.
    pub institutional_safety_review_body: Option<String>,
}

impl Authorisation {
    pub fn new() -> Self {
        Authorisation::default()
    }

    pub fn approved_by(mut self, approver: impl Into<String>) -> Self {
        self.human_approver = Some(approver.into());
        self
    }

    pub fn safety_reviewed_by(mut self, body: impl Into<String>) -> Self {
        self.institutional_safety_review_body = Some(body.into());
        self
    }

    /// Which of 36.10's two required human acts are missing, in blueprint order.
    pub fn missing(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.human_approver.is_none() {
            missing.push("human approval");
        }
        if self.institutional_safety_review_body.is_none() {
            missing.push("institutional safety review");
        }
        missing
    }
}

/// A plan's physical steps, addressed to a process outside this workspace.
///
/// # What this type is not
///
/// It is not a record that anything happened. It is not an instruction to a device. Nothing in
/// this crate or this workspace consumes one. There is no `PerformedAction`, no `ActionResult` and
/// no `execute`, which is the whole of 36.10's *no physical execution* control as this workspace
/// can implement it.
///
/// # Why there is no `Deserialize`
///
/// Decoding one would mint a referral without running [`refer`], and the only property a referral
/// has is that [`refer`] ran. A process receiving one over a wire decodes the plan and the
/// authorisation and re-refers them under its own check, which is what an independent boundary is
/// for. `bioprism_onco::ResearchOutput` declines `Deserialize` for the same reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhysicalReferral {
    subject: String,
    steps: Vec<PlannedStep>,
    human_approver: String,
    institutional_safety_review_body: String,
}

impl PhysicalReferral {
    /// The fixed statement that travels with every referral. Not configurable.
    pub const STATEMENT: &'static str =
        "Referral only. This workspace does not execute physical actions, and possessing this \
         record is not evidence that any action was taken.";

    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub fn steps(&self) -> &[PlannedStep] {
        &self.steps
    }

    pub fn human_approver(&self) -> &str {
        &self.human_approver
    }

    pub fn institutional_safety_review_body(&self) -> &str {
        &self.institutional_safety_review_body
    }
}

/// The only constructor for a [`PhysicalReferral`].
///
/// Refuses a disposition with no physical steps, because a referral for nothing is a record whose
/// only possible use is to imply that something was authorised.
pub fn refer(
    disposition: &PlanDisposition,
    authorisation: &Authorisation,
) -> Result<PhysicalReferral, BioethicsError> {
    if !disposition.requires_physical_authorisation() {
        return Err(BioethicsError::PhysicalStepUnauthorised {
            plan: disposition.subject.clone(),
            physical_steps: 0,
            missing: "a physical step to refer".to_string(),
        });
    }

    let missing = authorisation.missing();
    if !missing.is_empty() {
        return Err(BioethicsError::PhysicalStepUnauthorised {
            plan: disposition.subject.clone(),
            physical_steps: disposition.physical.len(),
            missing: missing.join(" and "),
        });
    }

    let human_approver = authorisation.human_approver.clone().unwrap_or_default();
    let body = authorisation
        .institutional_safety_review_body
        .clone()
        .unwrap_or_default();

    if human_approver.trim().is_empty() {
        return Err(BioethicsError::UnattributedAuthorisation {
            plan: disposition.subject.clone(),
            field: "human approver".to_string(),
        });
    }
    if body.trim().is_empty() {
        return Err(BioethicsError::UnattributedAuthorisation {
            plan: disposition.subject.clone(),
            field: "institutional safety review body".to_string(),
        });
    }

    Ok(PhysicalReferral {
        subject: disposition.subject.clone(),
        steps: disposition.physical.clone(),
        human_approver,
        institutional_safety_review_body: body,
    })
}
