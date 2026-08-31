//! Worldgen P11 AFA-worldgen-P11-F16 laboratory_integration workflow fabric.
use super::laboratory_integration_workflow_support::{self,InstrumentWorkflowRequest,InstrumentWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P11-F16"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-laboratory_integration-workflow/1.0";
pub fn worldgen_federated_continual_laboratory_integration_workflow_fabric_manifest()->serde_json::Value{laboratory_integration_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"InstrumentWorkflowRequest1@1","federated continual autonomous","A1")}
pub fn schedule_worldgen_federated_continual_laboratory_integration_workflow(request:&InstrumentWorkflowRequest)->Result<InstrumentWorkflowReceipt,laboratory_integration_workflow_support::InstrumentWorkflowError>{laboratory_integration_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",false,true)}
pub use laboratory_integration_workflow_support::{InstrumentWorkflowError,InstrumentWorkflowReceipt as WorldgenFederatedContinualLaboratoryIntegrationworkflowfabricReceipt,InstrumentWorkflowRequest as WorldgenFederatedContinualLaboratoryIntegrationworkflowfabricRequest};

