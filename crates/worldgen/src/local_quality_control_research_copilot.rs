//! Worldgen P07 AFA-worldgen-P07-F09 quality research copilot.
use super::quality_copilot_support::{self,QualityCopilotRequest,QualityCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P07-F09"; pub const CONTRACT_VERSION:&str="worldgen-local-quality-copilot/1.0";
pub fn worldgen_local_quality_control_research_copilot_manifest()->serde_json::Value{quality_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"QualityCopilotRequest1@1","local single-study","A0")}
pub fn run_worldgen_local_quality_control_research_copilot(request:&QualityCopilotRequest)->Result<QualityCopilotReceipt,quality_copilot_support::QualityCopilotError>{quality_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use quality_copilot_support::{QualityCopilotError,QualityCopilotReceipt as WorldgenLocalQualitycontrolresearchcopilotReceipt,QualityCopilotRequest as WorldgenLocalQualitycontrolresearchcopilotRequest};

