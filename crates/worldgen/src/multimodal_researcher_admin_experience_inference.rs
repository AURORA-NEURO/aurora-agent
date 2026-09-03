//! Worldgen P24 multimodal multi-study inference feature F02.
use super::researcher_admin_experience_support::{render,manifest,ResearchWorkspaceCard7,WorkspaceRequest4,FEATURE_ID as BASE_FEATURE_ID,CONTRACT_VERSION as BASE_CONTRACT_VERSION};
const FEATURE_ID:&str="AFA-worldgen-P24-F02";const CONTRACT_VERSION:&str="worldgen-multimodal-researcher-admin-experience-inference/1.0";
pub fn worldgen_multimodal_researcher_admin_experience_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
pub fn render_worldgen_multimodal_researcher_admin_experience(request:&WorkspaceRequest4)->Result<ResearchWorkspaceCard7,super::researcher_admin_experience_support::ResearcherAdminExperienceError>{render(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}

