//! Multimodal retrieval-synthesis operations service (`AFA-worldgen-P02-F30`).
use super::retrieval_operations_support::{self, RetrievalOperationsRequest, RetrievalOperationsReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F30";
pub const CONTRACT_VERSION:&str="worldgen-multimodal-retrieval-synthesis-operations/1.0";
pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery2@1";
pub fn worldgen_multimodal_retrieval_synthesis_operations_manifest()->serde_json::Value{retrieval_operations_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"multimodal multi-study","A1")}
pub fn operate_worldgen_multimodal_retrieval_synthesis_operations(r:&RetrievalOperationsRequest)->Result<RetrievalOperationsReceipt,retrieval_operations_support::RetrievalOperationsError>{retrieval_operations_support::operate(r,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use retrieval_operations_support::{RetrievalOperationsError,RetrievalOperationsReceipt as WorldgenMultimodalRetrievalOperationsReceipt,RetrievalOperationsRequest as WorldgenMultimodalRetrievalOperationsRequest};
