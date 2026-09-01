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
    surveil_glioma_evidence, EvidenceChange, EvidenceChangeKind, EvidenceSurveillance,
    EvidenceSurveillanceAction, EvidenceSurveillanceActionKind, EvidenceSurveillanceDisposition,
    EvidenceSurveillanceError, EvidenceSurveillanceRequest,
};
pub use programs::p02_evidence_knowledge::{
    compile_typed_knowledge, KnowledgeClaim, KnowledgeClaimDisposition, KnowledgeDisposition,
    KnowledgeError, KnowledgeRequest, TypedKnowledge,
};
pub use programs::p03_multimodal_ingestion_qc::{
    analyze_glioma_latent_factors, analyze_glioma_spatial_niches, analyze_multimodal_concordance,
    analyze_multimodal_consensus, harmonize_glioma_multimodal_batches,
    BatchHarmonizationDiagnostic, ConcordanceDisposition, ConcordanceError, ConcordanceRequest,
    ConsensusAssignment, ConsensusCluster, ConsensusDisposition, ConsensusError, ConsensusRequest,
    FeatureValue, HarmonizationDisposition, HarmonizationError, HarmonizationRequest,
    HarmonizationVector, HarmonizedFeature, HarmonizedVector, LatentFactorAnalysis,
    LatentFactorComponent, LatentFactorDisposition, LatentFactorError, LatentFactorRequest,
    LatentFactorVector, LatentLoading, LatentScore, ModalityConcordance, ModalityVector,
    MultimodalConcordance, MultimodalConsensus, MultimodalHarmonization,
    PairConcordanceDisposition, SpatialCell, SpatialNiche, SpatialNicheAnalysis,
    SpatialNicheDisposition, SpatialNicheError, SpatialNicheInteraction, SpatialNicheRequest,
};
pub use programs::p04_decision_context::{
    compile_decision_context, DecisionAction, DecisionActionKind, DecisionContext,
    DecisionContextDisposition, DecisionContextError, DecisionContextRequest,
};
pub use programs::p05_mechanism_exploration::{
    discriminate_mechanisms, propagate_glioma_mechanism_graph, MechanismDiscrimination,
    MechanismDiscriminationDisposition, MechanismDiscriminationError,
    MechanismDiscriminationRanking, MechanismDiscriminationRequest, MechanismDiscriminatorAction,
    MechanismFeatureObservation, MechanismGraphDisposition, MechanismGraphEdge,
    MechanismGraphError, MechanismGraphNode, MechanismGraphPropagation, MechanismGraphRelation,
    MechanismGraphRequest, MechanismHypothesis, MechanismInformationGain, MechanismNodeScore,
    MechanismPrediction,
};
pub use programs::p06_experiment_design::{
    allocate_glioma_assays, analyze_glioma_combination_synergy, analyze_glioma_dose_response,
    execute_glioma_closed_loop_campaign, plan_glioma_closed_loop_campaign, AdaptiveAllocation,
    AdaptiveAllocationActionKind, AdaptiveAllocationDisposition, AdaptiveAllocationError,
    AdaptiveAllocationRequest, AdaptiveArmObservation, AdaptiveArmPosterior, CampaignAction,
    CampaignActionScore, CampaignExecutionFailure, CampaignExecutionRound, CampaignMechanism,
    CampaignMechanismPosterior, CampaignObservation, CampaignRound, CampaignStopReason,
    ClosedLoopCampaign, ClosedLoopCampaignDisposition, ClosedLoopCampaignError,
    ClosedLoopCampaignExecution, ClosedLoopCampaignRequest, CombinationCell,
    CombinationCellDisposition, CombinationObservation, CombinationSynergyAnalysis,
    CombinationSynergyDisposition, CombinationSynergyError, CombinationSynergyRequest,
    DoseDirection, DosePair, DoseResponseAnalysis, DoseResponseDisposition, DoseResponseError,
    DoseResponseObservation, DoseResponsePoint, DoseResponseRequest, GliomaCampaignExecutor,
    EXECUTION_OUTPUT_SCHEMA,
};
pub use programs::p07_protocol_simulation::{
    protocol_request_from_experiment_design, simulate_glioma_protocol, ProtocolDisposition,
    ProtocolResource, ProtocolResourceKind, ProtocolSimulation, ProtocolSimulationError,
    ProtocolSimulationRequest, ProtocolTask, ResourceUtilization, ScheduleEntry,
};
pub use programs::p08_instrument_robotics::{
    analyze_instrument_calibration, CalibrationDisposition, CalibrationError, CalibrationPoint,
    CalibrationRequest, CalibrationRun, InstrumentCalibration,
};
pub use programs::p09_reproducible_computation::{
    assess_glioma_robustness, RobustnessCase, RobustnessCaseKind, RobustnessDisposition,
    RobustnessError, RobustnessRequest, RobustnessSuite,
};
pub use programs::p10_interpretation_replication::{
    analyze_causal_sensitivity, analyze_glioma_causal_contrast, analyze_glioma_trajectories,
    analyze_replication_meta_analysis, analyze_stratified_causal_adjustment,
    CausalContrastAnalysis, CausalContrastDisposition, CausalContrastError, CausalContrastRequest,
    CausalSensitivityAnalysis, CausalStratumSummary, MetaAnalysisDisposition, MetaAnalysisError,
    MetaAnalysisRequest, MetaStudyContribution, ReplicationMetaAnalysis, SensitivityDirection,
    SensitivityDisposition, SensitivityError, SensitivityObservation, SensitivityPoint,
    SensitivityRequest, StratifiedCausalActionKind, StratifiedCausalAdjustment,
    StratifiedCausalDisposition, StratifiedCausalError, StratifiedCausalRequest,
    StratifiedObservation, TrajectoryAnalysis, TrajectoryArmSummary, TrajectoryDisposition,
    TrajectoryError, TrajectoryObservation, TrajectoryRequest, UnitContrast, UnitTrajectory,
    UnitTrajectoryDisposition,
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
