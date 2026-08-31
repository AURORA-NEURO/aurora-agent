//! Worldgen P11 AFA-worldgen-P11-F04 laboratory_integration exploration inference.
use super::laboratory_integration_support::{self,InstrumentActionRequest,InstrumentActionReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P11-F04"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-laboratory_integration-exploration/1.0";
pub fn worldgen_federated_continual_laboratory_integration_inference_manifest()->serde_json::Value{laboratory_integration_support::manifest(FEATURE_ID,CONTRACT_VERSION,"InstrumentActionRequest1@1","federated continual autonomous","A1")}
pub fn integrate_worldgen_federated_continual_laboratory_integrations(request:&InstrumentActionRequest)->Result<InstrumentActionReceipt,laboratory_integration_support::LaboratoryIntegrationError>{laboratory_integration_support::integrate(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true)}
pub use laboratory_integration_support::{InstrumentAction,LaboratoryIntegrationError,InstrumentActionReceipt as WorldgenFederatedContinualLaboratoryIntegrationportfolioInference,InstrumentActionRequest as WorldgenFederatedContinualLaboratoryIntegrationquestionInference};

