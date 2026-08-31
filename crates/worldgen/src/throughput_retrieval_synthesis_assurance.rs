//! Throughput retrieval-synthesis assurance harness (`AFA-worldgen-P02-F27`).
use super::retrieval_assurance_support::{self, RetrievalAssuranceRequest, RetrievalAssuranceReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F27"; pub const CONTRACT_VERSION:&str="worldgen-throughput-retrieval-synthesis-assurance/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery3@1";
pub fn worldgen_throughput_retrieval_synthesis_assurance_manifest()->serde_json::Value{retrieval_assurance_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"prospective high-throughput","A1")}
pub fn assure_worldgen_throughput_retrieval_synthesis(r:&RetrievalAssuranceRequest)->Result<RetrievalAssuranceReceipt,retrieval_assurance_support::RetrievalAssuranceError>{retrieval_assurance_support::assure(r,FEATURE_ID,CONTRACT_VERSION,true,false)}
pub use retrieval_assurance_support::{RetrievalAssuranceError,RetrievalAssuranceReceipt as WorldgenThroughputRetrievalAssuranceReceipt,RetrievalAssuranceRequest as WorldgenThroughputRetrievalAssuranceRequest};
