//! Local retrieval-synthesis interoperability gateway (`AFA-worldgen-P02-F21`).
use super::retrieval_interoperability_support::{self, RetrievalInteroperabilityRequest, RetrievalInteroperabilityReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F21"; pub const CONTRACT_VERSION:&str="worldgen-local-retrieval-synthesis-interoperability-gateway/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery1@1";
pub fn worldgen_local_retrieval_synthesis_interoperability_gateway_manifest()->serde_json::Value{retrieval_interoperability_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"local single-study","A1")}
pub fn negotiate_worldgen_local_retrieval_synthesis_interoperability(r:&RetrievalInteroperabilityRequest)->Result<RetrievalInteroperabilityReceipt,retrieval_interoperability_support::RetrievalInteroperabilityError>{retrieval_interoperability_support::negotiate(r,FEATURE_ID,CONTRACT_VERSION,false,false)}
pub use retrieval_interoperability_support::{RetrievalInteroperabilityError,RetrievalInteroperabilityReceipt as WorldgenLocalRetrievalInteroperabilityReceipt,RetrievalInteroperabilityRequest as WorldgenLocalRetrievalInteroperabilityRequest};
