//! Dataops P32 multimodal multi-study contract_model ingestion-integrity feature F06.
use super::ingestion_integrity_support::{qualify,manifest,IngestionIntegrityCard7,IngestionIntegrityRequest4,IngestionIntegrityError};
const FEATURE_ID:&str="AFA-dataops-P32-F06";const CONTRACT_VERSION:&str="dataops-multimodal-ingestion-integrity-contract_model/1.0";
pub fn dataops_multimodal_ingestion_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract_model")}
pub fn qualify_dataops_multimodal_ingestion_integrity_contract_model(request:&IngestionIntegrityRequest4)->Result<IngestionIntegrityCard7,IngestionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract_model")}
