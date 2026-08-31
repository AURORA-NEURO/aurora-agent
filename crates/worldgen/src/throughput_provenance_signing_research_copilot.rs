//! Worldgen P18 F11 statistical, causal, and ML research copilot.
use super::provenance_signing_copilot_support::{self,ProvenanceCopilotRequest,ProvenanceCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P18-F11"; pub const CONTRACT_VERSION:&str="worldgen-throughput-provenance-signing-copilot/1.0";
pub fn worldgen_throughput_provenance_signing_research_copilot_manifest()->serde_json::Value{provenance_signing_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn run_worldgen_throughput_provenance_signing_research_copilot(request:&ProvenanceCopilotRequest)->Result<ProvenanceCopilotReceipt,provenance_signing_copilot_support::ProvenanceCopilotError>{provenance_signing_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true,true)}
pub use provenance_signing_copilot_support::{ProvenanceCopilotError,ProvenanceCopilotRequest as WorldgenTypedProvenanceCopilotRequest,ProvenanceCopilotReceipt as WorldgenTypedProvenanceCopilotReceipt};

