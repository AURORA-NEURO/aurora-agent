//! Worldgen P24 multimodal multi-study workflow fabric feature F14.
use super::researcher_admin_experience_support::{render,manifest,ResearchWorkspaceCard7,WorkspaceRequest4,FEATURE_ID as BASE_FEATURE_ID,CONTRACT_VERSION as BASE_CONTRACT_VERSION};
const FEATURE_ID:&str="AFA-worldgen-P24-F14";const CONTRACT_VERSION:&str="worldgen-multimodal-researcher-admin-experience-workflow_fabric/1.0";
pub fn worldgen_multimodal_researcher_admin_experience_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow fabric")}
pub fn render_worldgen_multimodal_researcher_admin_experience_workflow(request:&WorkspaceRequest4)->Result<ResearchWorkspaceCard7,super::researcher_admin_experience_support::ResearcherAdminExperienceError>{render(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow fabric")}

