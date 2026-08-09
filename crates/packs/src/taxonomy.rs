//! The axes a pack is indexed on: axis, domain, capability family, oracle tier, release wave.
//!
//! Blueprint 15.00 describes the portfolio as a matrix — "rows are capability nodes; columns are
//! domains/modalities/effect classes" — and requires results to be indexed on both axes. It does
//! not, however, enumerate the capability nodes. Section 29.00 does enumerate them, but only for
//! biology (B0–B12). So this module carries two capability vocabularies and says which is whose:
//! the biological one is the blueprint's, the agent one is derived here from the mechanism list
//! in 15.00 plus the packs that 15.01–15.25 actually define. The `A*` codes are ours and are not
//! blueprint identifiers.
//!
//! Oracle tiers are the load-bearing part. A pack's capability claim is only as good as the
//! strongest oracle that can decide its instances, and the blueprint invariant that
//! "nondeterministic judgments never silently override deterministic or execution-grounded
//! evidence" is expressed here as [`OracleTier::may_override`] rather than left to convention.

use serde::{Deserialize, Serialize};

/// Which of the portfolio's axes a pack sits on.
///
/// 15.00 names two: mechanism packs isolate context, planning, memory, tools, verification,
/// coordination, safety and robustness; domain packs place those mechanisms inside coding,
/// browser, data, science, biomedical and neuroscience worlds.
///
/// Six modules do not fit either. 15.17, 15.19, 15.20, 15.23, 15.24 and 15.25 measure the
/// evaluation platform — graders, traces, minimization, routing policies — not an agent
/// mechanism situated in a domain. Filing them under "mechanism" would let a healthy score on
/// benchmark meta-evaluation read as evidence about agents. [`PackAxis::Platform`] is therefore
/// added here, and is flagged in the report as an extension rather than blueprint text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackAxis {
    /// Isolates one agent decision mechanism across several worlds.
    Mechanism,
    /// Places mechanisms inside one world type.
    Domain,
    /// Measures the evaluation platform itself. Not a blueprint axis; see the type docs.
    Platform,
}

impl PackAxis {
    /// Whether 15.00 names this axis, or whether this crate added it.
    pub fn is_blueprint_axis(self) -> bool {
        matches!(self, PackAxis::Mechanism | PackAxis::Domain)
    }
}

/// The column of the coverage matrix: the kind of world a pack's parents live in.
///
/// 15.00 lists six (coding, browser, data, science, biomedical, neuroscience). The remaining six
/// are required by the modules themselves — 15.21 is a terminal/DevOps world, 15.14 is indexed by
/// modality rather than world, 15.18 and 15.20 hold out whole domains by construction — and are
/// marked by [`Domain::is_blueprint_domain`] so a coverage table can show which columns the
/// blueprint actually sanctioned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Coding,
    Browser,
    Data,
    Science,
    Biomedical,
    Neuroscience,
    Operations,
    Enterprise,
    MultiAgent,
    Multimodal,
    CrossDomain,
    Evaluation,
}

impl Domain {
    pub fn is_blueprint_domain(self) -> bool {
        matches!(
            self,
            Domain::Coding
                | Domain::Browser
                | Domain::Data
                | Domain::Science
                | Domain::Biomedical
                | Domain::Neuroscience
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Domain::Coding => "coding",
            Domain::Browser => "browser and computer use",
            Domain::Data => "data, spreadsheets and databases",
            Domain::Science => "scientific reasoning",
            Domain::Biomedical => "biomedical research",
            Domain::Neuroscience => "neuroscience and neuro-oncology",
            Domain::Operations => "terminal, DevOps and systems operations",
            Domain::Enterprise => "enterprise documents and workflows",
            Domain::MultiAgent => "multi-agent teams",
            Domain::Multimodal => "multimodal artifacts",
            Domain::CrossDomain => "held-out and cross-domain",
            Domain::Evaluation => "the evaluation platform itself",
        }
    }

    pub const ALL: &'static [Domain] = &[
        Domain::Coding,
        Domain::Browser,
        Domain::Data,
        Domain::Science,
        Domain::Biomedical,
        Domain::Neuroscience,
        Domain::Operations,
        Domain::Enterprise,
        Domain::MultiAgent,
        Domain::Multimodal,
        Domain::CrossDomain,
        Domain::Evaluation,
    ];
}

