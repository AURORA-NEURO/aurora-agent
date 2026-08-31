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

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ProtocolSimulation;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P07")
}
