//! Local retrieval-synthesis workflow fabric (`AFA-worldgen-P02-F13`).
use super::retrieval_workflow_support::{self, RetrievalWorkflowReceipt, RetrievalWorkflowRequest};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F13"; pub const CONTRACT_VERSION:&str="worldgen-local-retrieval-synthesis-workflow/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery1@1";
pub fn worldgen_local_retrieval_synthesis_workflow_fabric_manifest()->serde_json::Value{retrieval_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"local single-study","A1")}
pub fn schedule_worldgen_local_retrieval_synthesis_workflow(r:&RetrievalWorkflowRequest)->Result<RetrievalWorkflowReceipt,retrieval_workflow_support::RetrievalWorkflowError>{retrieval_workflow_support::schedule(r,FEATURE_ID,CONTRACT_VERSION,false,false)}
pub use retrieval_workflow_support::{RetrievalWorkflowError,RetrievalWorkflowReceipt as WorldgenLocalRetrievalWorkflowReceipt,RetrievalWorkflowRequest as WorldgenLocalRetrievalWorkflowRequest};