/// Agent-side capability nodes.
///
/// Section 15 never numbers its capability nodes; it names mechanisms in prose (15.00) and then
/// defines one pack per mechanism or domain. These fourteen are read back off 15.01–15.25 rather
/// than invented, but the `A*` codes are this crate's and should not be cited as blueprint ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCapability {
    /// 15.01: seek the smallest, highest-value evidence and stop.
    EvidenceAcquisition,
    /// 15.02: choose tools by capability, build valid calls, read results.
    ToolUse,
    /// 15.03: write, retrieve, expire, scope and refuse to store.
    Memory,
    /// 15.04: generate alternatives, expose assumptions, delay commitment.
    HypothesisAndPlanning,
    /// 15.05: falsify, detect silent failure, localize, recover.
    VerificationAndRecovery,
    /// 15.06: hold state and obligations across long executions, and stop.
    LongHorizonState,
    /// 15.07: delegate, merge, resolve conflict, avoid deadlock.
    Coordination,
    /// 15.15: effect, reversibility, least privilege, confirmation.
    Safety,
    /// 15.16: contextual integrity across recipients, purposes and channels.
    Privacy,
    /// 15.17: complete the intended task rather than the measured one.
    EvaluationIntegrity,
    /// 15.18: change behavior only when semantics change.
    Robustness,
    /// 15.20 and 15.25: predict which architecture works, and abstain when unsure.
    Routing,
    /// 15.22: ask, present options, preserve constraints, escalate.
    HumanCollaboration,
    /// 15.24: emit traces that support diagnosis, replay and audit.
    Observability,
}

impl AgentCapability {
    pub fn code(self) -> &'static str {
        match self {
            AgentCapability::EvidenceAcquisition => "A00",
            AgentCapability::ToolUse => "A01",
            AgentCapability::Memory => "A02",
            AgentCapability::HypothesisAndPlanning => "A03",
            AgentCapability::VerificationAndRecovery => "A04",
            AgentCapability::LongHorizonState => "A05",
            AgentCapability::Coordination => "A06",
            AgentCapability::Safety => "A07",
            AgentCapability::Privacy => "A08",
            AgentCapability::EvaluationIntegrity => "A09",
            AgentCapability::Robustness => "A10",
            AgentCapability::Routing => "A11",
            AgentCapability::HumanCollaboration => "A12",
            AgentCapability::Observability => "A13",
        }
    }

    pub const ALL: &'static [AgentCapability] = &[
        AgentCapability::EvidenceAcquisition,
        AgentCapability::ToolUse,
        AgentCapability::Memory,
        AgentCapability::HypothesisAndPlanning,
        AgentCapability::VerificationAndRecovery,
        AgentCapability::LongHorizonState,
        AgentCapability::Coordination,
        AgentCapability::Safety,
        AgentCapability::Privacy,
        AgentCapability::EvaluationIntegrity,
        AgentCapability::Robustness,
        AgentCapability::Routing,
        AgentCapability::HumanCollaboration,
        AgentCapability::Observability,
    ];
}

/// The biological capability taxonomy B0–B12, verbatim from blueprint 29.00.
///
/// Unlike the agent vocabulary these codes *are* blueprint identifiers, so they are reproduced
/// exactly. 29.00 is explicit that a capability score is meaningless without its index —
/// organism, modality, oracle class, architecture, date — and that "there is no context-free
/// universal biology score". This enum supplies only the first axis of that index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BioCapability {
    /// B0 research orientation and source discovery.
    ResearchOrientation,
    /// B1 data identity, access, and provenance.
    DataIdentity,
    /// B2 assay and measurement understanding.
    AssayUnderstanding,
    /// B3 quality control and harmonization.
    QualityControl,
    /// B4 cohort and study design.
    CohortAndStudyDesign,
    /// B5 statistical and causal inference.
    StatisticalAndCausalInference,
    /// B6 computational execution and reproducibility.
    ComputationalReproducibility,
    /// B7 biological interpretation and hypothesis management.
    InterpretationAndHypothesis,
    /// B8 experiment and evidence acquisition.
    ExperimentAndEvidence,
    /// B9 multimodal and multi-scale translation.
    MultimodalTranslation,
    /// B10 model development and generalization.
    ModelDevelopment,
    /// B11 verification, uncertainty, and abstention.
    VerificationAndAbstention,
    /// B12 collaboration, communication, and governance.
    CollaborationAndGovernance,
}

impl BioCapability {
    pub fn code(self) -> &'static str {
        match self {
            BioCapability::ResearchOrientation => "B0",
            BioCapability::DataIdentity => "B1",
            BioCapability::AssayUnderstanding => "B2",
            BioCapability::QualityControl => "B3",
            BioCapability::CohortAndStudyDesign => "B4",
            BioCapability::StatisticalAndCausalInference => "B5",
            BioCapability::ComputationalReproducibility => "B6",
            BioCapability::InterpretationAndHypothesis => "B7",
            BioCapability::ExperimentAndEvidence => "B8",
            BioCapability::MultimodalTranslation => "B9",
            BioCapability::ModelDevelopment => "B10",
            BioCapability::VerificationAndAbstention => "B11",
            BioCapability::CollaborationAndGovernance => "B12",
        }
    }

    pub const ALL: &'static [BioCapability] = &[
        BioCapability::ResearchOrientation,
        BioCapability::DataIdentity,
        BioCapability::AssayUnderstanding,
        BioCapability::QualityControl,
        BioCapability::CohortAndStudyDesign,
        BioCapability::StatisticalAndCausalInference,
        BioCapability::ComputationalReproducibility,
        BioCapability::InterpretationAndHypothesis,
        BioCapability::ExperimentAndEvidence,
        BioCapability::MultimodalTranslation,
        BioCapability::ModelDevelopment,
        BioCapability::VerificationAndAbstention,
        BioCapability::CollaborationAndGovernance,
    ];
}

