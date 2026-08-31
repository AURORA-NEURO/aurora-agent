//! Safety P32 federated continual autonomous contract_model control-integrity feature F14.
use super::control_integrity_support::{qualify,manifest,SafetyIntegrityCard7,SafetyIntegrityRequest4,SafetyIntegrityError};
const FEATURE_ID:&str="AFA-safety-P32-F14";const CONTRACT_VERSION:&str="safety-federated-control-integrity-contract_model/1.0";
pub fn safety_federated_control_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","contract_model")}
pub fn qualify_safety_federated_control_integrity_contract_model(request:&SafetyIntegrityRequest4)->Result<SafetyIntegrityCard7,SafetyIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","contract_model")}
