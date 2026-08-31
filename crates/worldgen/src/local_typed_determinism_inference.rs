//! Worldgen P17 F01 statistical, causal, and ML inference.
use super::typed_determinism_support::{self,TypedCapabilityInput3,CanonicalCapabilityOutput1};
pub const FEATURE_ID:&str="AFA-worldgen-P17-F01"; pub const CONTRACT_VERSION:&str="worldgen-local-typed-determinism/1.0";
pub fn worldgen_local_typed_determinism_inference_manifest()->serde_json::Value{typed_determinism_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn qualify_worldgen_local_typed_determinism_determinism(request:&TypedCapabilityInput3)->Result<CanonicalCapabilityOutput1,typed_determinism_support::TypedDeterminismError>{typed_determinism_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use typed_determinism_support::{CapabilityCandidate,DeterminismEvidenceState,TypedDeterminismError};

