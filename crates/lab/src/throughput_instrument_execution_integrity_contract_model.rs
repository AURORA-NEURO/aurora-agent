//! Lab P32 throughput contract_model instrument-execution integrity feature.
use super::instrument_execution_integrity_support::{manifest,qualify,InstrumentExecutionCard7,InstrumentExecutionIntegrityError,InstrumentExecutionRequest4};
pub const FEATURE_ID:&str="AFA-lab-P32-F07";pub const CONTRACT_VERSION:&str="lab-throughput_instrument_execution_integrity_contract_model/1.0";
pub fn throughput_instrument_execution_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","contract_model")}
pub fn qualify_throughput_instrument_execution_integrity_contract_model(request:&InstrumentExecutionRequest4)->Result<InstrumentExecutionCard7,InstrumentExecutionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"throughput","contract_model")}
