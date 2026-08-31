//! Dataops P32 local single-study research_copilot ingestion-integrity feature F03.
use super::ingestion_integrity_support::{qualify,manifest,IngestionIntegrityCard7,IngestionIntegrityRequest4,IngestionIntegrityError};
const FEATURE_ID:&str="AFA-dataops-P32-F03";const CONTRACT_VERSION:&str="dataops-local-ingestion-integrity-research_copilot/1.0";
pub fn dataops_local_ingestion_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research_copilot")}
pub fn qualify_dataops_local_ingestion_integrity_research_copilot(request:&IngestionIntegrityRequest4)->Result<IngestionIntegrityCard7,IngestionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research_copilot")}
