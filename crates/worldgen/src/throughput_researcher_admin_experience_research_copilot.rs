//! Worldgen P24 prospective high-throughput research copilot feature F11.
use super::researcher_admin_experience_support::{render,manifest,ResearchWorkspaceCard7,WorkspaceRequest4,FEATURE_ID as BASE_FEATURE_ID,CONTRACT_VERSION as BASE_CONTRACT_VERSION};
const FEATURE_ID:&str="AFA-worldgen-P24-F11";const CONTRACT_VERSION:&str="worldgen-throughput-researcher-admin-experience-research_copilot/1.0";
pub fn worldgen_throughput_researcher_admin_experience_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","research copilot")}
pub fn render_worldgen_throughput_researcher_admin_experience_copilot(request:&WorkspaceRequest4)->Result<ResearchWorkspaceCard7,super::researcher_admin_experience_support::ResearcherAdminExperienceError>{render(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","research copilot")}

