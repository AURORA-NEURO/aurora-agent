//! Throughput retrieval-synthesis researcher workbench (`AFA-worldgen-P02-F19`).
use super::retrieval_workbench_support::{self, RetrievalWorkbenchRequest, RetrievalWorkbenchReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F19"; pub const CONTRACT_VERSION:&str="worldgen-throughput-retrieval-synthesis-workbench/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery3@1";
pub fn worldgen_throughput_retrieval_synthesis_research_workbench_manifest()->serde_json::Value{retrieval_workbench_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"prospective high-throughput","A1")}
pub fn render_worldgen_throughput_retrieval_synthesis_research_workbench(r:&RetrievalWorkbenchRequest)->Result<RetrievalWorkbenchReceipt,retrieval_workbench_support::RetrievalWorkbenchError>{retrieval_workbench_support::render(r,FEATURE_ID,CONTRACT_VERSION,true,false)}
pub use retrieval_workbench_support::{RetrievalWorkbenchError,RetrievalWorkbenchReceipt as WorldgenThroughputRetrievalWorkbenchReceipt,RetrievalWorkbenchRequest as WorldgenThroughputRetrievalWorkbenchRequest};
