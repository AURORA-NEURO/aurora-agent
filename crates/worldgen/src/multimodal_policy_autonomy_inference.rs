//! Worldgen P19 F02 statistical, causal, and ML inference.
use super::policy_autonomy_support::{self,ArtifactAndDerivation,SignedPolicyAutonomyEnvelope1};
pub const FEATURE_ID:&str="AFA-worldgen-P19-F02"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-policy_autonomy-signing/1.0";
pub fn worldgen_multimodal_policy_autonomy_inference_manifest()->serde_json::Value{policy_autonomy_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn qualify_worldgen_multimodal_policy_autonomy_policy_autonomy(request:&ArtifactAndDerivation)->Result<SignedPolicyAutonomyEnvelope1,policy_autonomy_support::PolicyAutonomyError>{policy_autonomy_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use policy_autonomy_support::{ArtifactCandidate,PolicyAutonomyEvidenceState,PolicyAutonomyError};

