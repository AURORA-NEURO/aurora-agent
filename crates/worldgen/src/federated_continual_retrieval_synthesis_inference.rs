//! Federated continual retrieval and synthesis inference engine (`AFA-worldgen-P02-F04`).
use super::retrieval_support::{self, RetrievalQuery, RetrievalReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F04"; pub const CONTRACT_VERSION:&str="worldgen-federated-continual-retrieval-synthesis-inference/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery4@1";
pub fn worldgen_federated_continual_retrieval_synthesis_inference_manifest()->serde_json::Value{retrieval_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"federated continual autonomous","A1")}
pub fn infer_worldgen_federated_continual_retrieval_synthesis(q:&RetrievalQuery)->Result<RetrievalReceipt,retrieval_support::RetrievalError>{retrieval_support::infer(q,FEATURE_ID,CONTRACT_VERSION)}
pub use retrieval_support::{RetrievalCandidate,RetrievalError};
