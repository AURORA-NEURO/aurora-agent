//! Multimodal multi-study retrieval and synthesis inference engine (`AFA-worldgen-P02-F02`).
use super::retrieval_support::{self, RetrievalQuery, RetrievalReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F02"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-retrieval-synthesis-inference/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery2@1";
pub fn worldgen_multimodal_retrieval_synthesis_inference_manifest()->serde_json::Value{retrieval_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"multimodal multi-study","A1")}
pub fn infer_worldgen_multimodal_retrieval_synthesis(q:&RetrievalQuery)->Result<RetrievalReceipt,retrieval_support::RetrievalError>{retrieval_support::infer(q,FEATURE_ID,CONTRACT_VERSION)}
pub use retrieval_support::{RetrievalCandidate,RetrievalError};
