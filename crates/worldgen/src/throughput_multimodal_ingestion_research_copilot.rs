//! Worldgen P06 AFA-worldgen-P06-F11 throughput research_copilot.
use super::ingestion_support::{self,MultimodalIngestionRequest,MultimodalIngestionReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P06-F11"; pub const CONTRACT_VERSION:&str="worldgen-throughput-multimodal-ingestion-research_copilot/1.0";
pub fn worldgen_throughput_multimodal_ingestion_research_copilot_manifest()->serde_json::Value{ingestion_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MultimodalIngestionRequest1@1","prospective high-throughput","A1")}
pub fn run_worldgen_throughput_multimodal_ingestion(request:&MultimodalIngestionRequest)->Result<MultimodalIngestionReceipt,ingestion_support::MultimodalIngestionError>{ingestion_support::ingest(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false)}
pub use ingestion_support::{MultimodalIngestionError,MultimodalIngestionReceipt as WorldgenthroughputMultimodalIngestionReceipt,MultimodalIngestionRequest as WorldgenthroughputMultimodalIngestionRequest};
