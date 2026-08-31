//! Worldgen P11 AFA-worldgen-P11-F02 laboratory_integration exploration inference.
use super::laboratory_integration_support::{self,InstrumentActionRequest,InstrumentActionReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P11-F02"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-laboratory_integration-exploration/1.0";
pub fn worldgen_multimodal_laboratory_integration_inference_manifest()->serde_json::Value{laboratory_integration_support::manifest(FEATURE_ID,CONTRACT_VERSION,"InstrumentActionRequest1@1","multimodal multi-study","A1")}
pub fn integrate_worldgen_multimodal_laboratory_integrations(request:&InstrumentActionRequest)->Result<InstrumentActionReceipt,laboratory_integration_support::LaboratoryIntegrationError>{laboratory_integration_support::integrate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use laboratory_integration_support::{InstrumentAction,LaboratoryIntegrationError,InstrumentActionReceipt as WorldgenMultimodalLaboratoryIntegrationportfolioInference,InstrumentActionRequest as WorldgenMultimodalLaboratoryIntegrationquestionInference};

