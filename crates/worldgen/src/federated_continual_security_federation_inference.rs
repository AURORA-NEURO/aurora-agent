//! Worldgen P20 F04 security/federation admission.
use super::security_federation_support::{self,SecurityFederationRequest,SecurityFederationReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P20-F04"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-security-federation/1.0";
pub fn worldgen_federated_continual_security_federation_inference_manifest()->serde_json::Value{security_federation_support::manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous")}
pub fn qualify_worldgen_federated_continual_security_federation_security(request:&SecurityFederationRequest)->Result<SecurityFederationReceipt,security_federation_support::SecurityFederationError>{security_federation_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use security_federation_support::{SecurityFederationAction1,SecurityFederationError1,SecurityFederationEvidenceState};
