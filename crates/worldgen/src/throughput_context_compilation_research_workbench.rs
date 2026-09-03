//! Worldgen P03-F19 prospective high-throughput context-compilation researcher workbench.
use super::context_workbench_support::{self, ContextWorkbenchRequest, ContextWorkbenchReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F19";
pub const CONTRACT_VERSION: &str = "worldgen-throughput-context-compilation-workbench/1.0";
pub fn worldgen_throughput_context_compilation_research_workbench_manifest() -> serde_json::Value { context_workbench_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextCopilotRequest1@1", "prospective high-throughput", "A2") }
pub fn render_worldgen_throughput_context_compilation_research_workbench(request: &ContextWorkbenchRequest) -> Result<ContextWorkbenchReceipt, context_workbench_support::ContextWorkbenchError> { context_workbench_support::render(request, FEATURE_ID, CONTRACT_VERSION, true, true) }
pub use context_workbench_support::{ContextWorkbenchError, ContextWorkbenchReceipt as WorldgenThroughputContextWorkbenchReceipt, ContextWorkbenchRequest as WorldgenThroughputContextWorkbenchRequest};
