//! Federated continual retrieval-synthesis researcher workbench (`AFA-worldgen-P02-F20`).
use super::retrieval_workbench_support::{self, RetrievalWorkbenchRequest, RetrievalWorkbenchReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F20"; pub const CONTRACT_VERSION:&str="worldgen-federated-continual-retrieval-synthesis-workbench/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery4@1";
pub fn worldgen_federated_continual_retrieval_synthesis_research_workbench_manifest()->serde_json::Value{retrieval_workbench_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"federated continual","A2")}
pub fn render_worldgen_federated_continual_retrieval_synthesis_research_workbench(r:&RetrievalWorkbenchRequest)->Result<RetrievalWorkbenchReceipt,retrieval_workbench_support::RetrievalWorkbenchError>{retrieval_workbench_support::render(r,FEATURE_ID,CONTRACT_VERSION,true,true)}
pub use retrieval_workbench_support::{RetrievalWorkbenchError,RetrievalWorkbenchReceipt as WorldgenFederatedContinualRetrievalWorkbenchReceipt,RetrievalWorkbenchRequest as WorldgenFederatedContinualRetrievalWorkbenchRequest};
