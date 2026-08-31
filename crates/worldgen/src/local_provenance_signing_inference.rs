//! Worldgen P18 F01 statistical, causal, and ML inference.
use super::provenance_signing_support::{self,ArtifactAndDerivation,SignedProvenanceEnvelope1};
pub const FEATURE_ID:&str="AFA-worldgen-P18-F01"; pub const CONTRACT_VERSION:&str="worldgen-local-provenance-signing/1.0";
pub fn worldgen_local_provenance_signing_inference_manifest()->serde_json::Value{provenance_signing_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn qualify_worldgen_local_provenance_signing_provenance(request:&ArtifactAndDerivation)->Result<SignedProvenanceEnvelope1,provenance_signing_support::ProvenanceSigningError>{provenance_signing_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use provenance_signing_support::{ArtifactCandidate,ProvenanceEvidenceState,ProvenanceSigningError};

