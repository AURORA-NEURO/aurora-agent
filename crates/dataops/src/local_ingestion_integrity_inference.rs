//! Dataops P32 local single-study inference ingestion-integrity feature F01.
use super::ingestion_integrity_support::{qualify,manifest,IngestionIntegrityCard7,IngestionIntegrityRequest4,IngestionIntegrityError};
const FEATURE_ID:&str="AFA-dataops-P32-F01";const CONTRACT_VERSION:&str="dataops-local-ingestion-integrity-inference/1.0";
pub fn dataops_local_ingestion_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn qualify_dataops_local_ingestion_integrity_inference(request:&IngestionIntegrityRequest4)->Result<IngestionIntegrityCard7,IngestionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
