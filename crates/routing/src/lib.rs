#![allow(clippy::all)]

//! Evaluation-conditioned inference routing.
//!
//! Implements blueprint 09.01 (Inference Lab) and 09.08 (Evaluation-Conditioned Inference Router)
//! against the bar set by `00_START_HERE/04_CRITICAL_PATH.md` Gate 6:
//!
//! > Use prior evaluation evidence to choose between at least three architecture configurations
//! > on unseen tasks. Compare against a fixed default, a largest-model default, and an oracle
//! > retrospective selector.
//!
//! The executive summary's product loop ends with "route future tasks to a better inference
//! architecture", and its second flywheel closes only if that step can be *measured* rather than
//! asserted. This crate is that measurement, and it is deliberately small.
//!
//! # What this crate routes between
//!
//! Context architectures — the equal-engineering strategies of `bioprism-baseline`: full context,
//! depth-limited graph traversal at several depths, the whole connected component, the query
//! graph, BM25 at several cut-offs, and the FIBER compiler. Nothing else.
//!
//! **Not implemented, deliberately:** no model selection, no LLM call, no prompt, no agent
//! topology, no training of any kind, no contextual bandit, no online learning, no exploration
//! schedule, no latency or token-cost or provider-risk model, and no significance testing.
//! Blueprint 09.08 names hierarchical outcome models and bandits, and 09.11 names guarded
//! exploration with delayed labels and rollback monitoring; none of that is here. What *is* here
//! is a nearest-neighbour average over prior outcomes, which is the weakest thing that can still
//! be called evidence-conditioned — and therefore the honest thing to measure first.
//!
//! The restriction is not modesty for its own sake. Routing between context strategies is a claim
//! this workspace can settle with the deterministic oracle of blueprint 43.41: every candidate is
//! run, its verdict is checked against full context, and its protected closure is checked against
//! 43.13. Routing between models would need an evaluator this repository does not have.
//!
//! # The shape of the argument
//!
//! 1. [`fingerprint`] computes cheap structural features of a task **without compiling it**.
//!    A router that must compile in order to decide whether to compile has saved nothing, and a
//!    router that reads the oracle verdict is not routing. The test
//!    `the_fingerprint_cannot_see_the_answer` pins the second half: two worlds whose verdicts
//!    differ fingerprint identically.
//! 2. [`policy`] ranks approved architectures by mean utility among structurally nearby prior
//!    tasks, and **abstains to a safe default** when coverage or margin is insufficient rather
//!    than guessing.
//! 3. [`comparator`] fixes the four Gate 6 selectors, exactly one of which is allowed to know the
//!    answer.
//! 4. [`regret`] measures the router against the oracle bound and reports the *fraction of
//!    achievable gain captured*, not the win rate. A router that wins nine tasks in ten while
//!    converting a rounding error of the available improvement is a bad router that reports 90%.
//! 5. [`report`] prints all four comparators together, always, and states a computed verdict in
//!    which "the router lost" is checked before every other case.
//!
//! # Why the oracle is mandatory
//!
//! The oracle retrospective selector already knows each task's outcome, so it is not a policy and
//! could never be deployed. It is the ceiling. Without it, "the router beat the fixed default" is
//! compatible with capturing two percent of what was on the table, and is also easy to manufacture
//! by choosing a weak default. With it, the same sentence becomes a fraction a reader can check.
//! [`regret::RegretAccount`] therefore has four named fields rather than a list, so a report
//! cannot be assembled with the bound left out.
//!
//! # Example
//!
//! ```no_run
//! use bioprism_routing::{ApprovedSet, Architecture, LabSettings, RoutingPolicy, Task, lab};
//!
//! let approved = ApprovedSet::reference_panel();
//! let policy = RoutingPolicy::defaulting_to(approved, Architecture::FiberCompiled)?;
//! let settings = LabSettings::new(policy, Architecture::GraphKHop { depth: 5 })?;
//!
//! let tasks: Vec<Task> = Vec::new();
//! let report = lab::run(&tasks, &settings)?;
//! println!("{}", report.to_markdown());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod architecture;
pub mod calibration;
pub mod comparator;
pub mod error;
pub mod evidence;
pub mod federated_execution_copilot;
pub mod federated_multimodal_assurance;
pub mod federated_replication_negative_results_copilot;
pub mod fingerprint;
pub mod lab;
pub mod laboratory_inference_engine;
pub mod limitation_closure_workflow;
pub mod policy;
pub mod regret;
pub mod report;

