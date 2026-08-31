//! Worldgen P18 F04 statistical, causal, and ML inference.
use super::provenance_signing_support::{self,ArtifactAndDerivation,SignedProvenanceEnvelope1};
pub const FEATURE_ID:&str="AFA-worldgen-P18-F04"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-provenance-signing/1.0";
pub fn worldgen_federated_continual_provenance_signing_inference_manifest()->serde_json::Value{provenance_signing_support::manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous")}
pub fn qualify_worldgen_federated_continual_provenance_signing_provenance(request:&ArtifactAndDerivation)->Result<SignedProvenanceEnvelope1,provenance_signing_support::ProvenanceSigningError>{provenance_signing_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use provenance_signing_support::{ArtifactCandidate,ProvenanceEvidenceState,ProvenanceSigningError};

