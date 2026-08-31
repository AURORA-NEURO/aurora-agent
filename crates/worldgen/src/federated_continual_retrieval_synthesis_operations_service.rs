//! Federated continual retrieval-synthesis operations service (`AFA-worldgen-P02-F32`).
use super::retrieval_operations_support::{self, RetrievalOperationsRequest, RetrievalOperationsReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F32";
pub const CONTRACT_VERSION:&str="worldgen-federated-continual-retrieval-synthesis-operations/1.0";
pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery4@1";
pub fn worldgen_federated_continual_retrieval_synthesis_operations_manifest()->serde_json::Value{retrieval_operations_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"federated continual/autonomous","A2")}
pub fn operate_worldgen_federated_continual_retrieval_synthesis_operations(r:&RetrievalOperationsRequest)->Result<RetrievalOperationsReceipt,retrieval_operations_support::RetrievalOperationsError>{retrieval_operations_support::operate(r,FEATURE_ID,CONTRACT_VERSION,"federated continual/autonomous",true)}
pub use retrieval_operations_support::{RetrievalOperationsError,RetrievalOperationsReceipt as WorldgenFederatedContinualRetrievalOperationsReceipt,RetrievalOperationsRequest as WorldgenFederatedContinualRetrievalOperationsRequest};
