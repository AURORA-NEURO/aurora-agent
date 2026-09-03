//! Lab P32 local contract_model instrument-execution integrity feature.
use super::instrument_execution_integrity_support::{manifest,qualify,InstrumentExecutionCard7,InstrumentExecutionIntegrityError,InstrumentExecutionRequest4};
pub const FEATURE_ID:&str="AFA-lab-P32-F05";pub const CONTRACT_VERSION:&str="lab-local_instrument_execution_integrity_contract_model/1.0";
pub fn local_instrument_execution_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","contract_model")}
pub fn qualify_local_instrument_execution_integrity_contract_model(request:&InstrumentExecutionRequest4)->Result<InstrumentExecutionCard7,InstrumentExecutionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","contract_model")}
