//! Local single-study retrieval and synthesis inference engine (`AFA-worldgen-P02-F01`).
use super::retrieval_support::{self, RetrievalQuery, RetrievalReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F01"; pub const CONTRACT_VERSION:&str="worldgen-local-retrieval-synthesis-inference/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery1@1";
pub fn worldgen_local_retrieval_synthesis_inference_manifest()->serde_json::Value{retrieval_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"local single-study","A0")}
pub fn infer_worldgen_local_retrieval_synthesis(q:&RetrievalQuery)->Result<RetrievalReceipt,retrieval_support::RetrievalError>{retrieval_support::infer(q,FEATURE_ID,CONTRACT_VERSION)}
pub use retrieval_support::{RetrievalCandidate,RetrievalError};
