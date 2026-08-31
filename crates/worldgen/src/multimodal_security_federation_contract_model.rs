//! Worldgen P20 F02 security/federation contract negotiation.
use super::security_federation_contract_support::{self,SecurityFederationContractRequest,SecurityFederationContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P20-06"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-security-federation-contract/1.0";
pub fn worldgen_multimodal_security_federation_contract_model_manifest()->serde_json::Value{security_federation_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn negotiate_worldgen_multimodal_security_federation_contract(request:&SecurityFederationContractRequest)->Result<SecurityFederationContractReceipt,security_federation_contract_support::SecurityFederationContractError>{security_federation_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use security_federation_contract_support::{SecurityFederationContractError,SecurityFederationContractRequest as WorldgenSecurityFederationContractRequest,SecurityFederationContractReceipt as WorldgenSecurityFederationContractReceipt};
