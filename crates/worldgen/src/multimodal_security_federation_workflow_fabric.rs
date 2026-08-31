//! Worldgen P20 F02 security/federation workflow fabric.
use super::security_federation_workflow_support::{self,SecurityFederationWorkflowRequest,SecurityFederationWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P20-14"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-security-federation-workflow/1.0";
pub fn worldgen_multimodal_security_federation_workflow_fabric_manifest()->serde_json::Value{security_federation_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn schedule_worldgen_multimodal_security_federation_workflow(request:&SecurityFederationWorkflowRequest)->Result<SecurityFederationWorkflowReceipt,security_federation_workflow_support::SecurityFederationWorkflowError>{security_federation_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",true,false)}
pub use security_federation_workflow_support::{SecurityFederationWorkflowError,SecurityFederationWorkflowRequest as WorldgenSecurityFederationWorkflowRequest,SecurityFederationWorkflowReceipt as WorldgenSecurityFederationWorkflowReceipt};
