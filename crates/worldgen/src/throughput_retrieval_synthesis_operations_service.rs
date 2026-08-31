//! Prospective high-throughput retrieval-synthesis operations service (`AFA-worldgen-P02-F31`).
use super::retrieval_operations_support::{self, RetrievalOperationsRequest, RetrievalOperationsReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F31";
pub const CONTRACT_VERSION:&str="worldgen-throughput-retrieval-synthesis-operations/1.0";
pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery3@1";
pub fn worldgen_throughput_retrieval_synthesis_operations_manifest()->serde_json::Value{retrieval_operations_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"prospective high-throughput","A2")}
pub fn operate_worldgen_throughput_retrieval_synthesis_operations(r:&RetrievalOperationsRequest)->Result<RetrievalOperationsReceipt,retrieval_operations_support::RetrievalOperationsError>{retrieval_operations_support::operate(r,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true)}
pub use retrieval_operations_support::{RetrievalOperationsError,RetrievalOperationsReceipt as WorldgenThroughputRetrievalOperationsReceipt,RetrievalOperationsRequest as WorldgenThroughputRetrievalOperationsRequest};
