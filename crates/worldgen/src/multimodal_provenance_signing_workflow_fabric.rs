//! Worldgen P18 F14 statistical, causal, and ML workflow fabric.
use super::provenance_signing_workflow_support::{self,ProvenanceWorkflowRequest,ProvenanceWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P18-F14"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-provenance-signing-workflow/1.0";
pub fn worldgen_multimodal_provenance_signing_workflow_fabric_manifest()->serde_json::Value{provenance_signing_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn schedule_worldgen_multimodal_provenance_signing_workflow(request:&ProvenanceWorkflowRequest)->Result<ProvenanceWorkflowReceipt,provenance_signing_workflow_support::ProvenanceWorkflowError>{provenance_signing_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",true,false)}
pub use provenance_signing_workflow_support::{ProvenanceWorkflowError,ProvenanceWorkflowRequest as WorldgenTypedProvenanceWorkflowRequest,ProvenanceWorkflowReceipt as WorldgenTypedProvenanceWorkflowReceipt};