/// A row of the coverage matrix, drawn from either vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityFamily {
    Agent(AgentCapability),
    Biology(BioCapability),
}

impl CapabilityFamily {
    pub fn code(self) -> &'static str {
        match self {
            CapabilityFamily::Agent(c) => c.code(),
            CapabilityFamily::Biology(c) => c.code(),
        }
    }

    /// Whether the code is a blueprint identifier (B0–B12) or this crate's (A00–A13).
    pub fn code_is_from_blueprint(self) -> bool {
        matches!(self, CapabilityFamily::Biology(_))
    }

    /// Every family in both vocabularies. The denominator of the gap report.
    pub fn all() -> Vec<CapabilityFamily> {
        AgentCapability::ALL
            .iter()
            .map(|c| CapabilityFamily::Agent(*c))
            .chain(
                BioCapability::ALL
                    .iter()
                    .map(|c| CapabilityFamily::Biology(*c)),
            )
            .collect()
    }
}

/// How strong the strongest available judgement of an instance is.
///
/// Read off the "oracle strategy" section of every module in 15 and 29. The ordering matters more
/// than the names: a capability covered only by packs whose best oracle is a rubric is covered on
/// paper and not in fact, which is what [`crate::coverage`] reports as weak coverage.
///
/// Not modelled here: oracle *reliability*. Two packs may both declare `ExpertReview` and differ
/// by a factor of three in inter-rater agreement. Tier is an upper bound on what could be
/// decided, never a measurement of what was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleTier {
    /// An exact predicate over recorded state: schema checks, unique keys, lineage,
    /// set-valued acceptable actions, state-transition assertions.
    Deterministic,
    /// Runs something: hidden tests, recomputation, cohort queries, simulation ground truth,
    /// bounded suffix execution.
    Executable,
    /// A policy engine over effects: permission, privacy, irreversibility, forbidden
    /// materialization. Deterministic, but scoped to vetoing rather than scoring.
    PolicyVeto,
    /// A threshold or interval: tolerance-based comparison, meta-analytic heterogeneity,
    /// calibration, regret against a candidate set. Deterministic only once the threshold is
    /// fixed, and the threshold is a choice.
    Statistical,
    /// Domain expert adjudication. Reproducible only through agreement statistics.
    ExpertReview,
    /// A preference rubric or model judge. Nondeterministic by construction.
    Rubric,
}

impl OracleTier {
    /// Higher is more grounded. Used for `max_by_key`, never for serialization order.
    pub fn strength(self) -> u8 {
        match self {
            OracleTier::Deterministic => 6,
            OracleTier::Executable => 5,
            OracleTier::PolicyVeto => 4,
            OracleTier::Statistical => 3,
            OracleTier::ExpertReview => 2,
            OracleTier::Rubric => 1,
        }
    }

    /// Whether a disagreement can be settled by re-running rather than re-asking.
    pub fn is_execution_grounded(self) -> bool {
        matches!(
            self,
            OracleTier::Deterministic | OracleTier::Executable | OracleTier::PolicyVeto
        )
    }

    pub fn is_nondeterministic(self) -> bool {
        matches!(self, OracleTier::ExpertReview | OracleTier::Rubric)
    }

    /// Blueprint invariant (03.06, 15.00): "Nondeterministic judgments never silently override
    /// deterministic or execution-grounded evidence."
    ///
    /// This is a permission check, not a merge rule. It says a rubric may not overturn a failing
    /// hidden test; it says nothing about how the two should be combined when both are advisory.
    pub fn may_override(self, existing: OracleTier) -> bool {
        !(self.is_nondeterministic() && existing.is_execution_grounded())
    }

    pub const ALL: &'static [OracleTier] = &[
        OracleTier::Deterministic,
        OracleTier::Executable,
        OracleTier::PolicyVeto,
        OracleTier::Statistical,
        OracleTier::ExpertReview,
        OracleTier::Rubric,
    ];
}

/// Where a pack sits in 15.00's initial release order.
///
/// 15.00 sequences eight waves covering thirteen of the twenty-five packs. The other twelve are
/// simply not sequenced, and are recorded as such rather than being assigned a plausible wave —
/// an invented ordering would look like a plan and read like blueprint text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseWave {
    /// Waves 1–8 of the 15.00 initial release order.
    Wave(u8),
    /// The blueprint does not place this pack in the release order.
    Unsequenced,
}

impl ReleaseWave {
    pub fn is_sequenced(self) -> bool {
        matches!(self, ReleaseWave::Wave(_))
    }
}
