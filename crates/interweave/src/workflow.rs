//! The six reference workflows, their effect envelopes, and the deliverables each still owes.
//!
//! Blueprint 23.33.
//!
//! # What is implemented here and what is not
//!
//! 23.33 is the module in this crate's nine where the split between specification and content runs
//! straight through the middle, so it is worth being exact about which side each half falls on.
//!
//! **Content, not implemented.** The six workflows themselves — the WeaveLang programs, the
//! fixtures, the tutorials, the architecture diagrams — are authored artefacts. Writing them
//! requires a world, participants, adapters and an execution substrate, none of which this crate
//! has. Not one line of WeaveLang for any of the six exists in this workspace.
//!
//! **Specification, implemented.** 23.33's closing section is a nine-item completeness requirement
//! over every reference workflow, and that is a predicate. [`ReferenceWorkflow::owed`] evaluates it,
//! and on the registry this crate ships — [`catalogue`] — the honest answer is that all six
//! workflows owe all nine deliverables. That result is the point rather than an embarrassment: it
//! is the difference between a roadmap that says "reference workflows: planned" and one that names
//! fifty-four missing artefacts.
//!
//! The role sets and effect envelopes are also specification, because 23.33 states them and they
//! constrain each other. Workflow 3 declares "read-only access to approved data, sandboxed
//! analysis, no patient-level external publication"; [`ReferenceWorkflow::envelope_violations`]
//! checks each role's declared effects against that envelope using
//! `bioprism_fabric::effect::EffectSet`, so a role that acquires `external.publish` is a detectable
//! contradiction rather than a documentation drift.
//!
//! # The adapter clause is the one deliverable with teeth
//!
//! "At least two participant adapters" is checkable beyond counting, because 23.24 grades adapters.
//! [`ReferenceWorkflow::adapter_deliverable`] requires two adapters *and* a stated grade for each,
//! since two G0 opaque-transport bridges do not demonstrate that a workflow is portable — they
//! demonstrate that bytes move.

use crate::adapter::{AdapterProfile, Grade};
use bioprism_fabric::effect::{Effect, EffectKind, EffectSet};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 23.33's six reference workflows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowId {
    ReliableSoftwareRepair,
    ScientificClaimReproduction,
    BiomedicalResearchDataAudit,
    IncidentResponse,
    EvidenceGroundedPolicyComparison,
    DatasetTransformationMolecule,
}

impl WorkflowId {
    pub const ALL: [WorkflowId; 6] = [
        WorkflowId::ReliableSoftwareRepair,
        WorkflowId::ScientificClaimReproduction,
        WorkflowId::BiomedicalResearchDataAudit,
        WorkflowId::IncidentResponse,
        WorkflowId::EvidenceGroundedPolicyComparison,
        WorkflowId::DatasetTransformationMolecule,
    ];

    /// 23.33's numbering.
    pub fn number(self) -> u8 {
        match self {
            WorkflowId::ReliableSoftwareRepair => 1,
            WorkflowId::ScientificClaimReproduction => 2,
            WorkflowId::BiomedicalResearchDataAudit => 3,
            WorkflowId::IncidentResponse => 4,
            WorkflowId::EvidenceGroundedPolicyComparison => 5,
            WorkflowId::DatasetTransformationMolecule => 6,
        }
    }

