//! Worldgen P07 AFA-worldgen-P07-F12 quality research copilot.
use super::quality_copilot_support::{self,QualityCopilotRequest,QualityCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P07-F12"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-quality-copilot/1.0";
pub fn worldgen_federated_continual_quality_control_research_copilot_manifest()->serde_json::Value{quality_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"QualityCopilotRequest1@1","federated continual autonomous","A1")}
pub fn run_worldgen_federated_continual_quality_control_research_copilot(request:&QualityCopilotRequest)->Result<QualityCopilotReceipt,quality_copilot_support::QualityCopilotError>{quality_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",false,true)}
pub use quality_copilot_support::{QualityCopilotError,QualityCopilotReceipt as WorldgenFederatedContinualQualitycontrolresearchcopilotReceipt,QualityCopilotRequest as WorldgenFederatedContinualQualitycontrolresearchcopilotRequest};

