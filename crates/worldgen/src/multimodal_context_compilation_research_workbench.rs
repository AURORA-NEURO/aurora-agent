//! Worldgen P03-F18 multimodal context-compilation researcher workbench.
use super::context_workbench_support::{self, ContextWorkbenchRequest, ContextWorkbenchReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F18";
pub const CONTRACT_VERSION: &str = "worldgen-multimodal-context-compilation-workbench/1.0";
pub fn worldgen_multimodal_context_compilation_research_workbench_manifest() -> serde_json::Value { context_workbench_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextCopilotRequest1@1", "multimodal multi-study", "A1") }
pub fn render_worldgen_multimodal_context_compilation_research_workbench(request: &ContextWorkbenchRequest) -> Result<ContextWorkbenchReceipt, context_workbench_support::ContextWorkbenchError> { context_workbench_support::render(request, FEATURE_ID, CONTRACT_VERSION, true, false) }
pub use context_workbench_support::{ContextWorkbenchError, ContextWorkbenchReceipt as WorldgenMultimodalContextWorkbenchReceipt, ContextWorkbenchRequest as WorldgenMultimodalContextWorkbenchRequest};
