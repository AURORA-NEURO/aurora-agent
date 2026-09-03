//! Worldgen P03-F17 local context-compilation researcher workbench.
use super::context_workbench_support::{self, ContextWorkbenchRequest, ContextWorkbenchReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F17";
pub const CONTRACT_VERSION: &str = "worldgen-local-context-compilation-workbench/1.0";
pub fn worldgen_local_context_compilation_research_workbench_manifest() -> serde_json::Value { context_workbench_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextCopilotRequest1@1", "local single-study", "A1") }
pub fn render_worldgen_local_context_compilation_research_workbench(request: &ContextWorkbenchRequest) -> Result<ContextWorkbenchReceipt, context_workbench_support::ContextWorkbenchError> { context_workbench_support::render(request, FEATURE_ID, CONTRACT_VERSION, false, false) }
pub use context_workbench_support::{ContextWorkbenchError, ContextWorkbenchReceipt as WorldgenLocalContextWorkbenchReceipt, ContextWorkbenchRequest as WorldgenLocalContextWorkbenchRequest};
