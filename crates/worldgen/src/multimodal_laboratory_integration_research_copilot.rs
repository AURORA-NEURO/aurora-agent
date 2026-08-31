//! Worldgen P11 AFA-worldgen-P11-F10 laboratory_integration research copilot.
use super::laboratory_integration_copilot_support::{self,InstrumentCopilotRequest,InstrumentCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P11-F10"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-laboratory_integration-copilot/1.0";
pub fn worldgen_multimodal_laboratory_integration_research_copilot_manifest()->serde_json::Value{laboratory_integration_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"InstrumentCopilotRequest1@1","multimodal multi-study","A1")}
pub fn run_worldgen_multimodal_laboratory_integration_research_copilot(request:&InstrumentCopilotRequest)->Result<InstrumentCopilotReceipt,laboratory_integration_copilot_support::InstrumentCopilotError>{laboratory_integration_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false,false)}
pub use laboratory_integration_copilot_support::{InstrumentCopilotError,InstrumentCopilotReceipt as WorldgenMultimodalLaboratoryIntegrationresearchcopilotReceipt,InstrumentCopilotRequest as WorldgenMultimodalLaboratoryIntegrationresearchcopilotRequest};

