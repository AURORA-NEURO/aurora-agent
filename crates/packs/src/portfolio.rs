//! The pack portfolio (blueprint 15.00, 15.01–15.25, 29.01–29.21).
//!
//! Every numbered module in section 15 and section 29 is encoded below as a typed definition: the
//! construct it claims to measure, the capability families it evidences, the domains its parents
//! live in, the decision families it mines, and the oracle tiers that could decide an instance.
//!
//! The `measures` field is the one to read. It is taken from each module's own purpose or
//! objective rather than from its title, because titles converge on generic nouns and constructs
//! do not: 15.02 is not "tools", it is whether an agent distinguishes *tool completion from task
//! completion*; 15.06 is not "long horizon", it is whether obligations survive distractors and
//! whether the agent stops. A capability map built from titles would put 15.05 and 15.23 in the
//! same cell, and they measure opposite things — one is an agent recovering, the other is the
//! platform reconstructing why an agent did not.
//!
//! Counts. Section 15 contains twenty-five numbered pack modules, not twenty-six; `00` is the
//! portfolio specification itself and defines no pack. Section 29 contains twenty-one, with `00`
//! again the taxonomy rather than a pack. Forty-six definitions therefore, and the tests assert
//! the count against the module numbering rather than against a literal.
//!
//! Oracle tiers are the module's declared oracle *strategy*, mapped onto [`OracleTier`]. This is
//! an interpretation: the blueprint writes "expert review for open-ended scientific evidence
//! choices" and this crate records `ExpertReview`, but it writes "information gain computed from a
//! known hypothesis/world model" and that maps to `Executable`, because computing it requires
//! running the world model. Where a module lists several, all are recorded and the strongest is
//! derived; a pack is judged by its best available oracle, not its average.

use crate::error::PackError;
use crate::ir::{PackId, PackManifest, PackVersion, SchemaRange};
use crate::taxonomy::{
    AgentCapability, BioCapability, CapabilityFamily, Domain, OracleTier, PackAxis, ReleaseWave,
};

const fn agent(capability: AgentCapability) -> CapabilityFamily {
    CapabilityFamily::Agent(capability)
}

const fn bio(capability: BioCapability) -> CapabilityFamily {
    CapabilityFamily::Biology(capability)
}

/// A pack as the portfolio declares it, before any parents or instances exist.
///
/// Static because these are blueprint facts rather than runtime state. A [`PackDefinition`]
/// becomes a [`PackManifest`] once someone commits to owning and licensing it, and a
/// [`crate::ir::PackIr`] once parents have been authored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackDefinition {
    pub id: &'static str,
    pub title: &'static str,
    /// The blueprint module realized, e.g. `15.05`.
    pub blueprint_module: &'static str,
    pub axis: PackAxis,
    /// The construct, in one sentence, from the module's own purpose or objective.
    pub measures: &'static str,
    pub capabilities: &'static [CapabilityFamily],
    pub domains: &'static [Domain],
    /// The module's microbenchmark families or task ladder.
    pub decision_families: &'static [&'static str],
    pub oracles: &'static [OracleTier],
    pub release_wave: ReleaseWave,
}

impl PackDefinition {
    /// The best judgement available for this pack's instances.
    pub fn strongest_oracle(&self) -> Option<OracleTier> {
        self.oracles.iter().copied().max_by_key(|t| t.strength())
    }

    /// Whether any disagreement about an instance could be settled by re-running.
    pub fn has_grounded_oracle(&self) -> bool {
        self.oracles.iter().any(|t| t.is_execution_grounded())
    }

    pub fn covers(&self, family: CapabilityFamily) -> bool {
        self.capabilities.contains(&family)
    }

    /// The (axis, capabilities, domains) tuple used to screen for packs that differ only in
    /// wording. 15.00: "Avoid packs differing only by prompt wording."
    pub fn capability_signature(&self) -> String {
        let mut capabilities: Vec<&str> = self.capabilities.iter().map(|c| c.code()).collect();
        capabilities.sort_unstable();
        let mut domains: Vec<&str> = self.domains.iter().map(|d| d.label()).collect();
        domains.sort_unstable();
        format!(
            "{:?}|{}|{}",
            self.axis,
            capabilities.join(","),
            domains.join(",")
        )
    }

    /// Promote a definition to a manifest by supplying the things a blueprint cannot: who owns it
    /// and under what licence.
    pub fn to_manifest(
        &self,
        version: PackVersion,
        schema_range: SchemaRange,
        owners: Vec<String>,
        license: impl Into<String>,
    ) -> Result<PackManifest, PackError> {
        Ok(PackManifest {
            id: PackId::parse(self.id)?,
            version,
            schema_range,
            title: self.title.to_string(),
            measures: self.measures.to_string(),
            blueprint_module: self.blueprint_module.to_string(),
            axis: self.axis,
            capabilities: self.capabilities.to_vec(),
            domains: self.domains.to_vec(),
            owners,
            license: license.into(),
            dependencies: Vec::new(),
        })
    }
}

/// Every pack in the portfolio, section 15 first then section 29.
pub fn all() -> &'static [PackDefinition] {
    PACKS
}

/// The twenty-five agent and platform packs of blueprint section 15.
pub fn section_15() -> Vec<&'static PackDefinition> {
    PACKS
        .iter()
        .filter(|p| p.blueprint_module.starts_with("15."))
        .collect()
}

/// The twenty-one biological packs of blueprint section 29.
pub fn section_29() -> Vec<&'static PackDefinition> {
    PACKS
        .iter()
        .filter(|p| p.blueprint_module.starts_with("29."))
        .collect()
}

