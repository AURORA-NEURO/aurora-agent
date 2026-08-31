//! Local retrieval-synthesis research copilot (`AFA-worldgen-P02-F09`).
use super::retrieval_copilot_support::{self, RetrievalCopilotReceipt, RetrievalCopilotRequest};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F09"; pub const CONTRACT_VERSION:&str="worldgen-local-retrieval-synthesis-copilot/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery1@1";
pub fn worldgen_local_retrieval_synthesis_research_copilot_manifest()->serde_json::Value{retrieval_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"local single-study","A1")}
pub fn run_worldgen_local_retrieval_synthesis_research_copilot(r:&RetrievalCopilotRequest)->Result<RetrievalCopilotReceipt,retrieval_copilot_support::RetrievalCopilotError>{retrieval_copilot_support::run(r,FEATURE_ID,CONTRACT_VERSION,false,false)}
pub use retrieval_copilot_support::{RetrievalCopilotError,RetrievalCopilotReceipt as WorldgenLocalRetrievalCopilotReceipt,RetrievalCopilotRequest as WorldgenLocalRetrievalCopilotRequest};
