//! Worldgen P20 F01 security/federation copilot.
use super::security_federation_copilot_support::{self,SecurityFederationCopilotRequest,SecurityFederationCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P20-09"; pub const CONTRACT_VERSION:&str="worldgen-local-security-federation-copilot/1.0";
pub fn worldgen_local_security_federation_research_copilot_manifest()->serde_json::Value{security_federation_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn run_worldgen_local_security_federation_research_copilot(request:&SecurityFederationCopilotRequest)->Result<SecurityFederationCopilotReceipt,security_federation_copilot_support::SecurityFederationCopilotError>{security_federation_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use security_federation_copilot_support::{SecurityFederationCopilotError,SecurityFederationCopilotRequest as WorldgenSecurityFederationCopilotRequest,SecurityFederationCopilotReceipt as WorldgenSecurityFederationCopilotReceipt};
