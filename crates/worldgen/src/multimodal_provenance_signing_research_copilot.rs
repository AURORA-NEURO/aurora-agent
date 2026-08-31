//! Worldgen P18 F10 statistical, causal, and ML research copilot.
use super::provenance_signing_copilot_support::{self,ProvenanceCopilotRequest,ProvenanceCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P18-F10"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-provenance-signing-copilot/1.0";
pub fn worldgen_multimodal_provenance_signing_research_copilot_manifest()->serde_json::Value{provenance_signing_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn run_worldgen_multimodal_provenance_signing_research_copilot(request:&ProvenanceCopilotRequest)->Result<ProvenanceCopilotReceipt,provenance_signing_copilot_support::ProvenanceCopilotError>{provenance_signing_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",true,false)}
pub use provenance_signing_copilot_support::{ProvenanceCopilotError,ProvenanceCopilotRequest as WorldgenTypedProvenanceCopilotRequest,ProvenanceCopilotReceipt as WorldgenTypedProvenanceCopilotReceipt};

