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
    generate_feature_catalog, glioma_program_catalog, validate_feature_catalog, CatalogError,
    GliomaFeatureSpec, GliomaOperatingScale, GliomaProgramDescriptor, GliomaProgramId,
};
pub use evidence::{qualify_evidence, EvidenceQualification, EvidenceRecord, EvidenceRequest};
pub use experiment::{
    design_preclinical_experiment, ExperimentArm, ExperimentDesign, ExperimentRequest,
};
pub use mechanism::{explore_mechanisms, MechanismCandidate, MechanismPortfolio, MechanismRequest};
pub use multimodal::{
    harmonize_multimodal_inputs, MultimodalObservation, MultimodalQcReport, MultimodalRequest,
};
pub use programs::p03_multimodal_ingestion_qc::{
    analyze_multimodal_concordance, ConcordanceDisposition, ConcordanceError, ConcordanceRequest,
    FeatureValue, ModalityConcordance, ModalityVector, MultimodalConcordance,
    PairConcordanceDisposition,
};
pub use programs::p06_experiment_design::{
    analyze_glioma_dose_response, DoseDirection, DoseResponseAnalysis, DoseResponseDisposition,
    DoseResponseError, DoseResponseObservation, DoseResponsePoint, DoseResponseRequest,
};
pub use programs::p07_protocol_simulation::{
    protocol_request_from_experiment_design, simulate_glioma_protocol, ProtocolDisposition,
    ProtocolResource, ProtocolResourceKind, ProtocolSimulation, ProtocolSimulationError,
    ProtocolSimulationRequest, ProtocolTask, ResourceUtilization, ScheduleEntry,
};
pub use programs::p09_reproducible_computation::{
    assess_glioma_robustness, RobustnessCase, RobustnessCaseKind, RobustnessDisposition,
    RobustnessError, RobustnessRequest, RobustnessSuite,
};
pub use programs::p10_interpretation_replication::{
    analyze_glioma_trajectories, TrajectoryAnalysis, TrajectoryArmSummary, TrajectoryDisposition,
    TrajectoryError, TrajectoryObservation, TrajectoryRequest, UnitTrajectory,
    UnitTrajectoryDisposition,
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
