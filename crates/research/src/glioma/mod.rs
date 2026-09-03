//! Organized glioma product programs.
//!
//! [`crate::glioma_engine`] owns the cross-program execution graph and its provider seam.  This
//! module owns the product programs themselves: each submodule is a typed, deterministic
//! capability that can be called by a local executor, an MCP adapter, a notebook, or a batch
//! worker.  The modules intentionally exchange hashes and de-identified metadata rather than raw
//! specimen bytes.

pub mod analysis;
pub mod catalog;
pub mod evidence;
pub mod experiment;
pub mod mechanism;
pub mod multimodal;
pub mod programs;
pub mod release;
pub mod replication;
pub mod workflow;

pub use analysis::{
    analyze_preclinical_outcomes, AnalysisDataset, AnalysisRequest, AnalysisResult,
};
pub use catalog::{
    generate_feature_catalog, glioma_program_catalog, implemented_feature_ids,
    validate_feature_catalog, CatalogError, GliomaFeatureSpec, GliomaOperatingScale,
    GliomaProgramDescriptor, GliomaProgramId,
};
pub use evidence::{qualify_evidence, EvidenceQualification, EvidenceRecord, EvidenceRequest};
pub use experiment::{
    design_preclinical_experiment, ExperimentArm, ExperimentDesign, ExperimentRequest,
};
pub use mechanism::{explore_mechanisms, MechanismCandidate, MechanismPortfolio, MechanismRequest};
pub use multimodal::{
    harmonize_multimodal_inputs, MultimodalObservation, MultimodalQcReport, MultimodalRequest,
};
pub use programs::p01_evidence_surveillance::{
    prioritize_glioma_evidence, surveil_glioma_evidence, EvidenceChange, EvidenceChangeKind,
    EvidencePriorityAction, EvidencePriorityActionKind, EvidencePriorityDisposition,
    EvidencePriorityError, EvidencePriorityPlan, EvidencePriorityRequest, EvidencePriorityWeights,
    EvidenceSurveillance, EvidenceSurveillanceAction, EvidenceSurveillanceActionKind,
    EvidenceSurveillanceDisposition, EvidenceSurveillanceError, EvidenceSurveillanceRequest,
};
pub use programs::p02_evidence_knowledge::{
    compile_typed_knowledge, prioritize_knowledge_frontier, FrontierActionKind, KnowledgeClaim,
    KnowledgeClaimDisposition, KnowledgeDisposition, KnowledgeError, KnowledgeFrontier,
    KnowledgeFrontierDisposition, KnowledgeFrontierError, KnowledgeFrontierRequest,
    KnowledgeFrontierScore, KnowledgeFrontierWeights, KnowledgeRequest, TypedKnowledge,
};
pub use programs::p03_multimodal_ingestion_qc::{
    analyze_glioma_latent_factors, analyze_glioma_spatial_communication,
    analyze_glioma_spatial_niches, analyze_glioma_spatial_state_propagation,
    analyze_multimodal_concordance, analyze_multimodal_consensus,
    harmonize_glioma_multimodal_batches, BatchHarmonizationDiagnostic, ConcordanceDisposition,
    ConcordanceError, ConcordanceRequest, ConsensusAssignment, ConsensusCluster,
    ConsensusDisposition, ConsensusError, ConsensusRequest, FeatureValue, HarmonizationDisposition,
    HarmonizationError, HarmonizationRequest, HarmonizationVector, HarmonizedFeature,
    HarmonizedVector, LatentFactorAnalysis, LatentFactorComponent, LatentFactorDisposition,
    LatentFactorError, LatentFactorRequest, LatentFactorVector, LatentLoading, LatentScore,
    LigandReceptorPair, ModalityConcordance, ModalityVector, MultimodalConcordance,
    MultimodalConsensus, MultimodalHarmonization, PairConcordanceDisposition, SpatialCell,
    SpatialCommunicationAnalysis, SpatialCommunicationCell, SpatialCommunicationDisposition,
    SpatialCommunicationError, SpatialCommunicationPair, SpatialCommunicationPairDisposition,
    SpatialCommunicationRequest, SpatialNiche, SpatialNicheAnalysis, SpatialNicheDisposition,
    SpatialNicheError, SpatialNicheInteraction, SpatialNicheRequest, SpatialPropagationAnalysis,
    SpatialPropagationDisposition, SpatialPropagationEdge, SpatialPropagationError,
    SpatialPropagationRequest, SpatialPropagationTrajectory,
};
pub use programs::p04_decision_context::{
    compile_decision_context, plan_decision_actions, DecisionAction, DecisionActionKind,
    DecisionActionPlan, DecisionActionPlanDisposition, DecisionActionPlanError,
    DecisionActionPlanRequest, DecisionContext, DecisionContextDisposition, DecisionContextError,
    DecisionContextRequest,
};
pub use programs::p05_mechanism_exploration::{
    compile_mechanism_action_plan, discriminate_mechanisms,
    plan_glioma_robust_intervention_portfolio, propagate_glioma_mechanism_graph,
    simulate_glioma_counterfactual, simulate_glioma_counterfactual_ensemble,
    CounterfactualContrast, CounterfactualDirection, CounterfactualDisposition,
    CounterfactualEnsembleRequest, CounterfactualError, CounterfactualIntervention,
    CounterfactualModel, CounterfactualRequest, EnsembleCounterfactualError, EnsembleDirection,
    EnsembleDisposition, EnsembleModelResult, EnsembleTargetSummary, GliomaMechanismActionPlanner,
    MechanismActionPlan, MechanismActionPlannerConfig, MechanismActionPlannerError,
    MechanismCounterfactual, MechanismCounterfactualEnsemble, MechanismDiscrimination,
    MechanismDiscriminationDisposition, MechanismDiscriminationError,
    MechanismDiscriminationRanking, MechanismDiscriminationRequest, MechanismDiscriminatorAction,
    MechanismFeatureObservation, MechanismGraphDisposition, MechanismGraphEdge,
    MechanismGraphError, MechanismGraphNode, MechanismGraphPropagation, MechanismGraphRelation,
    MechanismGraphRequest, MechanismHypothesis, MechanismInformationGain, MechanismNodeScore,
    MechanismPrediction, PortfolioDirection, RobustInterventionCandidate,
    RobustInterventionPortfolio, RobustInterventionRequest, RobustInterventionScore,
    RobustPortfolioDisposition, RobustPortfolioError,
};
pub use programs::p06_experiment_design::{
    allocate_glioma_assays, analyze_glioma_combination_synergy, analyze_glioma_dose_response,
    execute_glioma_adaptive_information_campaign, execute_glioma_closed_loop_campaign,
    plan_glioma_adaptive_information_campaign, plan_glioma_closed_loop_campaign,
    plan_glioma_information_design, plan_glioma_multi_fidelity_optimization, AdaptiveAllocation,
    AdaptiveAllocationActionKind, AdaptiveAllocationDisposition, AdaptiveAllocationError,
    AdaptiveAllocationRequest, AdaptiveArmObservation, AdaptiveArmPosterior,
    AdaptiveInformationCampaignDisposition, AdaptiveInformationCampaignError,
    AdaptiveInformationCampaignExecution, AdaptiveInformationCampaignPlan,
    AdaptiveInformationCampaignRequest, AdaptiveInformationCampaignRound,
    AdaptiveInformationCampaignTermination, AdaptiveInformationExecutionFailure,
    AdaptiveInformationObservation, AdaptiveMechanismPosterior, CampaignAction,
    CampaignActionScore, CampaignExecutionFailure, CampaignExecutionRound, CampaignMechanism,
    CampaignMechanismPosterior, CampaignObservation, CampaignRound, CampaignStopReason,
    ClosedLoopCampaign, ClosedLoopCampaignDisposition, ClosedLoopCampaignError,
    ClosedLoopCampaignExecution, ClosedLoopCampaignRequest, CombinationCell,
    CombinationCellDisposition, CombinationObservation, CombinationSynergyAnalysis,
    CombinationSynergyDisposition, CombinationSynergyError, CombinationSynergyRequest,
    DesignAction, DesignMechanism, DesignOutcome, DoseDirection, DosePair, DoseResponseAnalysis,
    DoseResponseDisposition, DoseResponseError, DoseResponseObservation, DoseResponsePoint,
    DoseResponseRequest, EstimateSource, FidelityCalibration, FidelityCandidate, FidelityEstimate,
    FidelityLevel, FidelityObservation, GliomaCampaignExecutor, GliomaInformationDesignExecutor,
    InformationDesignActionScore, InformationDesignDisposition, InformationDesignError,
    InformationDesignPlan, InformationDesignRequest, MultiFidelityDisposition,
    MultiFidelityOptimizationError, MultiFidelityOptimizationPlan,
    MultiFidelityOptimizationRequest, OptimizationDirection, EXECUTION_OUTPUT_SCHEMA,
};
pub use programs::p07_protocol_simulation::{
    execute_glioma_action_portfolio, execute_glioma_autonomous_campaign, execute_glioma_protocol,
    execute_glioma_research_autopilot, protocol_request_from_experiment_design,
    simulate_glioma_protocol, ActionExecutionDisposition, ActionExecutionFailure,
    ActionExecutionResult, ActionPortfolioExecution, ActionPortfolioExecutionDisposition,
    ActionPortfolioExecutionError, ActionPortfolioExecutionRequest, ActionPortfolioStopReason,
    DryRunGliomaActionExecutor, DryRunGliomaProtocolExecutor, GliomaActionExecutor,
    GliomaActionPlanner, GliomaAutonomousCampaign, GliomaAutonomousCampaignDisposition,
    GliomaAutonomousCampaignError, GliomaAutonomousCampaignRequest, GliomaAutonomousCampaignRound,
    GliomaAutonomousCampaignStopReason, GliomaAutonomousPlannerContext, GliomaPlannerFailure,
    GliomaProtocolExecutor, GliomaResearchAutopilotDisposition, GliomaResearchAutopilotError,
    GliomaResearchAutopilotRequest, GliomaResearchAutopilotRun, ProtocolDisposition,
    ProtocolExecution, ProtocolExecutionDisposition, ProtocolExecutionError,
    ProtocolExecutionFailure, ProtocolExecutionRequest, ProtocolExecutionStopReason,
    ProtocolResource, ProtocolResourceKind, ProtocolSimulation, ProtocolSimulationError,
    ProtocolSimulationRequest, ProtocolTask, ProtocolTaskDisposition, ProtocolTaskResult,
    ResourceUtilization, ScheduleEntry, StaticGliomaActionPlanner,
    PROTOCOL_EXECUTION_OUTPUT_SCHEMA,
};
pub use programs::p08_instrument_robotics::{
    analyze_instrument_calibration, preflight_glioma_instrument, CalibrationDisposition,
    CalibrationError, CalibrationPoint, CalibrationRequest, CalibrationRun, InstrumentAction,
    InstrumentActionDecision, InstrumentActionDisposition, InstrumentAuthorization,
    InstrumentCalibration, InstrumentInterlockSnapshot, InstrumentOperation, InstrumentParameter,
    InstrumentPreflightDisposition, InstrumentPreflightError, InstrumentPreflightPlan,
    InstrumentPreflightRequest,
};
pub use programs::p09_reproducible_computation::{
    assess_glioma_robustness, execute_glioma_computation, ComputationCacheEntry,
    ComputationExecution, ComputationExecutionDisposition, ComputationExecutionError,
    ComputationExecutionFailure, ComputationExecutionRequest, ComputationExecutionStopReason,
    ComputationOperation, ComputationTask, ComputationTaskDisposition, ComputationTaskResult,
    DryRunGliomaComputationExecutor, GliomaComputationExecutor, RobustnessCase, RobustnessCaseKind,
    RobustnessDisposition, RobustnessError, RobustnessRequest, RobustnessSuite,
};
pub use programs::p10_interpretation_replication::{
    analyze_causal_sensitivity, analyze_glioma_causal_contrast, analyze_glioma_mediation,
    analyze_glioma_state_transitions, analyze_glioma_trajectories,
    analyze_replication_meta_analysis, analyze_stratified_causal_adjustment,
    CausalContrastAnalysis, CausalContrastDisposition, CausalContrastError, CausalContrastRequest,
    CausalSensitivityAnalysis, CausalStratumSummary, MediationAnalysis, MediationDisposition,
    MediationError, MediationObservation, MediationRequest, MetaAnalysisDisposition,
    MetaAnalysisError, MetaAnalysisRequest, MetaStudyContribution, ReplicationMetaAnalysis,
    SensitivityDirection, SensitivityDisposition, SensitivityError, SensitivityObservation,
    SensitivityPoint, SensitivityRequest, StateTransitionAnalysis, StateTransitionCell,
    StateTransitionContrast, StateTransitionDisposition, StateTransitionError,
    StateTransitionObservation, StateTransitionRequest, StratifiedCausalActionKind,
    StratifiedCausalAdjustment, StratifiedCausalDisposition, StratifiedCausalError,
    StratifiedCausalRequest, StratifiedObservation, TrajectoryAnalysis, TrajectoryArmSummary,
    TrajectoryDisposition, TrajectoryError, TrajectoryObservation, TrajectoryRequest,
    TransitionCellDisposition, TransitionContrastDisposition, TransitionDirection, UnitContrast,
    UnitTrajectory, UnitTrajectoryDisposition,
};
pub use programs::p12_federated_benchmarking::{
    analyze_federated_benchmark, FederatedBenchmarkConsensus, FederatedBenchmarkContribution,
    FederatedBenchmarkDisposition, FederatedBenchmarkError, FederatedBenchmarkRequest,
    FederatedBenchmarkSite, FederatedBenchmarkSiteDisposition,
};
pub use release::{build_research_object_manifest, ResearchObjectManifest, ResearchObjectRequest};
pub use replication::{
    assess_replication, ReplicationAssessment, ReplicationRequest, ReplicationStudy,
};
pub use workflow::{
    execute_glioma_workflow, plan_glioma_workflow, GliomaWorkflowBranch, GliomaWorkflowError,
    GliomaWorkflowExecution, GliomaWorkflowMode, GliomaWorkflowNode, GliomaWorkflowPlan,
    GliomaWorkflowRequest, WorkflowNodeDecision,
};
