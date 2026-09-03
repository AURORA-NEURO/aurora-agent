//! Graph P32 multimodal research_copilot projection-integrity feature.
use super::projection_integrity_support::{manifest,qualify,ProjectionCard7,ProjectionIntegrityError,ProjectionRequest4};
pub const FEATURE_ID:&str="AFA-graph-P32-F10";pub const CONTRACT_VERSION:&str="graph-multimodal_projection_integrity_research_copilot/1.0";
pub fn multimodal_projection_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal","research_copilot")}
pub fn qualify_multimodal_projection_integrity_research_copilot(request:&ProjectionRequest4)->Result<ProjectionCard7,ProjectionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal","research_copilot")}
