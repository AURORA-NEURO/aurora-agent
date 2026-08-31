//! Worldgen P20 F02 security/federation admission.
use super::security_federation_support::{self,SecurityFederationRequest,SecurityFederationReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P20-F02"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-security-federation/1.0";
pub fn worldgen_multimodal_security_federation_inference_manifest()->serde_json::Value{security_federation_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn qualify_worldgen_multimodal_security_federation_security(request:&SecurityFederationRequest)->Result<SecurityFederationReceipt,security_federation_support::SecurityFederationError>{security_federation_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use security_federation_support::{SecurityFederationAction1,SecurityFederationError1,SecurityFederationEvidenceState};
