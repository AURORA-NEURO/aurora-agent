//! Worldgen P20 F03 security/federation workflow fabric.
use super::security_federation_workflow_support::{self,SecurityFederationWorkflowRequest,SecurityFederationWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P20-15"; pub const CONTRACT_VERSION:&str="worldgen-throughput-security-federation-workflow/1.0";
pub fn worldgen_throughput_security_federation_workflow_fabric_manifest()->serde_json::Value{security_federation_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn schedule_worldgen_throughput_security_federation_workflow(request:&SecurityFederationWorkflowRequest)->Result<SecurityFederationWorkflowReceipt,security_federation_workflow_support::SecurityFederationWorkflowError>{security_federation_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true,false)}
pub use security_federation_workflow_support::{SecurityFederationWorkflowError,SecurityFederationWorkflowRequest as WorldgenSecurityFederationWorkflowRequest,SecurityFederationWorkflowReceipt as WorldgenSecurityFederationWorkflowReceipt};
