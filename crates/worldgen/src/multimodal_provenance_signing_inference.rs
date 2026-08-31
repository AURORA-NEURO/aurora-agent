//! Worldgen P18 F02 statistical, causal, and ML inference.
use super::provenance_signing_support::{self,ArtifactAndDerivation,SignedProvenanceEnvelope1};
pub const FEATURE_ID:&str="AFA-worldgen-P18-F02"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-provenance-signing/1.0";
pub fn worldgen_multimodal_provenance_signing_inference_manifest()->serde_json::Value{provenance_signing_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn qualify_worldgen_multimodal_provenance_signing_provenance(request:&ArtifactAndDerivation)->Result<SignedProvenanceEnvelope1,provenance_signing_support::ProvenanceSigningError>{provenance_signing_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use provenance_signing_support::{ArtifactCandidate,ProvenanceEvidenceState,ProvenanceSigningError};

