//! Protocol simulation and adaptive workflow program ownership.
//!
//! The workflow planner is re-exported here so callers can discover the P07 surface through the
//! folder-owned program module while the shared glioma namespace retains a stable API.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub use crate::glioma::workflow::{
    execute_glioma_workflow, plan_glioma_workflow, GliomaWorkflowBranch, GliomaWorkflowError,
    GliomaWorkflowExecution, GliomaWorkflowMode, GliomaWorkflowNode, GliomaWorkflowPlan,
    GliomaWorkflowRequest, WorkflowNodeDecision,
};
pub mod action_execution;
pub mod execution;
pub mod simulator;

pub use action_execution::{
    execute_glioma_action_portfolio, ActionExecutionDisposition, ActionExecutionFailure,
    ActionExecutionResult, ActionPortfolioExecution, ActionPortfolioExecutionDisposition,
    ActionPortfolioExecutionError, ActionPortfolioExecutionRequest, ActionPortfolioStopReason,
    DryRunGliomaActionExecutor, GliomaActionExecutor,
};

pub use execution::{
    execute_glioma_protocol, DryRunGliomaProtocolExecutor, GliomaProtocolExecutor,
    ProtocolExecution, ProtocolExecutionDisposition, ProtocolExecutionError,
    ProtocolExecutionFailure, ProtocolExecutionRequest, ProtocolExecutionStopReason,
    ProtocolTaskDisposition, ProtocolTaskResult, OUTPUT_SCHEMA as PROTOCOL_EXECUTION_OUTPUT_SCHEMA,
};

pub use simulator::{
    protocol_request_from_experiment_design, simulate_glioma_protocol, ProtocolDisposition,
    ProtocolResource, ProtocolResourceKind, ProtocolSimulation, ProtocolSimulationError,
    ProtocolSimulationRequest, ProtocolTask, ResourceUtilization, ScheduleEntry,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ProtocolSimulation;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P07")
}
