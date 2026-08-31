//! Worldgen P26 federated continual autonomous contract model feature F08.
use super::limitation_closure_support::{close,manifest,LimitationClosureCard7,LimitationClosureRequest4};
const FEATURE_ID:&str="AFA-worldgen-P26-F08";const CONTRACT_VERSION:&str="worldgen-federated_continual-limitation-closure-contract_model/1.0";
pub fn worldgen_federated_continual_limitation_closure_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","contract model")}
pub fn close_worldgen_federated_limitation_closure_contract(request:&LimitationClosureRequest4)->Result<LimitationClosureCard7,super::limitation_closure_support::LimitationClosureError>{close(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","contract model")}

