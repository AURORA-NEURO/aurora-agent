//! Question-to-decision context program ownership.

pub mod action_bridge;
pub mod context_compiler;

pub use action_bridge::{
    plan_decision_actions, DecisionActionPlan, DecisionActionPlanDisposition,
    DecisionActionPlanError, DecisionActionPlanRequest,
};
pub use context_compiler::{
    compile_decision_context, DecisionAction, DecisionActionKind, DecisionContext,
    DecisionContextDisposition, DecisionContextError, DecisionContextRequest,
};

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::DecisionContext;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P04")
}