#[cfg(test)]
mod test_support;

pub use architecture::{ApprovedSet, Architecture};
pub use calibration::{CalibrationBin, CalibrationCurve};
pub use comparator::Comparator;
pub use error::RoutingError;
pub use evidence::{utility, EvidenceLedger, Observation, INADMISSIBLE_UTILITY};
pub use federated_execution_copilot::{
    federated_execution_copilot_manifest, route_federated_execution, ExecutionPlanCandidate6,
    ExecutionPlanEvidenceState, ExecutionPlanPeer5, ExecutionRoutingReceipt9,
    FederatedExecutionCopilotError, FederatedExecutionCopilotRequest8,
    CONTRACT_VERSION as FEDERATED_EXECUTION_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_EXECUTION_COPILOT_FEATURE_ID,
};
pub use federated_multimodal_assurance::{
    assure_federated_multimodal, FederatedAssuranceDisposition, FederatedAssuranceError,
    FederatedMultimodalAssuranceReceipt, FederatedMultimodalAssuranceRequest,
    CONTRACT_VERSION as FEDERATED_MULTIMODAL_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_MULTIMODAL_ASSURANCE_FEATURE_ID,
};
pub use federated_replication_negative_results_copilot::{
    assure_federated_replication, federated_replication_negative_results_manifest,
    ReplicationCopilotArtifact8, ReplicationCopilotError, ReplicationCopilotReceipt8,
    ReplicationObservation6, ReplicationOutcome, ReplicationPeer5, ReplicationQuestion6,
    CONTRACT_VERSION as FEDERATED_REPLICATION_NEGATIVE_RESULTS_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_REPLICATION_NEGATIVE_RESULTS_FEATURE_ID,
};
pub use fingerprint::{AttachmentRegime, ChainRegime, Fingerprint, Regime, TagRegime};
pub use lab::{observe, Holdout, LabSettings, Task};
pub use laboratory_inference_engine::{
    infer_laboratory_actions, infer_laboratory_actions_json, laboratory_inference_manifest,
    validate_laboratory_actions_json, InstrumentActionReceipt1, InstrumentActionRequest4,
    InstrumentCandidate4, InstrumentDisposition, LaboratoryInferenceError,
    CONTRACT_VERSION as LABORATORY_INFERENCE_CONTRACT_VERSION,
    FEATURE_ID as LABORATORY_INFERENCE_FEATURE_ID,
};
pub use limitation_closure_workflow::{
    compile_limitation_closure_workflow, limitation_closure_workflow_manifest, Limitation6,
    LimitationClosureDisposition, LimitationClosureError, LimitationClosureWorkflowReceipt7,
    LimitationClosureWorkflowRequest5,
    CONTRACT_VERSION as LIMITATION_CLOSURE_WORKFLOW_CONTRACT_VERSION,
    FEATURE_ID as LIMITATION_CLOSURE_WORKFLOW_FEATURE_ID,
};
pub use policy::{
    ArchitectureScore, DecisionReason, RoutingDecision, RoutingPolicy,
    DEFAULT_CONFIDENCE_MARGIN_SCALE, DEFAULT_MIN_MARGIN, DEFAULT_NEIGHBOURHOOD_RADIUS,
};
pub use regret::{ComparatorOutcome, RegretAccount, RoutingVerdict, GAIN_EPSILON};
pub use report::{ComparatorPick, RoutingReport, TaskRow};
