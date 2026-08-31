//! Dataops P32 multimodal multi-study workflow_fabric ingestion-integrity feature F08.
use super::ingestion_integrity_support::{qualify,manifest,IngestionIntegrityCard7,IngestionIntegrityRequest4,IngestionIntegrityError};
const FEATURE_ID:&str="AFA-dataops-P32-F08";const CONTRACT_VERSION:&str="dataops-multimodal-ingestion-integrity-workflow_fabric/1.0";
pub fn dataops_multimodal_ingestion_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow_fabric")}
pub fn qualify_dataops_multimodal_ingestion_integrity_workflow_fabric(request:&IngestionIntegrityRequest4)->Result<IngestionIntegrityCard7,IngestionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow_fabric")}
