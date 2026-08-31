//! Worldgen P11 AFA-worldgen-P11-F12 laboratory_integration research copilot.
use super::laboratory_integration_copilot_support::{self,InstrumentCopilotRequest,InstrumentCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P11-F12"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-laboratory_integration-copilot/1.0";
pub fn worldgen_federated_continual_laboratory_integration_research_copilot_manifest()->serde_json::Value{laboratory_integration_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"InstrumentCopilotRequest1@1","federated continual autonomous","A1")}
pub fn run_worldgen_federated_continual_laboratory_integration_research_copilot(request:&InstrumentCopilotRequest)->Result<InstrumentCopilotReceipt,laboratory_integration_copilot_support::InstrumentCopilotError>{laboratory_integration_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",false,true)}
pub use laboratory_integration_copilot_support::{InstrumentCopilotError,InstrumentCopilotReceipt as WorldgenFederatedContinualLaboratoryIntegrationresearchcopilotReceipt,InstrumentCopilotRequest as WorldgenFederatedContinualLaboratoryIntegrationresearchcopilotRequest};

