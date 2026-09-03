//! Worldgen P18 F13 statistical, causal, and ML workflow fabric.
use super::provenance_signing_workflow_support::{self,ProvenanceWorkflowRequest,ProvenanceWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P18-F13"; pub const CONTRACT_VERSION:&str="worldgen-local-provenance-signing-workflow/1.0";
pub fn worldgen_local_provenance_signing_workflow_fabric_manifest()->serde_json::Value{provenance_signing_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn schedule_worldgen_local_provenance_signing_workflow(request:&ProvenanceWorkflowRequest)->Result<ProvenanceWorkflowReceipt,provenance_signing_workflow_support::ProvenanceWorkflowError>{provenance_signing_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use provenance_signing_workflow_support::{ProvenanceWorkflowError,ProvenanceWorkflowRequest as WorldgenTypedProvenanceWorkflowRequest,ProvenanceWorkflowReceipt as WorldgenTypedProvenanceWorkflowReceipt};

