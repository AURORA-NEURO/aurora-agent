//! Multimodal retrieval-synthesis assurance harness (`AFA-worldgen-P02-F26`).
use super::retrieval_assurance_support::{self, RetrievalAssuranceRequest, RetrievalAssuranceReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F26"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-retrieval-synthesis-assurance/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery2@1";
pub fn worldgen_multimodal_retrieval_synthesis_assurance_manifest()->serde_json::Value{retrieval_assurance_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"multimodal multi-study","A1")}
pub fn assure_worldgen_multimodal_retrieval_synthesis(r:&RetrievalAssuranceRequest)->Result<RetrievalAssuranceReceipt,retrieval_assurance_support::RetrievalAssuranceError>{retrieval_assurance_support::assure(r,FEATURE_ID,CONTRACT_VERSION,true,false)}
pub use retrieval_assurance_support::{RetrievalAssuranceError,RetrievalAssuranceReceipt as WorldgenMultimodalRetrievalAssuranceReceipt,RetrievalAssuranceRequest as WorldgenMultimodalRetrievalAssuranceRequest};
