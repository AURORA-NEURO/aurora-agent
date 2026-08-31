//! Worldgen P03-F20 federated continual context-compilation researcher workbench.
use super::context_workbench_support::{self, ContextWorkbenchRequest, ContextWorkbenchReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F20";
pub const CONTRACT_VERSION: &str = "worldgen-federated-continual-context-compilation-workbench/1.0";
pub fn worldgen_federated_continual_context_compilation_research_workbench_manifest() -> serde_json::Value { context_workbench_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextCopilotRequest1@1", "federated continual autonomous", "A2") }
pub fn render_worldgen_federated_continual_context_compilation_research_workbench(request: &ContextWorkbenchRequest) -> Result<ContextWorkbenchReceipt, context_workbench_support::ContextWorkbenchError> { context_workbench_support::render(request, FEATURE_ID, CONTRACT_VERSION, true, true) }
pub use context_workbench_support::{ContextWorkbenchError, ContextWorkbenchReceipt as WorldgenFederatedContinualContextWorkbenchReceipt, ContextWorkbenchRequest as WorldgenFederatedContinualContextWorkbenchRequest};
