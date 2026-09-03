//! Lab P32 throughput workflow_fabric instrument-execution integrity feature.
use super::instrument_execution_integrity_support::{manifest,qualify,InstrumentExecutionCard7,InstrumentExecutionIntegrityError,InstrumentExecutionRequest4};
pub const FEATURE_ID:&str="AFA-lab-P32-F15";pub const CONTRACT_VERSION:&str="lab-throughput_instrument_execution_integrity_workflow_fabric/1.0";
pub fn throughput_instrument_execution_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","workflow_fabric")}
pub fn qualify_throughput_instrument_execution_integrity_workflow_fabric(request:&InstrumentExecutionRequest4)->Result<InstrumentExecutionCard7,InstrumentExecutionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"throughput","workflow_fabric")}
