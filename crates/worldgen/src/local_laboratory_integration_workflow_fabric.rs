//! Worldgen P11 AFA-worldgen-P11-F13 laboratory_integration workflow fabric.
use super::laboratory_integration_workflow_support::{self,InstrumentWorkflowRequest,InstrumentWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P11-F13"; pub const CONTRACT_VERSION:&str="worldgen-local-laboratory_integration-workflow/1.0";
pub fn worldgen_local_laboratory_integration_workflow_fabric_manifest()->serde_json::Value{laboratory_integration_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"InstrumentWorkflowRequest1@1","local single-study","A0")}
pub fn schedule_worldgen_local_laboratory_integration_workflow(request:&InstrumentWorkflowRequest)->Result<InstrumentWorkflowReceipt,laboratory_integration_workflow_support::InstrumentWorkflowError>{laboratory_integration_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use laboratory_integration_workflow_support::{InstrumentWorkflowError,InstrumentWorkflowReceipt as WorldgenLocalLaboratoryIntegrationworkflowfabricReceipt,InstrumentWorkflowRequest as WorldgenLocalLaboratoryIntegrationworkflowfabricRequest};

