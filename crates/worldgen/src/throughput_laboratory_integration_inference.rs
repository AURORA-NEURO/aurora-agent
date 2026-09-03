//! Worldgen P11 AFA-worldgen-P11-F03 laboratory_integration exploration inference.
use super::laboratory_integration_support::{self,InstrumentActionRequest,InstrumentActionReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P11-F03"; pub const CONTRACT_VERSION:&str="worldgen-throughput-laboratory_integration-exploration/1.0";
pub fn worldgen_throughput_laboratory_integration_inference_manifest()->serde_json::Value{laboratory_integration_support::manifest(FEATURE_ID,CONTRACT_VERSION,"InstrumentActionRequest1@1","prospective high-throughput","A1")}
pub fn integrate_worldgen_throughput_laboratory_integrations(request:&InstrumentActionRequest)->Result<InstrumentActionReceipt,laboratory_integration_support::LaboratoryIntegrationError>{laboratory_integration_support::integrate(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false)}
pub use laboratory_integration_support::{InstrumentAction,LaboratoryIntegrationError,InstrumentActionReceipt as WorldgenThroughputLaboratoryIntegrationportfolioInference,InstrumentActionRequest as WorldgenThroughputLaboratoryIntegrationquestionInference};

