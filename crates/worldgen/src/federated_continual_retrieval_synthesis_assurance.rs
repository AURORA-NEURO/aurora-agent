//! Federated continual retrieval-synthesis assurance harness (`AFA-worldgen-P02-F28`).
use super::retrieval_assurance_support::{self, RetrievalAssuranceRequest, RetrievalAssuranceReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F28"; pub const CONTRACT_VERSION:&str="worldgen-federated-continual-retrieval-synthesis-assurance/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery4@1";
pub fn worldgen_federated_continual_retrieval_synthesis_assurance_manifest()->serde_json::Value{retrieval_assurance_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"federated continual","A2")}
pub fn assure_worldgen_federated_continual_retrieval_synthesis(r:&RetrievalAssuranceRequest)->Result<RetrievalAssuranceReceipt,retrieval_assurance_support::RetrievalAssuranceError>{retrieval_assurance_support::assure(r,FEATURE_ID,CONTRACT_VERSION,true,true)}
pub use retrieval_assurance_support::{RetrievalAssuranceError,RetrievalAssuranceReceipt as WorldgenFederatedContinualRetrievalAssuranceReceipt,RetrievalAssuranceRequest as WorldgenFederatedContinualRetrievalAssuranceRequest};