    /// The roles 23.33 lists, verbatim and in order.
    pub fn roles(self) -> &'static [&'static str] {
        match self {
            WorkflowId::ReliableSoftwareRepair => &[
                "lead",
                "repository investigator",
                "patcher",
                "test runner",
                "skeptic",
                "release gate",
            ],
            WorkflowId::ScientificClaimReproduction => &[
                "claim extractor",
                "source retriever",
                "dataset auditor",
                "execution runner",
                "statistician",
                "skeptical reviewer",
                "report compiler",
            ],
            WorkflowId::BiomedicalResearchDataAudit => &[
                "format reader",
                "metadata validator",
                "privacy monitor",
                "cohort auditor",
                "modality specialist",
                "reproducibility verifier",
            ],
            WorkflowId::IncidentResponse => &[
                "incident commander",
                "telemetry investigator",
                "change proposer",
                "rollback executor",
                "communications reviewer",
                "human approver",
            ],
            WorkflowId::EvidenceGroundedPolicyComparison => &[
                "source collectors with independent search strategies",
                "claim normalizer",
                "methods reviewer",
                "counterevidence agent",
                "synthesis agent",
                "human editor",
            ],
            WorkflowId::DatasetTransformationMolecule => &[
                "schema mapper",
                "unit and ontology validator",
                "transformer",
                "quality checker",
                "lineage attester",
            ],
        }
    }

    /// The distinctive behaviours 23.33 attributes to this workflow.
    ///
    /// Recorded as text because they are not decidable from a declaration: "competing root-cause
    /// hypotheses" is a property of a run, and this crate has no runs.
    pub fn distinctive_behaviours(self) -> &'static [&'static str] {
        match self {
            WorkflowId::ReliableSoftwareRepair => &[
                "competing root-cause hypotheses",
                "information-value context expansion",
                "continuation handoff from investigator to patcher",
                "parallel patch and counterexample branches",
                "deterministic test oracle",
                "no merge authority without human grant",
            ],
            WorkflowId::ScientificClaimReproduction => &[
                "separate manuscript claims from evidence",
                "preserve contradictory results",
                "run code in sandbox",
                "challenge cohort or statistical mismatch",
                "render one evidence state for researcher and lay audiences",
            ],
            WorkflowId::BiomedicalResearchDataAudit => &[
                "research-only, not clinical decision support",
                "read-only access to approved data",
                "sandboxed analysis",
                "no patient-level external publication",
                "explicit de-identification policy",
            ],
            WorkflowId::IncidentResponse => &[
                "strict decision rights",
                "time-bounded commitments",
                "sagas and compensation",
                "revocation and participant substitution",
                "separate internal and external communication authority",
            ],
            WorkflowId::EvidenceGroundedPolicyComparison => &[
                "source independence tracking",
                "conflict preservation",
                "no majority-as-truth",
                "explicit value judgments",
                "purpose-specific context capsules",
            ],
            WorkflowId::DatasetTransformationMolecule => &[
                "WIT or typed component interfaces",
                "deterministic transforms",
                "semantic-loss budget",
                "content-addressed outputs",
                "repeatable conformance tests",
            ],
        }
    }

    /// Effect kinds this workflow forbids outright, from its own text.
    ///
    /// Only workflow 3 states a prohibition — 23.33 gives it an explicit `Effects` section and
    /// gives no other workflow one. Inventing envelopes for the other five would be putting words
    /// in the blueprint's mouth, so they forbid nothing and [`ReferenceWorkflow::envelope_violations`]
    /// finds nothing for them. That is a limit of the source, recorded rather than papered over.
    pub fn forbidden_effects(self) -> &'static [EffectKind] {
        match self {
            WorkflowId::BiomedicalResearchDataAudit => &[
                EffectKind::ExternalPublish,
                EffectKind::FilesystemWrite,
                EffectKind::ClinicalOutput,
                EffectKind::NetworkWrite,
            ],
            _ => &[],
        }
    }
}

/// 23.33's nine cross-workflow deliverables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Deliverable {
    WeaveLangSource,
    CompiledWeaveIr,
    LocalDeterministicFixtures,
    TwoParticipantAdapters,
    FailureInjections,
    PrismDecisionCells,
    ResultBundle,
    SecurityAndPrivacyNotes,
    TutorialAndArchitectureDiagram,
}

impl Deliverable {
    pub const ALL: [Deliverable; 9] = [
        Deliverable::WeaveLangSource,
        Deliverable::CompiledWeaveIr,
        Deliverable::LocalDeterministicFixtures,
        Deliverable::TwoParticipantAdapters,
        Deliverable::FailureInjections,
        Deliverable::PrismDecisionCells,
        Deliverable::ResultBundle,
        Deliverable::SecurityAndPrivacyNotes,
        Deliverable::TutorialAndArchitectureDiagram,
    ];
}

/// One role in a workflow, with the effects it declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRole {
    pub name: String,
    pub effects: EffectSet,
}

impl WorkflowRole {
    pub fn new(name: impl Into<String>) -> Self {
        WorkflowRole {
            name: name.into(),
            effects: EffectSet::new(),
        }
    }

    pub fn performing(mut self, effect: Effect) -> Self {
        self.effects = std::mem::take(&mut self.effects).with(effect);
        self
    }
}

/// A role effect that contradicts its workflow's stated envelope.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, thiserror::Error)]
#[error("{workflow:?} forbids {kind} and role {role} declares it")]
pub struct EnvelopeViolation {
    pub workflow: WorkflowId,
    pub role: String,
    pub kind: EffectKind,
}

