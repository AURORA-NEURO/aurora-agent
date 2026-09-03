//! Local retrieval-synthesis researcher workbench (`AFA-worldgen-P02-F17`).
use super::retrieval_workbench_support::{self, RetrievalWorkbenchRequest, RetrievalWorkbenchReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F17"; pub const CONTRACT_VERSION:&str="worldgen-local-retrieval-synthesis-workbench/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery1@1";
pub fn worldgen_local_retrieval_synthesis_research_workbench_manifest()->serde_json::Value{retrieval_workbench_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"local single-study","A0")}
pub fn render_worldgen_local_retrieval_synthesis_research_workbench(r:&RetrievalWorkbenchRequest)->Result<RetrievalWorkbenchReceipt,retrieval_workbench_support::RetrievalWorkbenchError>{retrieval_workbench_support::render(r,FEATURE_ID,CONTRACT_VERSION,false,false)}
pub use retrieval_workbench_support::{RetrievalWorkbenchError,RetrievalWorkbenchReceipt as WorldgenLocalRetrievalWorkbenchReceipt,RetrievalWorkbenchRequest as WorldgenLocalRetrievalWorkbenchRequest};
