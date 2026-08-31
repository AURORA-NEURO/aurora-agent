//! Worldgen P24 prospective high-throughput contract model feature F07.
use super::researcher_admin_experience_support::{render,manifest,ResearchWorkspaceCard7,WorkspaceRequest4,FEATURE_ID as BASE_FEATURE_ID,CONTRACT_VERSION as BASE_CONTRACT_VERSION};
const FEATURE_ID:&str="AFA-worldgen-P24-F07";const CONTRACT_VERSION:&str="worldgen-throughput-researcher-admin-experience-contract_model/1.0";
pub fn worldgen_throughput_researcher_admin_experience_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","contract model")}
pub fn render_worldgen_throughput_researcher_admin_experience_contract(request:&WorkspaceRequest4)->Result<ResearchWorkspaceCard7,super::researcher_admin_experience_support::ResearcherAdminExperienceError>{render(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","contract model")}

