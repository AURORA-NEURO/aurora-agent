//! Autonomous research protocol runners: a validated request in, a digested dossier or a
//! domain-specific execution plan out — with every finding derived by a fixed rule from a cited
//! measurement.
//!
//! The original runner remains autonomous **measurement science over synthetic decision worlds** —
//! committed fixtures and seeded generators. [`glioma_engine`] adds a second production surface:
//! a preclinical glioma program planner and execution loop that delegates real work to
//! caller-owned local providers. The blueprint does not specify either runner; their request,
//! workflow, artifact and report schemas are this crate's design, stated as such. What the
//! synthetic steps *measure* is specified, and each step calls the crate that owns it: 43.26
//! context certificates via `bioprism-fiber`, the 43.38
//! equal-engineering comparison and the 43.39 structural families and sweep via
//! `bioprism-baseline`/`bioprism-worldgen`, the 03.08/32 metamorphic suite via
//! `bioprism-mutation`, and the 1-minimal reduction (in 43.40/43.41's refusal vocabulary) via
//! `bioprism-prism`. This crate adds orchestration and receipts, never measurement logic.
//!
//! # The honesty rules this crate is built around
//!
//! - **The question is never interpreted.** A [`ResearchRequest`] records its free-text question
//!   verbatim; the protocol is planned from the *other* fields alone, and no code path anywhere
//!   in this crate branches on the question's content. The runner executes the protocol; it does
//!   not understand the question.
//! - **Findings are derived, never free-generated.** Every [`Finding`] comes from one of the
//!   fixed rules in [`findings`], is levelled [`ObservationLevel::Observation`] — a
//!   single-variant enum, so no other level is representable — and cites the content digests of
//!   the artifacts it was derived from.
//! - **Negative findings are first-class.** A tie between the compiler and a baseline is a
//!   *required* finding, flagged `negative: true` and rendered in the same register as any
//!   positive result. The repository's own headline finding is a tie.
//! - **Every dossier anchors to the pinned parity digest.** Step 0 compiles the committed
//!   `fixtures/fiber-v0.1` pair (embedded at build time) and aborts unless the certificate digest
//!   is [`PINNED_REFERENCE_CERTIFICATE_SHA256`], the value CPython, the eager Rust path, and the
//!   indexed store agree on.
//! - **Partial protocols are unrepresentable.** A step that cannot complete is a typed
//!   [`ResearchError`]; [`dossier::StepOutcome`] has exactly one variant, so an emitted dossier
//!   cannot claim a step it did not finish.
//! - **The dossier is tamper-evident.** `dossier_sha256` is computed over the canonical document
//!   with the digest field removed; [`verify_dossier`] recomputes it, checks the 64-hex shape
//!   separately from mismatch, and checks that every finding's supporting digests actually name
//!   artifacts the dossier carries.
//!
//! # Boundary
//!
//! Research and developer infrastructure: it does not diagnose an individual, recommend
//! treatment, triage care, enroll participants, or claim medical-device functionality. It can
//! never claim biology or medicine, literature or prior-work coverage, external-world
//! observation, or release-level claims from fixture evidence. Oracle review is a human gate.
//! These sentences also ship inside every dossier as [`REQUIRED_LIMITATIONS`].
//!
//! # Not implemented
//!
//! - **No question understanding.** The question is recorded and rendered verbatim, digested,
//!   and never parsed, matched, or routed on. There is no NLP anywhere in this crate.
//! - **No literature fetching in the synthetic runner.** Nothing in that runner searches, cites,
//!   or claims coverage of prior work. The glioma engine accepts literature and other evidence as
//!   caller-supplied local artifact references and delegates retrieval to a provider.
//! - **No recurrence in the synthetic runner.** One request runs one protocol to one dossier in
//!   one call. The glioma engine provides bounded stage retries and checkpoints, but recurrence
//!   scheduling is still owned by the host.
//! - **No wall-clock.** No timestamps, durations, or dates appear in any artifact; determinism
//!   is byte-for-byte and the only order is protocol order.
//! - **No oracle acceptance.** The runner emits observations for human review and accepts,
//!   approves, and releases nothing.
//! - **Figures are static SVG only**, rendered by `bioprism-figures`: no raster, no scripts, no
//!   interactivity.
//! - **No I/O.** Fixtures are compiled in; the caller writes the dossier, report, and figures
//!   wherever they belong.
//! - **No generator knobs beyond the presets.** A request chooses one committed 43.39 family,
//!   one seed, and up to six distractor counts; skeleton, events, protected set, decision time,
//!   and policy stay at each preset's committed values, and the sweep runs the committed default
//!   grid at the grid's own seed.

pub mod dossier;
pub mod error;
pub mod findings;
pub mod glioma;
pub mod glioma_engine;
pub mod protocol;
pub mod report;
pub mod request;
pub mod runner;

