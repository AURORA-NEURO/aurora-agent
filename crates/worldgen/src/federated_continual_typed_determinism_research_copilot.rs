//! Worldgen P17 F12 statistical, causal, and ML research copilot.
use super::typed_determinism_copilot_support::{self,DeterminismCopilotRequest,DeterminismCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P17-F12"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-typed-determinism-copilot/1.0";
pub fn worldgen_federated_continual_typed_determinism_research_copilot_manifest()->serde_json::Value{typed_determinism_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous")}
pub fn run_worldgen_federated_continual_typed_determinism_research_copilot(request:&DeterminismCopilotRequest)->Result<DeterminismCopilotReceipt,typed_determinism_copilot_support::DeterminismCopilotError>{typed_determinism_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",false,true)}
pub use typed_determinism_copilot_support::{DeterminismCopilotError,DeterminismCopilotRequest as WorldgenTypedDeterminismCopilotRequest,DeterminismCopilotReceipt as WorldgenTypedDeterminismCopilotReceipt};

