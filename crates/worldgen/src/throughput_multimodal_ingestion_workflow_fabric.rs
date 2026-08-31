//! Worldgen P06 AFA-worldgen-P06-F15 throughput workflow_fabric.
use super::ingestion_support::{self,MultimodalIngestionRequest,MultimodalIngestionReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P06-F15"; pub const CONTRACT_VERSION:&str="worldgen-throughput-multimodal-ingestion-workflow_fabric/1.0";
pub fn worldgen_throughput_multimodal_ingestion_workflow_fabric_manifest()->serde_json::Value{ingestion_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MultimodalIngestionRequest1@1","prospective high-throughput","A2")}
pub fn schedule_worldgen_throughput_multimodal_ingestion(request:&MultimodalIngestionRequest)->Result<MultimodalIngestionReceipt,ingestion_support::MultimodalIngestionError>{ingestion_support::ingest(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false)}
pub use ingestion_support::{MultimodalIngestionError,MultimodalIngestionReceipt as WorldgenthroughputMultimodalIngestionReceipt,MultimodalIngestionRequest as WorldgenthroughputMultimodalIngestionRequest};
