//! Prospective high-throughput retrieval and synthesis inference engine (`AFA-worldgen-P02-F03`).
use super::retrieval_support::{self, RetrievalQuery, RetrievalReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F03"; pub const CONTRACT_VERSION:&str="worldgen-throughput-retrieval-synthesis-inference/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery3@1";
pub fn worldgen_throughput_retrieval_synthesis_inference_manifest()->serde_json::Value{retrieval_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"prospective high-throughput","A1")}
pub fn infer_worldgen_throughput_retrieval_synthesis(q:&RetrievalQuery)->Result<RetrievalReceipt,retrieval_support::RetrievalError>{retrieval_support::infer(q,FEATURE_ID,CONTRACT_VERSION)}
pub use retrieval_support::{RetrievalCandidate,RetrievalError};
