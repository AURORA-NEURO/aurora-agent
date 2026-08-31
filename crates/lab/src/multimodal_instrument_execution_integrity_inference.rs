//! Lab P32 multimodal inference instrument-execution integrity feature.
use super::instrument_execution_integrity_support::{manifest,qualify,InstrumentExecutionCard7,InstrumentExecutionIntegrityError,InstrumentExecutionRequest4};
pub const FEATURE_ID:&str="AFA-lab-P32-F02";pub const CONTRACT_VERSION:&str="lab-multimodal_instrument_execution_integrity_inference/1.0";
pub fn multimodal_instrument_execution_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal","inference")}
pub fn qualify_multimodal_instrument_execution_integrity_inference(request:&InstrumentExecutionRequest4)->Result<InstrumentExecutionCard7,InstrumentExecutionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal","inference")}
