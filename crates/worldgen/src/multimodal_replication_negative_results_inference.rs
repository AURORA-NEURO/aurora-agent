//! Worldgen P15 F02 statistical, causal, and ML inference.
use super::replication_negative_results_support::{self,ClaimAndProtocol3,ReplicationRecord1};
pub const FEATURE_ID:&str="AFA-worldgen-P15-F02"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-replication-negative-results/1.0";
pub fn worldgen_multimodal_replication_negative_results_inference_manifest()->serde_json::Value{replication_negative_results_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn qualify_worldgen_multimodal_replication_negative_results_replication(request:&ClaimAndProtocol3)->Result<ReplicationRecord1,replication_negative_results_support::ReplicationNegativeResultsError>{replication_negative_results_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use replication_negative_results_support::{ReplicationCandidate,ReplicationEvidenceState,ReplicationNegativeResultsError};

