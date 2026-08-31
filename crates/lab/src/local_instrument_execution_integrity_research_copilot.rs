//! Lab P32 local research_copilot instrument-execution integrity feature.
use super::instrument_execution_integrity_support::{manifest,qualify,InstrumentExecutionCard7,InstrumentExecutionIntegrityError,InstrumentExecutionRequest4};
pub const FEATURE_ID:&str="AFA-lab-P32-F09";pub const CONTRACT_VERSION:&str="lab-local_instrument_execution_integrity_research_copilot/1.0";
pub fn local_instrument_execution_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","research_copilot")}
pub fn qualify_local_instrument_execution_integrity_research_copilot(request:&InstrumentExecutionRequest4)->Result<InstrumentExecutionCard7,InstrumentExecutionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","research_copilot")}
