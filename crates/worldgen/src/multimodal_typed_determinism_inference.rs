//! Worldgen P17 F02 statistical, causal, and ML inference.
use super::typed_determinism_support::{self,TypedCapabilityInput3,CanonicalCapabilityOutput1};
pub const FEATURE_ID:&str="AFA-worldgen-P17-F02"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-typed-determinism/1.0";
pub fn worldgen_multimodal_typed_determinism_inference_manifest()->serde_json::Value{typed_determinism_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn qualify_worldgen_multimodal_typed_determinism_determinism(request:&TypedCapabilityInput3)->Result<CanonicalCapabilityOutput1,typed_determinism_support::TypedDeterminismError>{typed_determinism_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use typed_determinism_support::{CapabilityCandidate,DeterminismEvidenceState,TypedDeterminismError};

