//! Question-to-decision context program ownership.

pub mod action_bridge;
pub mod action_graph;
pub mod adaptive_branch_campaign;
pub mod branch_campaign;
pub mod branch_planner;
pub mod campaign;
pub mod context_compiler;
pub mod decision_cycle;
pub mod omission_certificate;

pub use action_bridge::{
    plan_decision_actions, DecisionActionPlan, DecisionActionPlanDisposition,
    DecisionActionPlanError, DecisionActionPlanRequest,
};
pub use action_graph::{
    compile_decision_action_graph, DecisionActionGraph, DecisionActionGraphDisposition,
    DecisionActionGraphError, DecisionActionGraphRequest, DecisionGraphNode,
};
pub use adaptive_branch_campaign::{
    execute_glioma_adaptive_decision_branch_campaign,
    execute_glioma_adaptive_decision_branch_campaign_dry_run, AdaptiveDecisionBranchCampaign,
    AdaptiveDecisionBranchCampaignDisposition, AdaptiveDecisionBranchCampaignError,
    AdaptiveDecisionBranchCampaignRequest, AdaptiveDecisionBranchCampaignRound,
    AdaptiveDecisionBranchCampaignStopReason,
};
pub use branch_campaign::{
    execute_glioma_decision_branch_campaign, BranchExecutionDisposition, DecisionBranchCampaign,
    DecisionBranchCampaignDisposition, DecisionBranchCampaignError, DecisionBranchCampaignRequest,
    DecisionBranchCampaignStopReason, DecisionBranchExecution,
};
pub use branch_planner::{
    plan_glioma_decision_branches, DecisionBranchPlan, DecisionBranchPlanDisposition,
    DecisionBranchPlannerError, DecisionBranchPlannerRequest, DecisionBranchPortfolio,
    DecisionScenario, DecisionScenarioOutcome, DecisionScenarioScore,
};
pub use campaign::{
    execute_glioma_decision_context_campaign, DecisionContextCampaign,
    DecisionContextCampaignDisposition, DecisionContextCampaignError,
    DecisionContextCampaignExecutionFailure, DecisionContextCampaignExecutor,
    DecisionContextCampaignRequest, DecisionContextCampaignRound,
    DecisionContextCampaignStopReason, DryRunDecisionContextCampaignExecutor,
};
pub use context_compiler::{
    compile_decision_context, DecisionAction, DecisionActionKind, DecisionContext,
    DecisionContextDisposition, DecisionContextError, DecisionContextRequest,
};
pub use decision_cycle::{
    execute_glioma_decision_operating_cycle, DecisionOperatingCycle,
    DecisionOperatingCycleDisposition, DecisionOperatingCycleError, DecisionOperatingCycleRequest,
};
pub use omission_certificate::{
    certify_decision_omissions, DecisionCoverageState, DecisionOmissionCertificate,
    DecisionOmissionCertificateError, DecisionOmissionCertificateRequest,
    DecisionOmissionDisposition, DecisionOmissionEntry,
};

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::DecisionContext;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P04")
}
