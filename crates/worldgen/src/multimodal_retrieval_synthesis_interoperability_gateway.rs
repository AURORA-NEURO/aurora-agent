//! Multimodal retrieval-synthesis interoperability gateway (`AFA-worldgen-P02-F22`).
use super::retrieval_interoperability_support::{self, RetrievalInteroperabilityRequest, RetrievalInteroperabilityReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F22"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-retrieval-synthesis-interoperability-gateway/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery2@1";
pub fn worldgen_multimodal_retrieval_synthesis_interoperability_gateway_manifest()->serde_json::Value{retrieval_interoperability_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"multimodal multi-study","A1")}
pub fn negotiate_worldgen_multimodal_retrieval_synthesis_interoperability(r:&RetrievalInteroperabilityRequest)->Result<RetrievalInteroperabilityReceipt,retrieval_interoperability_support::RetrievalInteroperabilityError>{retrieval_interoperability_support::negotiate(r,FEATURE_ID,CONTRACT_VERSION,true,false)}
pub use retrieval_interoperability_support::{RetrievalInteroperabilityError,RetrievalInteroperabilityReceipt as WorldgenMultimodalRetrievalInteroperabilityReceipt,RetrievalInteroperabilityRequest as WorldgenMultimodalRetrievalInteroperabilityRequest};
