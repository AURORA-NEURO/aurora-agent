//! Worldgen P06 AFA-worldgen-P06-F14 multimodal workflow_fabric.
use super::ingestion_support::{self,MultimodalIngestionRequest,MultimodalIngestionReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P06-F14"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-multimodal-ingestion-workflow_fabric/1.0";
pub fn worldgen_multimodal_multimodal_ingestion_workflow_fabric_manifest()->serde_json::Value{ingestion_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MultimodalIngestionRequest1@1","multimodal multi-study","A1")}
pub fn schedule_worldgen_multimodal_multimodal_ingestion(request:&MultimodalIngestionRequest)->Result<MultimodalIngestionReceipt,ingestion_support::MultimodalIngestionError>{ingestion_support::ingest(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use ingestion_support::{MultimodalIngestionError,MultimodalIngestionReceipt as WorldgenmultimodalMultimodalIngestionReceipt,MultimodalIngestionRequest as WorldgenmultimodalMultimodalIngestionRequest};
