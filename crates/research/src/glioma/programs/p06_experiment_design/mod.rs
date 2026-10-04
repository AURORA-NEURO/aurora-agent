//! Power-aware experiment-design program ownership.

use crate::glioma::catalog::{GliomaProgramDescriptor, GliomaProgramId, glioma_program_catalog};

pub mod active_learning;
pub mod adaptive_allocation;
pub mod adaptive_allocation_campaign;
pub mod adaptive_dose_surface;
pub mod adaptive_information_campaign;
pub mod adaptive_panel;
pub mod blocked_randomization;
pub mod campaign;
pub mod carryover_sequence;
pub mod clonal_panel;
pub mod contrast_design;
pub mod dose_response;
pub mod frontier_controller;
pub mod heterogeneity_aware_portfolio;
pub mod information_design;
pub mod lineage_acquisition_design;
pub mod lineage_guided_campaign;
pub mod lineage_response_calibration;
pub mod mechanism_validation;
pub mod mechanism_validation_protocol;
pub mod multi_fidelity;
pub mod multi_fidelity_campaign;
pub mod operating_cycle;
pub mod posterior_batch;
pub mod power_reestimation;
pub mod power_stress_surface;
pub mod replication_continuation;
pub mod replication_plan;
pub mod replication_protocol;
pub mod robust_active_learning;
pub mod robust_design;
pub mod sequential_campaign;
pub mod sequential_design;
pub mod simulation_gated_campaign;
pub mod state_plasticity_instrument_campaign;
pub mod state_stratified_campaign;
pub mod synergy;
pub mod transition_guided_campaign;
pub mod validation_batch_assessment;
pub mod validation_campaign;
pub use active_learning::{
    ActiveLearningCandidate,
    ActiveLearningCandidateDisposition,
    ActiveLearningDirection,
    ActiveLearningDisposition,
    ActiveLearningError,
    ActiveLearningObservation,
    ActiveLearningPlan,
    ActiveLearningRequest,
    ActiveLearningScore,
    plan_glioma_active_learning,
    evaluate_glioma_active_learning,
    ActiveLearningEvaluation,
    ActiveLearningEvaluationError,
    ActiveLearningEvaluationMetric,
    ActiveLearningEvaluationPolicy,
};
pub use adaptive_allocation::{
    AdaptiveAllocation,
    AdaptiveAllocationActionKind,
    AdaptiveAllocationDisposition,
    AdaptiveAllocationError,
    AdaptiveAllocationRequest,
    AdaptiveArmObservation,
    AdaptiveArmPosterior,
    allocate_glioma_assays,
};
pub use adaptive_allocation_campaign::{
    AdaptiveAllocationBatchObservation,
    AdaptiveAllocationCampaign,
    AdaptiveAllocationCampaignDisposition,
    AdaptiveAllocationCampaignError,
    AdaptiveAllocationCampaignExecutionFailure,
    AdaptiveAllocationCampaignExecutor,
    AdaptiveAllocationCampaignRequest,
    AdaptiveAllocationCampaignRound,
    AdaptiveAllocationCampaignStopReason,
    DryRunAdaptiveAllocationCampaignExecutor,
    execute_glioma_adaptive_allocation_campaign,
};
pub use adaptive_dose_surface::{
    AdaptiveDoseSurfaceDisposition,
    AdaptiveDoseSurfaceError,
    AdaptiveDoseSurfacePlan,
    AdaptiveDoseSurfaceRequest,
    DoseSurfaceCellState,
    DoseSurfaceEstimate,
    DoseSurfaceObservation,
    plan_adaptive_glioma_dose_surface,
};
pub use adaptive_information_campaign::{
    AdaptiveInformationCampaignDisposition,
    AdaptiveInformationCampaignError,
    AdaptiveInformationCampaignExecution,
    AdaptiveInformationCampaignPlan,
    AdaptiveInformationCampaignRequest,
    AdaptiveInformationCampaignRound,
    AdaptiveInformationCampaignTermination,
    AdaptiveInformationExecutionFailure,
    AdaptiveInformationObservation,
    AdaptiveMechanismPosterior,
    GliomaInformationDesignExecutor,
    execute_glioma_adaptive_information_campaign,
    plan_glioma_adaptive_information_campaign,
};
pub use adaptive_panel::{
    AdaptivePanelActionKind,
    AdaptivePanelDesign,
    AdaptivePanelDisposition,
    AdaptivePanelError,
    AdaptivePanelRequest,
    AdaptivePanelSelection,
    PanelAction,
    PanelMechanism,
    PanelOutcome,
    plan_glioma_adaptive_panel,
};
pub use blocked_randomization::{
    BlockArmAllocation,
    BlockedRandomizationDesign,
    BlockedRandomizationDisposition,
    BlockedRandomizationError,
    BlockedRandomizationRequest,
    RandomizationArm,
    RandomizationBlock,
    plan_glioma_blocked_randomization,
};
pub use campaign::{
    CampaignAction,
    CampaignActionScore,
    CampaignExecutionFailure,
    CampaignExecutionRound,
    CampaignMechanism,
    CampaignMechanismPosterior,
    CampaignObservation,
    CampaignRound,
    CampaignStopReason,
    ClosedLoopCampaign,
    ClosedLoopCampaignDisposition,
    ClosedLoopCampaignError,
    ClosedLoopCampaignExecution,
    ClosedLoopCampaignRequest,
    EXECUTION_OUTPUT_SCHEMA,
    GliomaCampaignExecutor,
    execute_glioma_closed_loop_campaign,
    plan_glioma_closed_loop_campaign,
};
pub use carryover_sequence::{
    CarryoverAction,
    CarryoverSequenceDesign,
    CarryoverSequenceDisposition,
    CarryoverSequenceError,
    CarryoverSequenceRequest,
    CarryoverSequenceStep,
    plan_glioma_carryover_sequence,
};
pub use clonal_panel::{
    CloneBranchCoverage,
    ClonePerturbationCandidate,
    ClonePerturbationDecision,
    ClonePerturbationKind,
    ClonePerturbationPanel,
    ClonePerturbationPanelDisposition,
    ClonePerturbationPanelError,
    ClonePerturbationPanelRequest,
    plan_glioma_clone_perturbation_panel,
};
pub use contrast_design::{
    ContrastCondition,
    ContrastDesign,
    ContrastDesignDisposition,
    ContrastDesignError,
    ContrastDesignRequest,
    ContrastFactor,
    EstimandContrast,
    design_glioma_contrast_panel,
};
pub use dose_response::{
    DoseDirection,
    DoseResponseAnalysis,
    DoseResponseDisposition,
    DoseResponseError,
    DoseResponseObservation,
    DoseResponsePoint,
    DoseResponseRequest,
    analyze_glioma_dose_response,
};
pub use frontier_controller::{
    DryRunGliomaExperimentFrontierExecutor,
    GliomaExperimentFrontierError,
    GliomaExperimentFrontierExecutor,
    GliomaExperimentFrontierRequest,
    GliomaExperimentFrontierRun,
    GliomaFrontierCandidate,
    GliomaFrontierDisposition,
    GliomaFrontierExecutionFailure,
    GliomaFrontierMechanism,
    GliomaFrontierObservation,
    GliomaFrontierOutcome,
    GliomaFrontierRound,
    GliomaFrontierScore,
    GliomaFrontierStopReason,
    execute_glioma_experiment_frontier_controller,
};
pub use heterogeneity_aware_portfolio::{
    plan_glioma_heterogeneity_aware_experiment_portfolio,
    HeterogeneityAwareExperimentPortfolio,
    HeterogeneityAwareExperimentPortfolioError,
    HeterogeneityAwareExperimentPortfolioRequest,
    HeterogeneityExperimentCandidate,
    HeterogeneityExperimentStratum,
    HeterogeneityPortfolioDeferral,
    HeterogeneityPortfolioDisposition,
    HeterogeneityPortfolioSelection,
    HeterogeneityPowerStressPoint,
};
pub use information_design::{
    DesignAction,
    DesignMechanism,
    DesignOutcome,
    InformationDesignActionScore,
    InformationDesignDisposition,
    InformationDesignError,
    InformationDesignPlan,
    InformationDesignRequest,
    plan_glioma_information_design,
    plan_glioma_information_design_with_objective,
    InformationAcquisitionObjective,
};
pub use multi_fidelity::{
    EstimateSource,
    FidelityCalibration,
    FidelityCandidate,
    FidelityEstimate,
    FidelityLevel,
    FidelityObservation,
    MultiFidelityDisposition,
    MultiFidelityOptimizationError,
    MultiFidelityOptimizationPlan,
    MultiFidelityOptimizationRequest,
    OptimizationDirection,
    plan_glioma_multi_fidelity_optimization,
};
pub use multi_fidelity_campaign::{
    DryRunMultiFidelityCampaignExecutor,
    MultiFidelityCampaign,
    MultiFidelityCampaignDisposition,
    MultiFidelityCampaignError,
    MultiFidelityCampaignExecutor,
    MultiFidelityCampaignRequest,
    MultiFidelityCampaignRound,
    MultiFidelityCampaignStopReason,
    MultiFidelityExecutionFailure,
    execute_glioma_multi_fidelity_campaign,
};
pub use posterior_batch::{
    PosteriorBatchCandidate,
    PosteriorBatchCandidateDisposition,
    PosteriorBatchDisposition,
    PosteriorBatchError,
    PosteriorBatchPlan,
    PosteriorBatchRequest,
    PosteriorBatchScore,
    PosteriorBatchTarget,
    PosteriorPredictiveDraw,
    plan_glioma_posterior_batch,
};
pub use mechanism_validation::{
    MechanismValidationArm,
    MechanismValidationDisposition,
    MechanismValidationError,
    MechanismValidationPlan,
    MechanismValidationPlanRequest,
    ValidationAction,
    ValidationActionKind,
    ValidationArmRole,
    plan_glioma_mechanism_validation,
};
pub use mechanism_validation_protocol::{
    MechanismValidationProtocolCompilation,
    MechanismValidationProtocolCompileRequest,
    MechanismValidationProtocolDisposition,
    MechanismValidationProtocolError,
    compile_glioma_mechanism_validation_protocol,
};
pub use operating_cycle::{
    DryRunExperimentOperatingCycleExecutor,
    ExperimentOperatingCycle,
    ExperimentOperatingCycleDisposition,
    ExperimentOperatingCycleError,
    ExperimentOperatingCycleRequest,
    execute_glioma_experiment_operating_cycle,
};
pub use power_reestimation::{
    PowerArmDecision,
    PowerArmObservation,
    PowerDecisionKind,
    PowerReestimationDisposition,
    PowerReestimationError,
    PowerReestimationPlan,
    PowerReestimationRequest,
    plan_glioma_power_reestimation,
};
pub use power_stress_surface::{
    PowerStressArm,
    PowerStressArmResult,
    PowerStressDisposition,
    PowerStressError,
    PowerStressScenario,
    PowerStressScenarioResult,
    PowerStressSurface,
    PowerStressSurfaceRequest,
    plan_glioma_power_stress_surface,
};
pub use validation_batch_assessment::{
    ValidationBatchAssessment,
    ValidationBatchAssessmentDisposition,
    ValidationBatchAssessmentError,
    ValidationBatchAssessmentRequest,
    assess_glioma_validation_batch,
};
pub use validation_campaign::{
    ValidationCampaignDisposition,
    ValidationCampaignError,
    ValidationCampaignRequest,
    ValidationCampaignRound,
    ValidationCampaignRun,
    ValidationCampaignStopReason,
    execute_glioma_validation_campaign,
};
pub use replication_plan::{
    ReplicationObservation,
    ReplicationPlan,
    ReplicationPlanDisposition,
    ReplicationPlanError,
    ReplicationPlanRequest,
    ReplicationSiteAction,
    ReplicationSitePlan,
    plan_glioma_replication,
};
pub use lineage_acquisition_design::{
    plan_glioma_lineage_assay_acquisition,
    plan_glioma_lineage_assay_acquisition_for_target,
    update_lineage_propagation_particle_weights,
    LineagePropagationAcquisitionError,
    LineagePropagationAcquisitionPlan,
    LineagePropagationAcquisitionPolicy,
    LineagePropagationAcquisitionTarget,
    LineagePropagationAssayResponseModel,
    LineagePropagationAssayScore,
    LineagePropagationJointAssayResponseModel,
    LineagePropagationJointOutcomeLikelihood,
    LineagePropagationObservedAssay,
    LineagePropagationOutcomeLikelihood,
    LineagePropagationOutcomeProbability,
    LineagePropagationResponseDependence,
    MIN_POSTERIOR_EFFECTIVE_SAMPLE_FRACTION_MILLI,
};
pub use lineage_guided_campaign::{
    execute_glioma_lineage_guided_assay_workflow,
    execute_glioma_lineage_guided_instrument_campaign,
    plan_glioma_lineage_guided_state_priorities,
    LineageGuidanceWeights,
    LineageGuidedAssayWorkflowError,
    LineageGuidedAssayWorkflowRequest,
    LineageGuidedAssayWorkflowRun,
    LineageGuidedInstrumentCampaignRun,
    LineageGuidedStatePriority,
};
pub use lineage_response_calibration::{
    calibrate_glioma_lineage_assay_response_model,
    calibrate_glioma_lineage_joint_assay_response_model,
    LineageAssayCalibrationObservation,
    LineageAssayJointCalibrationObservation,
    LineageAssayJointResponseCalibrationDiagnostics,
    LineageAssayJointResponseCalibrationDisposition,
    LineageAssayJointResponseCalibrationRequest,
    LineageAssayJointResponseCalibrationRun,
    LineageAssayResponseCalibrationDiagnostics,
    LineageAssayResponseCalibrationDisposition,
    LineageAssayResponseCalibrationError,
    LineageAssayResponseCalibrationRequest,
    LineageAssayResponseCalibrationRun,
    DEFAULT_MINIMUM_BRIER_SKILL_PPM,
    JOINT_OUTPUT_SCHEMA,
};
pub use replication_continuation::{
    ReplicationContinuationAction,
    ReplicationContinuationDisposition,
    ReplicationContinuationError,
    ReplicationContinuationObservation,
    ReplicationContinuationPlan,
    ReplicationContinuationRequest,
    ReplicationContinuationSiteAction,
    plan_glioma_replication_continuation,
};
pub use replication_protocol::{
    ReplicationProtocolCompilation,
    ReplicationProtocolCompilationDisposition,
    ReplicationProtocolCompilationError,
    ReplicationProtocolCompileRequest,
    compile_glioma_replication_protocol,
};
pub use robust_active_learning::{
    RobustActiveLearningCandidate,
    RobustActiveLearningCandidateDisposition,
    RobustActiveLearningDisposition,
    RobustActiveLearningError,
    RobustActiveLearningModel,
    RobustActiveLearningObservation,
    RobustActiveLearningPlan,
    RobustActiveLearningRequest,
    RobustActiveLearningScore,
    plan_glioma_robust_active_learning,
};
pub use robust_design::{
    RobustCandidateAllocation,
    RobustDesignActionKind,
    RobustDesignCandidate,
    RobustDesignScenario,
    RobustExperimentDesign,
    RobustExperimentDesignDisposition,
    RobustExperimentDesignError,
    RobustExperimentDesignRequest,
    design_glioma_robust_experiment,
};
pub use sequential_campaign::{
    DryRunSequentialCampaignExecutor,
    SequentialBatchObservation,
    SequentialCampaign,
    SequentialCampaignDisposition,
    SequentialCampaignError,
    SequentialCampaignExecutionFailure,
    SequentialCampaignExecutor,
    SequentialCampaignRequest,
    SequentialCampaignRound,
    SequentialCampaignStopReason,
    execute_glioma_sequential_campaign,
};
pub use sequential_design::{
    SequentialArmDecision,
    SequentialArmObservation,
    SequentialDecisionKind,
    SequentialDesignDisposition,
    SequentialDesignError,
    SequentialDesignPlan,
    SequentialDesignRequest,
    SequentialDesignRound,
    plan_glioma_sequential_design,
};
pub use simulation_gated_campaign::{
    execute_glioma_simulation_gated_assay_campaign,
    execute_glioma_simulation_gated_assay_campaign_with_lineage_acquisition,
    AssayRouteDisposition,
    AssayRouteReadiness,
    ExecutedGliomaAssayRoute,
    GliomaInstrumentOutcomeInterpreter,
    InterpretedGliomaAssayOutcome,
    SimulationGatedAssayCampaignDisposition,
    SimulationGatedAssayCampaignError,
    SimulationGatedAssayCampaignRun,
    SimulationGatedAssayRoute,
};
pub use state_plasticity_instrument_campaign::{
    execute_glioma_lineage_propagation_guided_state_plasticity_campaign,
    execute_glioma_lineage_propagation_guided_state_plasticity_campaign_for_target,
    execute_glioma_lineage_response_guided_state_plasticity_campaign,
    execute_glioma_state_plasticity_instrument_campaign,
    GliomaStatePlasticityInstrumentError,
    GliomaStatePlasticityInstrumentInputs,
    GliomaStatePlasticityInstrumentRequest,
    GliomaStatePlasticityInstrumentRun,
    LineagePropagationStatePriority,
};
pub use state_stratified_campaign::{
    execute_glioma_state_stratified_campaign,
    CandidateSpecificAcquisitionPriority,
    CandidateSpecificOutcomeUpdate,
    GliomaResearchStratum,
    GliomaStateStratifiedAssayExecutor,
    RecordedStratifiedObservation,
    StateStratifiedCampaign,
    StateStratifiedCampaignError,
    StateStratifiedCampaignRequest,
    StateStratifiedDisposition,
    StateStratifiedExecutionFailure,
    StateStratifiedRound,
    StateStratifiedStopReason,
    StateStratumPosterior,
    StratifiedAssayCandidate,
    StratifiedAssayObservation,
};
pub use synergy::{
    CombinationCell,
    CombinationCellDisposition,
    CombinationObservation,
    CombinationSynergyAnalysis,
    CombinationSynergyDisposition,
    CombinationSynergyError,
    CombinationSynergyRequest,
    DosePair,
    analyze_glioma_combination_synergy,
};
pub use transition_guided_campaign::{
    execute_glioma_state_plasticity_research_workflow,
    execute_glioma_transition_guided_state_campaign,
    plan_glioma_transition_guided_state_priorities,
    GliomaStatePlasticityResearchRun,
    GliomaStatePlasticityWorkflowError,
    GliomaStatePlasticityWorkflowRequest,
    TransitionGuidanceRequest,
    TransitionGuidedStateCampaign,
    TransitionGuidedStateCampaignError,
    TransitionGuidedStateCampaignRequest,
    TransitionGuidedStatePriority,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ExperimentDesign;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P06")
}
