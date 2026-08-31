//! Worldgen P17 F10 statistical, causal, and ML research copilot.
use super::typed_determinism_copilot_support::{self,DeterminismCopilotRequest,DeterminismCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P17-F10"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-typed-determinism-copilot/1.0";
pub fn worldgen_multimodal_typed_determinism_research_copilot_manifest()->serde_json::Value{typed_determinism_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn run_worldgen_multimodal_typed_determinism_research_copilot(request:&DeterminismCopilotRequest)->Result<DeterminismCopilotReceipt,typed_determinism_copilot_support::DeterminismCopilotError>{typed_determinism_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",true,false)}
pub use typed_determinism_copilot_support::{DeterminismCopilotError,DeterminismCopilotRequest as WorldgenTypedDeterminismCopilotRequest,DeterminismCopilotReceipt as WorldgenTypedDeterminismCopilotReceipt};