pub fn find(id: &str) -> Result<&'static PackDefinition, PackError> {
    PACKS
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| PackError::UnknownPack(id.to_string()))
}

pub fn by_axis(axis: PackAxis) -> Vec<&'static PackDefinition> {
    PACKS.iter().filter(|p| p.axis == axis).collect()
}

/// Packs 15.00 places in the initial release order, in that order.
///
/// Ties within a wave are broken by blueprint module number, which is arbitrary but stable; the
/// blueprint pairs packs inside a wave ("coding/terminal vertical slice") without ordering them.
pub fn release_order() -> Vec<&'static PackDefinition> {
    let mut sequenced: Vec<&'static PackDefinition> = PACKS
        .iter()
        .filter(|p| p.release_wave.is_sequenced())
        .collect();
    sequenced.sort_by_key(|p| (p.release_wave, p.blueprint_module));
    sequenced
}

/// Packs the blueprint never places in the release order.
pub fn unsequenced() -> Vec<&'static PackDefinition> {
    PACKS
        .iter()
        .filter(|p| !p.release_wave.is_sequenced())
        .collect()
}

/// Packs whose (axis, capabilities, domains) signature is shared with another pack.
///
/// A screen, not a verdict. Two packs can legitimately share a signature and differ in their
/// decision families — 29.05 and 29.06 both claim B5 — so this returns candidates for the
/// redundancy review that 15.00 asks for, grouped by signature.
pub fn duplicate_signatures() -> Vec<(String, Vec<&'static str>)> {
    let mut groups: Vec<(String, Vec<&'static str>)> = Vec::new();
    for pack in PACKS {
        let signature = pack.capability_signature();
        match groups.iter_mut().find(|(s, _)| *s == signature) {
            Some((_, ids)) => ids.push(pack.id),
            None => groups.push((signature, vec![pack.id])),
        }
    }
    groups.retain(|(_, ids)| ids.len() > 1);
    groups
}

const PACKS: &[PackDefinition] = &[
    PackDefinition {
        id: "prism.context-acquisition",
        title: "Context Acquisition and Evidence Value",
        blueprint_module: "15.01",
        axis: PackAxis::Mechanism,
        measures: "Whether an agent seeks the smallest, highest-value evidence needed to \
                   distinguish plausible actions, and stops before retrieval becomes wasteful or \
                   unsafe.",
        capabilities: &[
            agent(AgentCapability::EvidenceAcquisition),
            agent(AgentCapability::HypothesisAndPlanning),
        ],
        domains: &[
            Domain::Coding,
            Domain::Science,
            Domain::Operations,
            Domain::Enterprise,
        ],
        decision_families: &[
            "choose which artifact to inspect next",
            "choose a query or test that separates two hypotheses",
            "decide between reading more context, acting, asking a user, or abstaining",
            "reject a semantically similar but stale or unauthoritative source",
            "detect when full-context ingestion introduces distractor failure",
            "compress context while preserving decision-relevant evidence",
            "estimate whether a new source justifies its privacy or latency cost",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::PolicyVeto,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Wave(1),
    },
    PackDefinition {
        id: "prism.tool-selection",
        title: "Tool Selection, Arguments and Outcomes",
        blueprint_module: "15.02",
        axis: PackAxis::Mechanism,
        measures: "Whether agents choose tools by capability rather than name familiarity, \
                   construct valid and safe calls, interpret results correctly, and distinguish \
                   tool completion from task completion.",
        capabilities: &[
            agent(AgentCapability::ToolUse),
            agent(AgentCapability::VerificationAndRecovery),
        ],
        domains: &[
            Domain::Coding,
            Domain::Data,
            Domain::Science,
            Domain::Enterprise,
        ],
        decision_families: &[
            "select a tool versus answering directly",
            "choose among overlapping tools",
            "populate arguments with correct identifiers, units, scope and time range",
            "recognize that a tool returned stale, partial or schema-shifted data",
            "avoid repeating a non-idempotent action after a timeout",
            "check state after a nominally successful call",
            "choose retry, fallback, rollback, escalation or abstention",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::PolicyVeto,
        ],
        release_wave: ReleaseWave::Wave(2),
    },
    PackDefinition {
        id: "prism.memory-lifecycle",
        title: "Memory Lifecycle",
        blueprint_module: "15.03",
        axis: PackAxis::Mechanism,
        measures: "What an agent writes, retrieves, updates, expires, scopes and refuses to store \
                   across tasks and users.",
        capabilities: &[
            agent(AgentCapability::Memory),
            agent(AgentCapability::Privacy),
        ],
        domains: &[
            Domain::Coding,
            Domain::Science,
            Domain::Enterprise,
            Domain::MultiAgent,
        ],
        decision_families: &[
            "write versus do not write",
            "choose memory scope and retention",
            "retrieve one relevant item among distractors",
            "update superseded facts while preserving history",
            "detect contradiction and request validation",
            "ignore malicious instructions stored as data",
            "avoid leaking another task's or user's memory",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::PolicyVeto,
            OracleTier::Rubric,
        ],
        release_wave: ReleaseWave::Wave(5),
    },
    PackDefinition {
        id: "prism.hypothesis-planning",
        title: "Hypothesis, Planning and Commitment",
        blueprint_module: "15.04",
        axis: PackAxis::Mechanism,
        measures: "Whether agents generate structurally diverse alternatives, expose assumptions, \
                   choose plans supported by evidence, and revise before costly commitment.",
        capabilities: &[agent(AgentCapability::HypothesisAndPlanning)],
        domains: &[
            Domain::Coding,
            Domain::Science,
            Domain::Operations,
            Domain::Data,
        ],
        decision_families: &[
            "generate alternatives with non-overlapping assumptions",
            "select the next discriminating experiment",
            "choose a plan under budget, permission and dependency constraints",
            "recognize that plans are observationally equivalent with current evidence",
            "revise after a failed test",
            "stop unproductive planning and execute",
            "backtrack to the earliest invalid assumption",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Wave(5),
    },
    PackDefinition {
        id: "prism.verification-recovery",
        title: "Verification, Recovery and Backtracking",
        blueprint_module: "15.05",
        axis: PackAxis::Mechanism,
        measures:
            "Whether agents verify claims that could be falsified, detect partial and silent \
                   failure, localize the earliest invalid assumption, and recover without \
                   compounding harm.",
        capabilities: &[agent(AgentCapability::VerificationAndRecovery)],
        domains: &[
            Domain::Coding,
            Domain::Data,
            Domain::Science,
            Domain::Operations,
        ],
        decision_families: &[
            "select the next verifier",
            "interpret a failed or flaky check",
            "detect a false-positive success signal",
            "choose retry versus a different strategy",
            "identify the earliest invalid assumption",
            "roll back a reversible change",
            "escalate when evidence or permissions are insufficient",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Wave(3),
    },
    PackDefinition {
        id: "prism.long-horizon-state",
        title: "Long-Horizon State and Termination",
        blueprint_module: "15.06",
        axis: PackAxis::Mechanism,
        measures: "Whether agents maintain coherent state over many steps, preserve constraints \
                   and unresolved obligations, recognize progress and dead ends, and stop \
                   correctly.",
        capabilities: &[
            agent(AgentCapability::LongHorizonState),
            agent(AgentCapability::Memory),
        ],
        domains: &[
            Domain::Coding,
            Domain::Browser,
            Domain::Science,
            Domain::Enterprise,
        ],
        decision_families: &[
            "recall an active constraint after many unrelated events",
            "resume from a checkpoint",
            "detect repeated or circular action",
            "choose the next unfinished subgoal",
            "recognize task completion despite noisy logs",
            "avoid premature finalization",
            "summarize state for another agent without dropping obligations",
        ],
        oracles: &[OracleTier::Deterministic, OracleTier::Rubric],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "prism.multi-agent-coordination",
        title: "Multi-Agent Coordination",
        blueprint_module: "15.07",
        axis: PackAxis::Mechanism,
        measures:
            "Delegation, shared state, communication, conflict resolution and privacy across \
                   multiple specialized or peer agents.",
        capabilities: &[
            agent(AgentCapability::Coordination),
            agent(AgentCapability::Privacy),
        ],
        domains: &[Domain::MultiAgent, Domain::Coding, Domain::Science],
        decision_families: &[
            "choose delegation versus direct work",
            "select recipient and information scope",
            "merge conflicting reports",
            "schedule dependent tasks",
            "prevent concurrent destructive actions",
            "escalate disagreement",
            "preserve private information across internal channels",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::PolicyVeto,
        ],
        release_wave: ReleaseWave::Wave(7),
    },
    PackDefinition {
        id: "prism.coding-repository-inference",
        title: "Coding and Repository Inference",
        blueprint_module: "15.08",
        axis: PackAxis::Domain,
        measures: "Repository evidence acquisition, semantic code understanding, minimal patch \
                   planning, execution and regression verification at decision level.",
        capabilities: &[
            agent(AgentCapability::EvidenceAcquisition),
            agent(AgentCapability::VerificationAndRecovery),
            agent(AgentCapability::EvaluationIntegrity),
        ],
        domains: &[Domain::Coding],
        decision_families: &[
            "choose the next file, test or log",
            "identify a plausible root cause",
            "select patch location and scope",
            "interpret a test failure",
            "detect idempotency, concurrency, state, type or configuration errors",
            "verify regression and hidden side effects",
            "decide that issue evidence is insufficient",
        ],
        oracles: &[OracleTier::Deterministic, OracleTier::Executable],
        release_wave: ReleaseWave::Wave(4),
    },
    PackDefinition {
        id: "prism.browser-and-computer-use",
        title: "Browser and Computer Use",
        blueprint_module: "15.09",
        axis: PackAxis::Domain,
        measures: "Stateful navigation, information extraction, verification before irreversible \
                   submission, recovery, and resistance to visual or textual injection.",
        capabilities: &[
            agent(AgentCapability::ToolUse),
            agent(AgentCapability::Safety),
            agent(AgentCapability::LongHorizonState),
            agent(AgentCapability::Privacy),
        ],
        domains: &[Domain::Browser, Domain::Enterprise],
        decision_families: &[
            "choose the next UI action",
            "identify the authoritative field or value",
            "detect stale or changed DOM state",
            "confirm before submission or a payment-like effect",
            "recover from navigation or session failure",
            "ignore injection embedded in page content",
            "avoid leaking private context in external forms or queries",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::PolicyVeto,
        ],
        release_wave: ReleaseWave::Wave(7),
    },
    PackDefinition {
        id: "prism.data-and-database-reasoning",
        title: "Data, Spreadsheet and Database Reasoning",
        blueprint_module: "15.10",
        axis: PackAxis::Domain,
        measures: "Schema and lineage discovery, formula reasoning, transformation choice, \
                   data-quality diagnosis, and safe database operations that preserve source data.",
        capabilities: &[
            agent(AgentCapability::EvidenceAcquisition),
            agent(AgentCapability::VerificationAndRecovery),
            agent(AgentCapability::Safety),
        ],
        domains: &[Domain::Data],
        decision_families: &[
            "choose which table, sheet or range to inspect",
            "infer join key and cardinality",
            "repair or explain a formula",
            "detect a unit or date mismatch",
            "choose a read query over a write action",
            "trace a reported value to source cells or rows",
            "validate cohort inclusion, exclusion and leakage",
        ],
        oracles: &[OracleTier::Deterministic, OracleTier::Executable],
        release_wave: ReleaseWave::Wave(6),
    },
    PackDefinition {
        id: "prism.scientific-reasoning",
        title: "Scientific Reasoning and Reproducibility",
        blueprint_module: "15.11",
        axis: PackAxis::Domain,
        measures:
            "Claim-to-evidence tracing, method selection, statistical verification, artifact \
                   reconciliation, and detection of unsupported conclusions.",
        capabilities: &[
            agent(AgentCapability::EvidenceAcquisition),
            agent(AgentCapability::VerificationAndRecovery),
            agent(AgentCapability::HypothesisAndPlanning),
        ],
        domains: &[Domain::Science],
        decision_families: &[
            "trace a claim to exact artifacts",
            "choose the missing evidence to inspect",
            "recompute a statistic or figure",
            "detect a mismatch among manuscript, code and data",
            "select a statistical test under its assumptions",
            "identify unsupported causal or generalization claims",
            "explain uncertainty to expert and lay audiences from one evidence state",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::Statistical,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Wave(6),
    },
    PackDefinition {
        id: "prism.biomedical-research-workflows",
        title: "Biomedical Research Workflows",
        blueprint_module: "15.12",
        axis: PackAxis::Domain,
        measures: "Biomedical evidence acquisition, data integration, ontology and cohort \
                   reasoning, and communicated uncertainty, without patient-specific clinical \
                   advice.",
        capabilities: &[
            agent(AgentCapability::EvidenceAcquisition),
            agent(AgentCapability::Safety),
            bio(BioCapability::DataIdentity),
            bio(BioCapability::CohortAndStudyDesign),
        ],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "map identifiers and versions",
            "choose the authoritative source or dataset",
            "detect population or assay mismatch",
            "construct or audit a cohort",
            "select an analysis and its negative controls",
            "trace a result to code and data",
            "recognize a question that requires clinical expertise or cannot be answered from \
             evidence",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::PolicyVeto,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Wave(8),
    },
    PackDefinition {
        id: "prism.neuro-oncology-research",
        title: "Neuroscience and Neuro-Oncology Research",
        blueprint_module: "15.13",
        axis: PackAxis::Domain,
        measures: "Multimodal provenance, imaging and cohort metadata reconciliation, and claim \
                   tracing in neuroscience and neuro-oncology research, without diagnosis.",
        capabilities: &[
            agent(AgentCapability::EvidenceAcquisition),
            bio(BioCapability::DataIdentity),
            bio(BioCapability::MultimodalTranslation),
            bio(BioCapability::ComputationalReproducibility),
        ],
        domains: &[Domain::Neuroscience],
        decision_families: &[
            "choose the relevant imaging metadata or preprocessing artifact",
            "detect subject, session or run mismatch",
            "trace a figure or model result to cohort and code",
            "audit inclusion, exclusion and longitudinal alignment",
            "identify a missing modality or provenance gap",
            "reconcile molecular and imaging identifiers",
            "state what evidence supports a research claim without diagnosing",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::PolicyVeto,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Wave(8),
    },
    PackDefinition {
        id: "prism.multimodal-artifact-understanding",
        title: "Multimodal Artifact Understanding",
        blueprint_module: "15.14",
        axis: PackAxis::Domain,
        measures:
            "Decisions requiring coordinated reading of text, tables, diagrams, screenshots, \
                   plots, audio and structured data, including recognizing evidence that is too \
                   ambiguous to read.",
        capabilities: &[
            agent(AgentCapability::EvidenceAcquisition),
            agent(AgentCapability::VerificationAndRecovery),
        ],
        domains: &[Domain::Multimodal],
        decision_families: &[
            "choose the modality or source to inspect",
            "extract and align a value across chart, table and text",
            "detect a figure-caption mismatch",
            "trace a plotted point to underlying data",
            "recognize unreadable or ambiguous visual evidence",
            "choose between OCR, a structured source and direct visual reasoning",
            "avoid inferring hidden numerical precision from images",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Statistical,
            OracleTier::Rubric,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "prism.safety-and-irreversible-actions",
        title: "Safety, Permissions and Irreversible Actions",
        blueprint_module: "15.15",
        axis: PackAxis::Mechanism,
        measures: "Whether agents identify effect and reversibility, respect explicit and \
                   contextual permission, use least privilege and confirm high-impact actions, \
                   while retaining task utility.",
        capabilities: &[agent(AgentCapability::Safety)],
        domains: &[
            Domain::Enterprise,
            Domain::Coding,
            Domain::Operations,
            Domain::Science,
        ],
        decision_families: &[
            "read versus write",
            "choose a sandbox or test target over production",
            "recognize non-idempotent retry risk",
            "request confirmation",
            "select a scoped credential or permission",
            "refuse an unauthorized transfer or disclosure",
            "recover after a denied action",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::PolicyVeto,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "prism.privacy-contextual-integrity",
        title: "Privacy and Contextual Integrity",
        blueprint_module: "15.16",
        axis: PackAxis::Mechanism,
        measures:
            "Whether agents use necessary context without transmitting sensitive information \
                   across unauthorized recipients, purposes, tools, memories or internal channels.",
        capabilities: &[
            agent(AgentCapability::Privacy),
            agent(AgentCapability::Memory),
            agent(AgentCapability::Coordination),
        ],
        domains: &[Domain::Enterprise, Domain::MultiAgent, Domain::Science],
        decision_families: &[
            "choose what to include in a message or tool call",
            "form external search queries without leaking private context",
            "route information to the appropriate agent or recipient",
            "store or retrieve sensitive memory",
            "recognize innocuous fragments that become sensitive in aggregate",
            "recognize purpose change or unauthorized secondary use",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::PolicyVeto,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "prism.reward-hacking-and-exploits",
        title: "Reward Hacking and Benchmark Exploits",
        blueprint_module: "15.17",
        axis: PackAxis::Platform,
        measures: "Whether agents exploit graders, hidden state, metadata or task infrastructure \
                   instead of satisfying intended behavior, and whether benchmarks resist such \
                   attacks.",
        capabilities: &[
            agent(AgentCapability::EvaluationIntegrity),
            agent(AgentCapability::Safety),
        ],
        domains: &[Domain::Evaluation, Domain::Coding, Domain::Browser],
        decision_families: &[
            "choose a legitimate solution over grader tampering",
            "handle a visible hidden-answer artifact",
            "respond to an evaluator error",
            "detect an instruction-verifier mismatch",
            "audit a benchmark for exploitability",
            "patch a vulnerable oracle without breaking valid solutions",
            "report rather than exploit a defect",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::PolicyVeto,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "prism.robustness-metamorphic",
        title: "Robustness and Metamorphic Generalization",
        blueprint_module: "15.18",
        axis: PackAxis::Mechanism,
        measures:
            "Whether behavior changes only when task semantics change — and does change when \
                   they do — across representation, ordering, naming, timing and environment \
                   perturbation. Its parents are cells drawn from every other pack.",
        capabilities: &[agent(AgentCapability::Robustness)],
        domains: &[Domain::CrossDomain],
        decision_families: &[
            "invariant action under rename, layout change or paraphrase",
            "equivariant output under unit or identifier mapping",
            "monotonic response to additional evidence or permission",
            "contrastive behavior when one fact changes",
            "robust recovery under a fault-severity ladder",
            "cross-runner and cross-provider conformance",
            "detect unsupported distribution shift",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Statistical,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "prism.benchmark-meta-evaluation",
        title: "Benchmark Meta-Evaluation",
        blueprint_module: "15.19",
        axis: PackAxis::Platform,
        measures: "Benchmark builders, graders, mutation generators and PRISM itself, on task \
                   validity, exploit resistance, reproducibility and diagnostic value.",
        capabilities: &[agent(AgentCapability::EvaluationIntegrity)],
        domains: &[Domain::Evaluation],
        decision_families: &[
            "classify a benchmark defect",
            "find a minimal exploit",
            "determine whether a failure is the agent's or the task's",
            "validate a mutation relation",
            "choose a repair and its regression tests",
            "assess whether a task measures its claimed capability",
            "estimate effective diversity",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::Statistical,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "prism.transfer-and-routing",
        title: "Cross-Domain Transfer and Routing",
        blueprint_module: "15.20",
        axis: PackAxis::Platform,
        measures: "Whether capability estimates and architecture-selection policies generalize to \
                   unseen tasks, domains, tools and distributions, and abstain when they do not.",
        capabilities: &[
            agent(AgentCapability::Routing),
            agent(AgentCapability::Robustness),
        ],
        domains: &[Domain::CrossDomain, Domain::Evaluation],
        decision_families: &[
            "choose an architecture among candidates",
            "choose context, branch, verifier and model budget",
            "predict capability and cost",
            "abstain to a default or request a probe evaluation",
            "update after a small diagnostic panel",
            "detect domain shift and avoid overconfident specialization",
        ],
        oracles: &[
            OracleTier::Executable,
            OracleTier::PolicyVeto,
            OracleTier::Statistical,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "prism.terminal-and-devops",
        title: "Terminal, DevOps and Systems Operations",
        blueprint_module: "15.21",
        axis: PackAxis::Domain,
        measures:
            "Diagnosis and repair of command-line, service, deployment, networking, build and \
                   observability incidents under real state, permission and recovery constraints.",
        capabilities: &[
            agent(AgentCapability::ToolUse),
            agent(AgentCapability::VerificationAndRecovery),
            agent(AgentCapability::Safety),
            agent(AgentCapability::EvidenceAcquisition),
        ],
        domains: &[Domain::Operations],
        decision_families: &[
            "select the next command or log source",
            "interpret ambiguous command output",
            "choose rollback versus forward fix",
            "construct a safe command under permissions",
            "recognize that a command succeeded but the service outcome failed",
            "recover from a partial deployment or killed process",
            "verify service health beyond a superficial status",
        ],
        oracles: &[OracleTier::Deterministic, OracleTier::Executable],
        release_wave: ReleaseWave::Wave(4),
    },
    PackDefinition {
        id: "prism.human-collaboration",
        title: "Human Collaboration and Clarification",
        blueprint_module: "15.22",
        axis: PackAxis::Mechanism,
        measures:
            "Whether agents ask high-value questions, present useful choices, preserve prior \
                   user intent and escalate, instead of optimizing autonomous completion alone.",
        capabilities: &[
            agent(AgentCapability::HumanCollaboration),
            agent(AgentCapability::Safety),
        ],
        domains: &[
            Domain::Coding,
            Domain::Data,
            Domain::Operations,
            Domain::Science,
        ],
        decision_families: &[
            "act versus ask versus abstain",
            "choose the question with the highest expected value",
            "avoid re-asking information already answered",
            "present bounded options with their consequences",
            "summarize state for human takeover",
            "incorporate a correction without losing prior constraints",
            "communicate a negative or uncertain result honestly",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::PolicyVeto,
            OracleTier::Rubric,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "prism.incident-regression-mining",
        title: "Production Incidents and Regression Mining",
        blueprint_module: "15.23",
        axis: PackAxis::Platform,
        measures: "Whether a failed agent run can be turned into a faithful, minimized, reusable \
                   regression, and whether a candidate system fixes the mechanism rather than the \
                   symptom.",
        capabilities: &[
            agent(AgentCapability::EvaluationIntegrity),
            agent(AgentCapability::VerificationAndRecovery),
            agent(AgentCapability::Observability),
        ],
        domains: &[Domain::Evaluation, Domain::Coding],
        decision_families: &[
            "select the causal boundary among candidates",
            "identify state missing for replay",
            "choose a minimization removal",
            "classify the failure mechanism",
            "determine whether two failures are the same family",
            "judge a local suffix fix against end-to-end regression",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "prism.observability-trace-semantics",
        title: "Agent Observability and Trace Semantics",
        blueprint_module: "15.24",
        axis: PackAxis::Platform,
        measures: "Whether tracing and adapters capture enough semantic state for diagnosis, \
                   replay, attribution and audit, without silently fabricating information the \
                   source never carried.",
        capabilities: &[
            agent(AgentCapability::Observability),
            agent(AgentCapability::Privacy),
        ],
        domains: &[Domain::Evaluation],
        decision_families: &[
            "classify event and actor",
            "link a tool request to its result",
            "infer a causal parent with uncertainty",
            "detect a dropped effect or state delta",
            "choose a safe redaction",
            "determine replay grade",
            "resolve out-of-order and duplicate events",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "prism.architecture-component-routing",
        title: "Architecture, Component and Routing",
        blueprint_module: "15.25",
        axis: PackAxis::Platform,
        measures: "Whether an individual context, memory, planning, verification, branching or \
                   routing policy improves the state it targets without introducing harmful \
                   complexity elsewhere.",
        capabilities: &[agent(AgentCapability::Routing)],
        domains: &[Domain::Evaluation, Domain::CrossDomain],
        decision_families: &[
            "select a candidate architecture",
            "choose a context, planner or verifier policy",
            "allocate branch count",
            "decide whether local evidence supports routing",
            "handle an out-of-distribution fingerprint",
            "explain a route with its uncertainty and constraints",
        ],
        oracles: &[OracleTier::PolicyVeto, OracleTier::Statistical],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.source-discovery",
        title: "Source Discovery and Data Access",
        blueprint_module: "29.01",
        axis: PackAxis::Domain,
        measures: "Whether an agent can locate the authoritative, lawful, version-correct data or \
                   evidence source required for a biological question.",
        capabilities: &[
            bio(BioCapability::ResearchOrientation),
            bio(BioCapability::DataIdentity),
        ],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "locate a dataset, accession, paper supplement or code repository",
            "distinguish metadata access from raw-data access",
            "choose among overlapping repositories and releases",
            "construct a reproducible manifest",
            "operate under public, controlled, enclave or historical-cutoff constraints",
        ],
        oracles: &[OracleTier::Deterministic, OracleTier::PolicyVeto],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.data-qc",
        title: "Data Quality Control and Assay Validation",
        blueprint_module: "29.02",
        axis: PackAxis::Domain,
        measures: "Whether an agent detects unusable or biased measurements before downstream \
                   analysis, treating published QC thresholds as context rather than universal \
                   truth.",
        capabilities: &[
            bio(BioCapability::AssayUnderstanding),
            bio(BioCapability::QualityControl),
        ],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "parse assay-specific QC metrics",
            "identify sample and run outliers",
            "distinguish technical failure from biological signal",
            "choose thresholds and justify the tradeoff",
            "evaluate sensitivity to QC choices",
            "decide whether data are fit for the intended claim",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.sample-identity",
        title: "Sample Identity and Lineage",
        blueprint_module: "29.03",
        axis: PackAxis::Domain,
        measures: "Construction and auditing of subject-lesion-specimen-assay-artifact \
                   relationships, including swapped samples and ambiguous mappings that must stay \
                   ambiguous.",
        capabilities: &[bio(BioCapability::DataIdentity)],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "reconstruct lineage from fragmented metadata",
            "detect duplicate, swapped or mismatched samples",
            "align spatial regions and longitudinal time points",
            "preserve uncertainty in ambiguous mappings",
            "construct patient-level splits",
        ],
        oracles: &[OracleTier::Deterministic, OracleTier::Statistical],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.cohort-construction",
        title: "Cohort Construction and Phenotyping",
        blueprint_module: "29.04",
        axis: PackAxis::Domain,
        measures:
            "Whether an agent defines populations, time zero, exposures, outcomes, exclusions \
                   and analysis units reproducibly.",
        capabilities: &[bio(BioCapability::CohortAndStudyDesign)],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "interpret a research question as a cohort contract",
            "build inclusion and exclusion logic",
            "define baseline and follow-up",
            "handle repeated measures and censoring",
            "produce a machine-readable cohort manifest",
            "audit transportability and missingness",
        ],
        oracles: &[OracleTier::Deterministic, OracleTier::Executable],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.statistical-estimands",
        title: "Statistical Estimands and Analysis Selection",
        blueprint_module: "29.05",
        axis: PackAxis::Domain,
        measures: "Whether an agent defines the quantity of interest and selects analyses \
                   consistent with study design and data generation, reporting effect size rather \
                   than significance alone.",
        capabilities: &[bio(BioCapability::StatisticalAndCausalInference)],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "state the estimand, unit, population and uncertainty target",
            "choose models for repeated measures, censoring, counts, compositional or spatial data",
            "check assumptions and diagnostics",
            "control multiplicity",
            "conduct sensitivity and robustness analyses",
            "interpret effect size rather than only significance",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.causal-inference",
        title: "Causal Inference and Confounding",
        blueprint_module: "29.06",
        axis: PackAxis::Domain,
        measures: "Whether an agent distinguishes prediction, association, mechanism and \
                   intervention effects, and recognizes what is not identifiable at all.",
        capabilities: &[bio(BioCapability::StatisticalAndCausalInference)],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "draw or critique a causal graph",
            "identify confounders, mediators, colliders and selection",
            "choose a target trial or experimental design",
            "apply negative controls or sensitivity analyses",
            "recognize non-identifiability",
            "avoid causal language when its obligations are unmet",
        ],
        oracles: &[
            OracleTier::Executable,
            OracleTier::Statistical,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.ml-pipeline",
        title: "Machine Learning Pipeline Development",
        blueprint_module: "29.07",
        axis: PackAxis::Domain,
        measures: "End-to-end model construction with leakage-free splits, a baseline before \
                   complex models, calibration, and external generalization.",
        capabilities: &[
            bio(BioCapability::ModelDevelopment),
            bio(BioCapability::ComputationalReproducibility),
        ],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "understand the prediction unit and target",
            "build leakage-free splits",
            "construct a baseline before complex models",
            "fit, tune, calibrate and estimate uncertainty",
            "evaluate external generalization",
            "package code, model and artifacts reproducibly",
        ],
        oracles: &[OracleTier::Deterministic, OracleTier::Executable],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.multimodal-integration",
        title: "Multimodal Integration",
        blueprint_module: "29.08",
        axis: PackAxis::Domain,
        measures: "Whether an agent combines modalities only when their subjects, specimens, \
                   regions, times and biological scales are compatible, and shows incremental \
                   value.",
        capabilities: &[bio(BioCapability::MultimodalTranslation)],
        domains: &[Domain::Biomedical, Domain::Multimodal],
        decision_families: &[
            "create a cross-modal manifest",
            "assess missingness and overlap",
            "choose early, late, joint or evidence-level integration",
            "avoid using one modality as an unvalidated proxy for another",
            "evaluate incremental value and ablations",
            "preserve modality-specific uncertainty",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Statistical,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.hypothesis-generation",
        title: "Hypothesis Generation and Competing Explanations",
        blueprint_module: "29.09",
        axis: PackAxis::Domain,
        measures: "Whether agents generate diverse, falsifiable, evidence-linked explanations \
                   instead of one fluent story, and keep unresolved alternatives alive.",
        capabilities: &[bio(BioCapability::InterpretationAndHypothesis)],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "produce a structured hypothesis set",
            "identify the assumptions separating hypotheses",
            "assign priors or confidence with rationale",
            "choose evidence that discriminates alternatives",
            "revise or retire hypotheses",
            "preserve unresolved alternatives",
        ],
        oracles: &[OracleTier::Executable, OracleTier::ExpertReview],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.experiment-design",
        title: "Experiment Design and Falsification",
        blueprint_module: "29.10",
        axis: PackAxis::Domain,
        measures:
            "Whether an agent proposes feasible experiments that can invalidate a hypothesis \
                   as well as support it.",
        capabilities: &[bio(BioCapability::ExperimentAndEvidence)],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "define question, intervention, comparator, outcome and unit",
            "choose controls, randomization, blinding, replication and sample size logic",
            "anticipate assay and model limitations",
            "pre-register analysis and stopping",
            "identify failure modes and alternative interpretations",
            "prioritize orthogonal validation",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.value-of-information",
        title: "Value of Information and Assay Selection",
        blueprint_module: "29.11",
        axis: PackAxis::Domain,
        measures: "Which analysis, assay, cohort or expert review an agent acquires next under \
                   cost, time, sample, privacy and risk constraints — and whether predicted value \
                   matches realized value.",
        capabilities: &[bio(BioCapability::ExperimentAndEvidence)],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "enumerate feasible evidence actions",
            "predict their hypothesis discrimination",
            "account for cost, time, sample, privacy and risk",
            "choose an action or stop",
            "compare predicted and realized value",
            "learn from repeated decisions",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.negative-results-abstention",
        title: "Negative Results, Non-Identifiability and Abstention",
        blueprint_module: "29.12",
        axis: PackAxis::Domain,
        measures: "Whether an agent recognizes that evidence does not support a conclusion, \
                   separates no effect from no power, and declines to salvage a preferred \
                   hypothesis.",
        capabilities: &[
            bio(BioCapability::VerificationAndAbstention),
            bio(BioCapability::StatisticalAndCausalInference),
        ],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "distinguish no effect from no power",
            "detect a non-identifiable estimand",
            "interpret failed QC or assay limits",
            "abstain with explicit missing evidence",
            "propose the cheapest discriminating next step",
            "avoid narrative salvage of a preferred hypothesis",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.reproducibility-claim-tracing",
        title: "Reproducibility and Claim Tracing",
        blueprint_module: "29.13",
        axis: PackAxis::Domain,
        measures:
            "Whether an agent can reconstruct a result and connect every conclusion to exact \
                   data, code, environment and source evidence, explaining any discrepancy.",
        capabilities: &[bio(BioCapability::ComputationalReproducibility)],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "find data and code",
            "reconstruct the environment",
            "execute the analysis",
            "compare figures, tables and statistics",
            "explain discrepancies",
            "emit a portable research object",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::Statistical,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.claim-boundaries",
        title: "Biological Interpretation and Claim Boundaries",
        blueprint_module: "29.14",
        axis: PackAxis::Domain,
        measures:
            "Whether an agent interprets a result at the correct biological scale and within \
                   its population, assay and causal limits, without changing the estimand when it \
                   changes audience.",
        capabilities: &[
            bio(BioCapability::InterpretationAndHypothesis),
            bio(BioCapability::CollaborationAndGovernance),
        ],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "summarize a result without changing the estimand",
            "separate observation, association, mechanism and utility",
            "identify plausible alternatives",
            "map evidence onto the Causal Translation Lattice",
            "write population and external-validity boundaries",
            "communicate uncertainty to different audiences without factual drift",
        ],
        oracles: &[OracleTier::Deterministic, OracleTier::ExpertReview],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.biomarker-validation",
        title: "Biomarker Discovery and Validation",
        blueprint_module: "29.15",
        axis: PackAxis::Domain,
        measures: "Biomarker workflows from candidate generation through analytical validity, \
                   external validation, incremental value and a bounded utility claim.",
        capabilities: &[
            bio(BioCapability::ModelDevelopment),
            bio(BioCapability::InterpretationAndHypothesis),
        ],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "define intended use and population",
            "choose a discovery design and its controls",
            "avoid leakage and overfitting",
            "validate analytically and externally",
            "assess calibration and incremental value",
            "write a bounded evidence claim",
        ],
        oracles: &[OracleTier::Deterministic, OracleTier::Executable],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.target-validation",
        title: "Target Discovery and Validation",
        blueprint_module: "29.16",
        axis: PackAxis::Domain,
        measures: "Evidence integration for target hypotheses across genetics, perturbation, \
                   expression, dependency, mechanism, model relevance and pharmacology, \
                   distinguishing correlation from dependency.",
        capabilities: &[
            bio(BioCapability::InterpretationAndHypothesis),
            bio(BioCapability::ExperimentAndEvidence),
        ],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "define target and disease context",
            "assemble independent evidence channels",
            "distinguish correlation from dependency and mechanism",
            "identify liabilities and context specificity",
            "propose orthogonal validation",
            "map the translation chain and its unresolved edges",
        ],
        oracles: &[OracleTier::Executable, OracleTier::ExpertReview],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.cross-cohort-replication",
        title: "Cross-Cohort Replication and Generalization",
        blueprint_module: "29.17",
        axis: PackAxis::Domain,
        measures:
            "Whether findings survive independent cohorts, sites, platforms, populations and \
                   analytic choices, and whether the claim scope is updated when they do not.",
        capabilities: &[
            bio(BioCapability::ModelDevelopment),
            bio(BioCapability::QualityControl),
        ],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "find a genuinely independent validation cohort",
            "harmonize without erasing differences",
            "pre-specify replication criteria",
            "quantify heterogeneity",
            "explain failure or partial replication",
            "update the claim scope",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Statistical,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.cross-species-translation",
        title: "Cross-Species and Translational Reasoning",
        blueprint_module: "29.18",
        axis: PackAxis::Domain,
        measures:
            "Whether evidence is transported across model systems and biological scales with \
                   explicit assumptions, and whether the translation cliff is located.",
        capabilities: &[bio(BioCapability::MultimodalTranslation)],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "map orthologs and model characteristics",
            "identify conserved and divergent mechanisms",
            "evaluate model exposure and phenotype relevance",
            "combine complementary models",
            "locate the translation cliff",
            "propose the next validation scale",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.evidence-synthesis",
        title: "Literature, Trial and Evidence Synthesis",
        blueprint_module: "29.19",
        axis: PackAxis::Domain,
        measures: "Multi-source synthesis that preserves study design, population, time, result \
                   direction, uncertainty and source-level provenance, producing a claim map \
                   rather than a narrative.",
        capabilities: &[
            bio(BioCapability::ResearchOrientation),
            bio(BioCapability::InterpretationAndHypothesis),
        ],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "formulate a structured question",
            "retrieve and screen sources",
            "extract comparable evidence",
            "assess bias and applicability",
            "reconcile conflicts",
            "produce a claim map rather than a narrative-only answer",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Statistical,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.multi-agent-collaboration",
        title: "Multi-Agent Biological Collaboration",
        blueprint_module: "29.20",
        axis: PackAxis::Domain,
        measures: "Whether specialized agents share evidence, assumptions, authority and \
                   continuations without losing biological provenance, and challenge unsupported \
                   claims.",
        capabilities: &[
            bio(BioCapability::CollaborationAndGovernance),
            agent(AgentCapability::Coordination),
        ],
        domains: &[Domain::Biomedical, Domain::MultiAgent],
        decision_families: &[
            "assign roles by modality and decision need",
            "create recipient-specific Context Capsules",
            "delegate analyses with explicit contracts",
            "challenge unsupported claims",
            "fork competing hypotheses",
            "merge material and epistemic state",
            "escalate unresolved conflict",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::Executable,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
    PackDefinition {
        id: "bio.safety-and-dual-use",
        title: "Safety, Dual Use and Boundary Compliance",
        blueprint_module: "29.21",
        axis: PackAxis::Domain,
        measures: "Whether biological agents respect data, clinical, physical and dual-use \
                   boundaries while remaining useful on benign research tasks.",
        capabilities: &[
            bio(BioCapability::CollaborationAndGovernance),
            agent(AgentCapability::Safety),
        ],
        domains: &[Domain::Biomedical],
        decision_families: &[
            "classify data and action risk",
            "request only necessary permissions",
            "refuse or redirect prohibited operational work",
            "preserve research utility under restrictions",
            "avoid clinical overreach",
            "log and escalate ambiguous cases",
        ],
        oracles: &[
            OracleTier::Deterministic,
            OracleTier::PolicyVeto,
            OracleTier::ExpertReview,
        ],
        release_wave: ReleaseWave::Unsequenced,
    },
];
