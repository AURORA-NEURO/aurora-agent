//! Worldgen P15 F04 statistical, causal, and ML inference.
use super::replication_negative_results_support::{self,ClaimAndProtocol3,ReplicationRecord1};
pub const FEATURE_ID:&str="AFA-worldgen-P15-F04"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-replication-negative-results/1.0";
pub fn worldgen_federated_continual_replication_negative_results_inference_manifest()->serde_json::Value{replication_negative_results_support::manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous")}
pub fn qualify_worldgen_federated_continual_replication_negative_results_replication(request:&ClaimAndProtocol3)->Result<ReplicationRecord1,replication_negative_results_support::ReplicationNegativeResultsError>{replication_negative_results_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use replication_negative_results_support::{ReplicationCandidate,ReplicationEvidenceState,ReplicationNegativeResultsError};

