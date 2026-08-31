//! Worldgen P07 AFA-worldgen-P07-F10 quality research copilot.
use super::quality_copilot_support::{self,QualityCopilotRequest,QualityCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P07-F10"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-quality-copilot/1.0";
pub fn worldgen_multimodal_quality_control_research_copilot_manifest()->serde_json::Value{quality_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"QualityCopilotRequest1@1","multimodal multi-study","A1")}
pub fn run_worldgen_multimodal_quality_control_research_copilot(request:&QualityCopilotRequest)->Result<QualityCopilotReceipt,quality_copilot_support::QualityCopilotError>{quality_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false,false)}
pub use quality_copilot_support::{QualityCopilotError,QualityCopilotReceipt as WorldgenMultimodalQualitycontrolresearchcopilotReceipt,QualityCopilotRequest as WorldgenMultimodalQualitycontrolresearchcopilotRequest};

