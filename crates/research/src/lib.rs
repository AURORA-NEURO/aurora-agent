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
    allocate_glioma_assays, analyze_causal_sensitivity, analyze_federated_benchmark,
    analyze_glioma_causal_contrast, analyze_glioma_combination_synergy,
    analyze_glioma_dose_response, analyze_glioma_latent_factors, analyze_glioma_mediation,
    analyze_glioma_spatial_communication, analyze_glioma_spatial_niches,
    analyze_glioma_spatial_state_propagation, analyze_glioma_state_transitions,
    analyze_glioma_trajectories, analyze_instrument_calibration, analyze_multimodal_concordance,
    analyze_multimodal_consensus, analyze_preclinical_outcomes, analyze_replication_meta_analysis,
    analyze_stratified_causal_adjustment, assess_glioma_robustness, assess_replication,
    build_research_object_manifest, compile_decision_context, compile_mechanism_action_plan,
    compile_typed_knowledge, design_preclinical_experiment, discriminate_mechanisms,
    execute_glioma_action_portfolio, execute_glioma_adaptive_information_campaign,
    execute_glioma_autonomous_campaign, execute_glioma_computation,
    execute_glioma_evidence_campaign, execute_glioma_instrument_plan, execute_glioma_protocol,
    execute_glioma_research_autopilot, execute_glioma_workflow, explore_mechanisms,
    generate_feature_catalog, glioma_program_catalog, harmonize_glioma_multimodal_batches,
    harmonize_multimodal_inputs, implemented_feature_ids, plan_decision_actions,
    plan_glioma_adaptive_information_campaign, plan_glioma_closed_loop_campaign,
    plan_glioma_information_design, plan_glioma_multi_fidelity_optimization,
    plan_glioma_robust_intervention_portfolio, plan_glioma_workflow, preflight_glioma_instrument,
    prioritize_glioma_evidence, prioritize_knowledge_frontier, propagate_glioma_mechanism_graph,
    protocol_request_from_experiment_design, qualify_evidence, simulate_glioma_counterfactual,
    simulate_glioma_counterfactual_ensemble, simulate_glioma_protocol, surveil_glioma_evidence,
    validate_feature_catalog, ActionExecutionDisposition, ActionExecutionFailure,
    ActionExecutionResult, ActionPortfolioExecution, ActionPortfolioExecutionDisposition,
    ActionPortfolioExecutionError, ActionPortfolioExecutionRequest, ActionPortfolioStopReason,
    AdaptiveAllocation, AdaptiveAllocationActionKind, AdaptiveAllocationDisposition,
    AdaptiveAllocationError, AdaptiveAllocationRequest, AdaptiveArmObservation,
    AdaptiveArmPosterior, AdaptiveInformationCampaignDisposition, AdaptiveInformationCampaignError,
    AdaptiveInformationCampaignExecution, AdaptiveInformationCampaignPlan,
    AdaptiveInformationCampaignRequest, AdaptiveInformationCampaignRound,
    AdaptiveInformationCampaignTermination, AdaptiveInformationExecutionFailure,
    AdaptiveInformationObservation, AdaptiveMechanismPosterior, AnalysisDataset, AnalysisRequest,
    AnalysisResult, BatchHarmonizationDiagnostic, CalibrationDisposition, CalibrationError,
    CalibrationPoint, CalibrationRequest, CalibrationRun, CampaignAction, CampaignActionScore,
    CampaignExecutionFailure, CampaignExecutionRound, CampaignMechanism,
    CampaignMechanismPosterior, CampaignObservation, CampaignRound, CampaignStopReason,
    CatalogError, CausalContrastAnalysis, CausalContrastDisposition, CausalContrastError,
    CausalContrastRequest, CausalSensitivityAnalysis, CausalStratumSummary, ClosedLoopCampaign,
    ClosedLoopCampaignDisposition, ClosedLoopCampaignError, ClosedLoopCampaignExecution,
    ClosedLoopCampaignRequest, CombinationCell, CombinationCellDisposition, CombinationObservation,
    CombinationSynergyAnalysis, CombinationSynergyDisposition, CombinationSynergyError,
    CombinationSynergyRequest, ComputationCacheEntry, ComputationExecution,
    ComputationExecutionDisposition, ComputationExecutionError, ComputationExecutionFailure,
    ComputationExecutionRequest, ComputationExecutionStopReason, ComputationOperation,
    ComputationTask, ComputationTaskDisposition, ComputationTaskResult, ConcordanceDisposition,
    ConcordanceError, ConcordanceRequest, ConsensusAssignment, ConsensusCluster,
    ConsensusDisposition, ConsensusError, ConsensusRequest, CounterfactualContrast,
    CounterfactualDirection, CounterfactualDisposition, CounterfactualEnsembleRequest,
    CounterfactualError, CounterfactualIntervention, CounterfactualModel, CounterfactualRequest,
    DecisionAction, DecisionActionKind, DecisionActionPlan, DecisionActionPlanDisposition,
    DecisionActionPlanError, DecisionActionPlanRequest, DecisionContext,
    DecisionContextDisposition, DecisionContextError, DecisionContextRequest, DesignAction,
    DesignMechanism, DesignOutcome, DoseDirection, DosePair, DoseResponseAnalysis,
    DoseResponseDisposition, DoseResponseError, DoseResponseObservation, DoseResponsePoint,
    DoseResponseRequest, DryRunGliomaActionExecutor, DryRunGliomaComputationExecutor,
    DryRunGliomaProtocolExecutor, DryRunInstrumentExecutor, EnsembleCounterfactualError,
    EnsembleDirection, EnsembleDisposition, EnsembleModelResult, EnsembleTargetSummary,
    EstimateSource, EvidenceChange, EvidenceChangeKind, EvidencePriorityAction,
    EvidencePriorityActionKind, EvidencePriorityDisposition, EvidencePriorityError,
    EvidencePriorityPlan, EvidencePriorityRequest, EvidencePriorityWeights, EvidenceQualification,
    EvidenceRecord, EvidenceRequest, EvidenceSurveillance, EvidenceSurveillanceAction,
    EvidenceSurveillanceActionKind, EvidenceSurveillanceDisposition, EvidenceSurveillanceError,
    EvidenceSurveillanceRequest, ExperimentArm, ExperimentDesign, ExperimentRequest, FeatureValue,
    FederatedBenchmarkConsensus, FederatedBenchmarkContribution, FederatedBenchmarkDisposition,
    FederatedBenchmarkError, FederatedBenchmarkRequest, FederatedBenchmarkSite,
    FederatedBenchmarkSiteDisposition, FidelityCalibration, FidelityCandidate, FidelityEstimate,
    FidelityLevel, FidelityObservation, FrontierActionKind, GliomaActionExecutor,
    GliomaActionPlanner, GliomaAutonomousCampaign, GliomaAutonomousCampaignDisposition,
    GliomaAutonomousCampaignError, GliomaAutonomousCampaignRequest, GliomaAutonomousCampaignRound,
    GliomaAutonomousCampaignStopReason, GliomaAutonomousPlannerContext, GliomaCampaignExecutor,
    GliomaComputationExecutor, GliomaEvidenceCampaignDisposition, GliomaEvidenceCampaignError,
    GliomaEvidenceCampaignExecution, GliomaEvidenceCampaignRequest, GliomaFeatureSpec,
    GliomaInformationDesignExecutor, GliomaMechanismActionPlanner, GliomaOperatingScale,
    GliomaPlannerFailure, GliomaProgramDescriptor, GliomaProgramId, GliomaProtocolExecutor,
    GliomaResearchAutopilotDisposition, GliomaResearchAutopilotError,
    GliomaResearchAutopilotRequest, GliomaResearchAutopilotRun, GliomaWorkflowBranch,
    GliomaWorkflowError, GliomaWorkflowExecution, GliomaWorkflowMode, GliomaWorkflowNode,
    GliomaWorkflowPlan, GliomaWorkflowRequest, HarmonizationDisposition, HarmonizationError,
    HarmonizationRequest, HarmonizationVector, HarmonizedFeature, HarmonizedVector,
    InformationDesignActionScore, InformationDesignDisposition, InformationDesignError,
    InformationDesignPlan, InformationDesignRequest, InstrumentAction, InstrumentActionDecision,
    InstrumentActionDisposition, InstrumentAuthorization, InstrumentCalibration,
    InstrumentExecutionDisposition, InstrumentExecutionError, InstrumentExecutionFailure,
    InstrumentExecutionRequest, InstrumentExecutionResult, InstrumentExecutionRun,
    InstrumentExecutionStopReason, InstrumentExecutor, InstrumentInterlockSnapshot,
    InstrumentOperation, InstrumentParameter, InstrumentPreflightDisposition,
    InstrumentPreflightError, InstrumentPreflightPlan, InstrumentPreflightRequest, KnowledgeClaim,
    KnowledgeClaimDisposition, KnowledgeDisposition, KnowledgeError, KnowledgeFrontier,
    KnowledgeFrontierDisposition, KnowledgeFrontierError, KnowledgeFrontierRequest,
    KnowledgeFrontierScore, KnowledgeFrontierWeights, KnowledgeRequest, LatentFactorAnalysis,
    LatentFactorComponent, LatentFactorDisposition, LatentFactorError, LatentFactorRequest,
    LatentFactorVector, LatentLoading, LatentScore, LigandReceptorPair, MechanismActionPlan,
    MechanismActionPlannerConfig, MechanismActionPlannerError, MechanismCandidate,
    MechanismCounterfactual, MechanismCounterfactualEnsemble, MechanismDiscrimination,
    MechanismDiscriminationDisposition, MechanismDiscriminationError,
    MechanismDiscriminationRanking, MechanismDiscriminationRequest, MechanismDiscriminatorAction,
    MechanismFeatureObservation, MechanismGraphDisposition, MechanismGraphEdge,
    MechanismGraphError, MechanismGraphNode, MechanismGraphPropagation, MechanismGraphRelation,
    MechanismGraphRequest, MechanismHypothesis, MechanismInformationGain, MechanismNodeScore,
    MechanismPortfolio, MechanismPrediction, MechanismRequest, MediationAnalysis,
    MediationDisposition, MediationError, MediationObservation, MediationRequest,
    MetaAnalysisDisposition, MetaAnalysisError, MetaAnalysisRequest, MetaStudyContribution,
    ModalityConcordance, ModalityVector, MultiFidelityDisposition, MultiFidelityOptimizationError,
    MultiFidelityOptimizationPlan, MultiFidelityOptimizationRequest, MultimodalConcordance,
    MultimodalConsensus, MultimodalHarmonization, MultimodalObservation, MultimodalQcReport,
    MultimodalRequest, OptimizationDirection, PairConcordanceDisposition, PortfolioDirection,
    ProtocolDisposition, ProtocolExecution, ProtocolExecutionDisposition, ProtocolExecutionError,
    ProtocolExecutionFailure, ProtocolExecutionRequest, ProtocolExecutionStopReason,
    ProtocolResource, ProtocolResourceKind, ProtocolSimulation, ProtocolSimulationError,
    ProtocolSimulationRequest, ProtocolTask, ProtocolTaskDisposition, ProtocolTaskResult,
    ReplicationAssessment, ReplicationMetaAnalysis, ReplicationRequest, ReplicationStudy,
    ResearchObjectManifest, ResearchObjectRequest, ResourceUtilization,
    RobustInterventionCandidate, RobustInterventionPortfolio, RobustInterventionRequest,
    RobustInterventionScore, RobustPortfolioDisposition, RobustPortfolioError, RobustnessCase,
    RobustnessCaseKind, RobustnessDisposition, RobustnessError, RobustnessRequest, RobustnessSuite,
    ScheduleEntry, SensitivityDirection, SensitivityDisposition, SensitivityError,
    SensitivityObservation, SensitivityPoint, SensitivityRequest, SpatialCell,
    SpatialCommunicationAnalysis, SpatialCommunicationCell, SpatialCommunicationDisposition,
    SpatialCommunicationError, SpatialCommunicationPair, SpatialCommunicationPairDisposition,
    SpatialCommunicationRequest, SpatialNiche, SpatialNicheAnalysis, SpatialNicheDisposition,
    SpatialNicheError, SpatialNicheInteraction, SpatialNicheRequest, SpatialPropagationAnalysis,
    SpatialPropagationDisposition, SpatialPropagationEdge, SpatialPropagationError,
    SpatialPropagationRequest, SpatialPropagationTrajectory, StateTransitionAnalysis,
    StateTransitionCell, StateTransitionContrast, StateTransitionDisposition, StateTransitionError,
    StateTransitionObservation, StateTransitionRequest, StaticGliomaActionPlanner,
    StratifiedCausalActionKind, StratifiedCausalAdjustment, StratifiedCausalDisposition,
    StratifiedCausalError, StratifiedCausalRequest, StratifiedObservation, TrajectoryAnalysis,
    TrajectoryArmSummary, TrajectoryDisposition, TrajectoryError, TrajectoryObservation,
    TrajectoryRequest, TransitionCellDisposition, TransitionContrastDisposition,
    TransitionDirection, TypedKnowledge, UnitContrast, UnitTrajectory, UnitTrajectoryDisposition,
    WorkflowNodeDecision, EXECUTION_OUTPUT_SCHEMA, PROTOCOL_EXECUTION_OUTPUT_SCHEMA,
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