/// A reference workflow as this workspace can currently describe it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceWorkflow {
    pub id: WorkflowId,
    pub roles: Vec<WorkflowRole>,
    /// Deliverables the workspace actually has for this workflow.
    pub present: BTreeSet<Deliverable>,
    /// Adapters offered for [`Deliverable::TwoParticipantAdapters`], with the grade each was
    /// assessed at.
    pub adapters: Vec<(AdapterProfile, Grade)>,
}

impl ReferenceWorkflow {
    /// A workflow with 23.33's role list, no declared effects, and nothing delivered.
    pub fn from_blueprint(id: WorkflowId) -> Self {
        ReferenceWorkflow {
            id,
            roles: id.roles().iter().map(|r| WorkflowRole::new(*r)).collect(),
            present: BTreeSet::new(),
            adapters: Vec::new(),
        }
    }

    pub fn with_role(mut self, role: WorkflowRole) -> Self {
        if let Some(slot) = self.roles.iter_mut().find(|r| r.name == role.name) {
            *slot = role;
        } else {
            self.roles.push(role);
        }
        self
    }

    pub fn delivering(mut self, deliverable: Deliverable) -> Self {
        self.present.insert(deliverable);
        self
    }

    pub fn with_adapter(mut self, adapter: AdapterProfile) -> Self {
        let grade = adapter.grade();
        self.adapters.push((adapter, grade));
        self
    }

    /// Whether the two-adapter deliverable is genuinely met.
    ///
    /// Two adapters, and both above [`Grade::G0`]. 23.33 asks for "at least two participant
    /// adapters" to prove portability, and two opaque-transport bridges prove that bytes move
    /// rather than that the workflow's semantics survive a change of participant.
    pub fn adapter_deliverable(&self) -> bool {
        self.adapters.len() >= 2 && self.adapters.iter().all(|(_, grade)| *grade > Grade::G0)
    }

    /// The deliverables this workflow still owes.
    pub fn owed(&self) -> BTreeSet<Deliverable> {
        Deliverable::ALL
            .into_iter()
            .filter(|deliverable| match deliverable {
                Deliverable::TwoParticipantAdapters => !self.adapter_deliverable(),
                other => !self.present.contains(other),
            })
            .collect()
    }

    pub fn complete(&self) -> bool {
        self.owed().is_empty()
    }

    /// Role effects that contradict the workflow's own stated envelope.
    pub fn envelope_violations(&self) -> Vec<EnvelopeViolation> {
        let forbidden: BTreeSet<EffectKind> = self.id.forbidden_effects().iter().copied().collect();
        let mut violations: Vec<EnvelopeViolation> = self
            .roles
            .iter()
            .flat_map(|role| {
                role.effects
                    .iter()
                    .filter(|effect| forbidden.contains(&effect.kind))
                    .map(|effect| EnvelopeViolation {
                        workflow: self.id,
                        role: role.name.clone(),
                        kind: effect.kind,
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        violations.sort();
        violations
    }
}

/// The six workflows as the blueprint describes them, with nothing delivered.
///
/// This is the honest state of 23.33 in this workspace. Every workflow in this catalogue owes
/// every one of the nine deliverables, and the arithmetic is asserted in the tests so that the
/// number moves only when an artefact actually lands.
pub fn catalogue() -> Vec<ReferenceWorkflow> {
    WorkflowId::ALL
        .into_iter()
        .map(ReferenceWorkflow::from_blueprint)
        .collect()
}

/// How many deliverables the whole reference-workflow programme still owes.
pub fn outstanding_deliverables(workflows: &[ReferenceWorkflow]) -> BTreeMap<WorkflowId, usize> {
    workflows
        .iter()
        .map(|workflow| (workflow.id, workflow.owed().len()))
        .collect()
}

/// 23.33's shared-state description for workflow 2, recorded as text.
///
/// "Evidence lattice with claim obligations, artifact lineage, cohort definitions, code/data
/// versions, and reproduction results." The lattice itself belongs to `bioprism-weave`'s epistemic
/// ledger (23.08) and is not rebuilt here.
pub const CLAIM_REPRODUCTION_SHARED_STATE: [&str; 5] = [
    "claim obligations",
    "artifact lineage",
    "cohort definitions",
    "code/data versions",
    "reproduction results",
];

/// 23.33's evaluation boundaries for workflow 1, recorded as text.
///
/// These are the six decision points a benchmark would score. Turning them into instances needs a
/// repository, a test runner and a model, so they are named and not generated.
pub const SOFTWARE_REPAIR_EVALUATION_BOUNDARIES: [&str; 6] = [
    "next file selection",
    "patcher binding",
    "branch creation",
    "challenge timing",
    "retry versus rollback",
    "final join",
];