pub use dossier::{
    artifact_record, build_dossier, verify_dossier, RecordedArtifact, StepOutcome, DOSSIER_SCHEMA,
    INLINE_ARTIFACT_CAP_BYTES, REQUIRED_LIMITATIONS,
};
pub use error::ResearchError;
pub use findings::{
    comparison_findings, minimization_findings, mutation_findings, reference_anchor_finding,
    sweep_findings, Finding, ObservationLevel,
};
pub use glioma::{
    analyze_glioma_causal_contrast, analyze_glioma_combination_synergy,
    analyze_glioma_dose_response, analyze_glioma_trajectories, analyze_multimodal_concordance,
    analyze_multimodal_consensus, analyze_preclinical_outcomes, analyze_replication_meta_analysis,
    assess_glioma_robustness, assess_replication, build_research_object_manifest,
    compile_decision_context, compile_typed_knowledge, design_preclinical_experiment,
    execute_glioma_workflow, explore_mechanisms, generate_feature_catalog, glioma_program_catalog,
    harmonize_multimodal_inputs, plan_glioma_workflow, protocol_request_from_experiment_design,
    qualify_evidence, simulate_glioma_protocol, validate_feature_catalog, AnalysisDataset,
    AnalysisRequest, AnalysisResult, CatalogError, CausalContrastAnalysis,
    CausalContrastDisposition, CausalContrastError, CausalContrastRequest, CombinationCell,
    CombinationCellDisposition, CombinationObservation, CombinationSynergyAnalysis,
    CombinationSynergyDisposition, CombinationSynergyError, CombinationSynergyRequest,
    ConcordanceDisposition, ConcordanceError, ConcordanceRequest, ConsensusAssignment,
    ConsensusCluster, ConsensusDisposition, ConsensusError, ConsensusRequest, DecisionAction,
    DecisionActionKind, DecisionContext, DecisionContextDisposition, DecisionContextError,
    DecisionContextRequest, DoseDirection, DosePair, DoseResponseAnalysis, DoseResponseDisposition,
    DoseResponseError, DoseResponseObservation, DoseResponsePoint, DoseResponseRequest,
    EvidenceQualification, EvidenceRecord, EvidenceRequest, ExperimentArm, ExperimentDesign,
    ExperimentRequest, FeatureValue, GliomaFeatureSpec, GliomaOperatingScale,
    GliomaProgramDescriptor, GliomaProgramId, GliomaWorkflowBranch, GliomaWorkflowError,
    GliomaWorkflowExecution, GliomaWorkflowMode, GliomaWorkflowNode, GliomaWorkflowPlan,
    GliomaWorkflowRequest, KnowledgeClaim, KnowledgeClaimDisposition, KnowledgeDisposition,
    KnowledgeError, KnowledgeRequest, MechanismCandidate, MechanismPortfolio, MechanismRequest,
    MetaAnalysisDisposition, MetaAnalysisError, MetaAnalysisRequest, MetaStudyContribution,
    ModalityConcordance, ModalityVector, MultimodalConcordance, MultimodalConsensus,
    MultimodalObservation, MultimodalQcReport, MultimodalRequest, PairConcordanceDisposition,
    ProtocolDisposition, ProtocolResource, ProtocolResourceKind, ProtocolSimulation,
    ProtocolSimulationError, ProtocolSimulationRequest, ProtocolTask, ReplicationAssessment,
    ReplicationMetaAnalysis, ReplicationRequest, ReplicationStudy, ResearchObjectManifest,
    ResearchObjectRequest, ResourceUtilization, RobustnessCase, RobustnessCaseKind,
    RobustnessDisposition, RobustnessError, RobustnessRequest, RobustnessSuite, ScheduleEntry,
    TrajectoryAnalysis, TrajectoryArmSummary, TrajectoryDisposition, TrajectoryError,
    TrajectoryObservation, TrajectoryRequest, TypedKnowledge, UnitContrast, UnitTrajectory,
    UnitTrajectoryDisposition, WorkflowNodeDecision,
};
pub use glioma_engine::{
    compile_glioma_research, dry_run_glioma_research, execute_glioma_research,
    glioma_research_engine_manifest, select_glioma_actions, DryRunGliomaExecutor,
    GliomaActionCandidate, GliomaActionDecision, GliomaActionSelection, GliomaEngineError,
    GliomaExecutionReceipt, GliomaModality, GliomaModelSystem, GliomaPlanDisposition,
    GliomaResearchIntent, GliomaResearchPlan, GliomaSelectionConfig, GliomaSelectionWeights,
    GliomaStage, GliomaStageDisposition, GliomaStageExecution, GliomaStageExecutor,
    GliomaStageFailure, GliomaStageInput, GliomaStageKind, LocalArtifactRef, StageReadiness,
    ACTION_SELECTION_OUTPUT_SCHEMA as GLIOMA_ACTION_SELECTION_OUTPUT_SCHEMA,
    CONTRACT_VERSION as GLIOMA_ENGINE_CONTRACT_VERSION, FEATURE_ID as GLIOMA_ENGINE_FEATURE_ID,
};
pub use protocol::{plan_protocol, ProtocolStep, ResearchProtocol};
pub use report::{render_report, RenderedReport};
pub use request::{
    ResearchRequest, ResearchRequestDocument, WorldFamily, MAX_DISTRACTORS_PER_POINT,
    MAX_DISTRACTOR_POINTS, MAX_QUESTION_BYTES, MAX_RESEARCH_ID_CHARS,
};
pub use runner::{run_research, PINNED_REFERENCE_CERTIFICATE_SHA256, UNSWEPT_KNOBS_CAVEAT};
