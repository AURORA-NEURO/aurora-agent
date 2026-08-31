//! Worldgen P18 F05 statistical, causal, and ML contract model.
use super::provenance_signing_contract_support::{self,ProvenanceContractRequest,ProvenanceContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P18-F05"; pub const CONTRACT_VERSION:&str="worldgen-local-provenance-signing-contract/1.0";
pub fn worldgen_local_provenance_signing_contract_model_manifest()->serde_json::Value{provenance_signing_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn negotiate_worldgen_local_provenance_signing_contract(request:&ProvenanceContractRequest)->Result<ProvenanceContractReceipt,provenance_signing_contract_support::ProvenanceContractError>{provenance_signing_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false)}
pub use provenance_signing_contract_support::{ProvenanceContractError,ProvenanceContractRequest as WorldgenTypedProvenanceContractRequest,ProvenanceContractReceipt as WorldgenTypedProvenanceContractReceipt};

