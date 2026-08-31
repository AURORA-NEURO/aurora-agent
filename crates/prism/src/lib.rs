//! PRISM: decision-state evaluation.
//!
//! Implements the core of blueprint 03 (Decision Cell IR, Oracle IR, Result Bundle), 06 (the
//! benchmark compiler's minimization step) and 07 (matched evaluation on deterministic evidence).
//!
//! The unit of evaluation is a frozen decision state rather than a task outcome. Candidates resume
//! from the identical state, so a difference between them is attributable to the one component the
//! cell left free — here, the context policy. That is the whole argument for cells over end-to-end
//! comparison, and it is why the fork reports *attribution* rather than a score.

pub mod analysis_workbench;
pub mod evaluation_integrity_support;
pub mod local_evaluation_integrity_inference;
pub mod multimodal_evaluation_integrity_inference;
pub mod throughput_evaluation_integrity_inference;
pub mod federated_continual_evaluation_integrity_inference;
pub mod local_evaluation_integrity_contract_model;
pub mod multimodal_evaluation_integrity_contract_model;
pub mod throughput_evaluation_integrity_contract_model;
pub mod federated_continual_evaluation_integrity_contract_model;
pub mod local_evaluation_integrity_research_copilot;
pub mod multimodal_evaluation_integrity_research_copilot;
pub mod throughput_evaluation_integrity_research_copilot;
pub mod federated_continual_evaluation_integrity_research_copilot;
pub mod local_evaluation_integrity_workflow_fabric;
pub mod multimodal_evaluation_integrity_workflow_fabric;
pub mod throughput_evaluation_integrity_workflow_fabric;
pub mod federated_continual_evaluation_integrity_workflow_fabric;
pub mod architecture;
pub mod bundle;
pub mod cell;
pub mod fork;
pub mod minimize;
pub mod protocol_simulation_assurance;
pub mod laboratory_integration_copilot;

pub use analysis_workbench::{
    analysis_workbench_manifest, qualify_analysis_workbench, AnalysisJob6,
    AnalysisWorkbenchDisposition, AnalysisWorkbenchError, AnalysisWorkbenchReceipt7,
    AnalysisWorkbenchRequest5, CONTENT_TYPE as ANALYSIS_WORKBENCH_CONTENT_TYPE,
    CONTRACT_VERSION as ANALYSIS_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as ANALYSIS_WORKBENCH_FEATURE_ID,
};
pub use architecture::{Architecture, StrategySpec};
pub use evaluation_integrity_support::{evaluate as evaluate_integrity, manifest as evaluation_integrity_manifest, EvaluationArm4, EvaluationIntegrityArtifact4, EvaluationIntegrityCard7, EvaluationIntegrityError, EvaluationIntegrityRequest4, BOUNDARY as EVALUATION_INTEGRITY_BOUNDARY, CONTENT_TYPE as EVALUATION_INTEGRITY_CONTENT_TYPE};
pub use local_evaluation_integrity_inference::*;
pub use multimodal_evaluation_integrity_inference::*;
pub use throughput_evaluation_integrity_inference::*;
pub use federated_continual_evaluation_integrity_inference::*;
pub use local_evaluation_integrity_contract_model::*;
pub use multimodal_evaluation_integrity_contract_model::*;
pub use throughput_evaluation_integrity_contract_model::*;
pub use federated_continual_evaluation_integrity_contract_model::*;
pub use local_evaluation_integrity_research_copilot::*;
pub use multimodal_evaluation_integrity_research_copilot::*;
pub use throughput_evaluation_integrity_research_copilot::*;
pub use federated_continual_evaluation_integrity_research_copilot::*;
pub use local_evaluation_integrity_workflow_fabric::*;
pub use multimodal_evaluation_integrity_workflow_fabric::*;
pub use throughput_evaluation_integrity_workflow_fabric::*;
pub use federated_continual_evaluation_integrity_workflow_fabric::*;
pub use bundle::{Attestation, Reproduction, ResultBundle, BUNDLE_SCHEMA_VERSION};
pub use cell::{Acceptance, DecisionCell, InputRef, CELL_SCHEMA_VERSION};
pub use fork::{
    matched_fork, render_table, Arm, ArmFailure, ForkResult, NotAttemptedReason, Trial,
};
pub use minimize::{
    minimize, minimize_world, preserves, Minimization, MinimizeError, Preservation, UnjudgedRemoval,
};
pub use protocol_simulation_assurance::{
    assure as assure_protocol_simulation, assure_json as assure_protocol_simulation_json,
    capability_manifest as protocol_simulation_capability_manifest, ProtocolDraft,
    ProtocolSimulationAssuranceError, ProtocolSimulationReport,
    CONTRACT_VERSION as PROTOCOL_SIMULATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as PROTOCOL_SIMULATION_ASSURANCE_FEATURE_ID,
    INPUT_SCHEMA as PROTOCOL_SIMULATION_ASSURANCE_INPUT_SCHEMA,
    OUTPUT_SCHEMA as PROTOCOL_SIMULATION_ASSURANCE_OUTPUT_SCHEMA,
};
pub use laboratory_integration_copilot::{
    admit_laboratory_integration_action, laboratory_integration_copilot_manifest,
    InstrumentActionArtifact3, InstrumentActionReceipt3, InstrumentActionRequest4,
    LaboratoryIntegrationError,
    CONTRACT_VERSION as LABORATORY_INTEGRATION_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as LABORATORY_INTEGRATION_COPILOT_FEATURE_ID,
    INPUT_SCHEMA as LABORATORY_INTEGRATION_COPILOT_INPUT_SCHEMA,
    OUTPUT_SCHEMA as LABORATORY_INTEGRATION_COPILOT_OUTPUT_SCHEMA,
};
