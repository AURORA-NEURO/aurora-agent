//! Worldgen P06 AFA-worldgen-P06-F07 throughput contract_model.
use super::ingestion_support::{self,MultimodalIngestionRequest,MultimodalIngestionReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P06-F07"; pub const CONTRACT_VERSION:&str="worldgen-throughput-multimodal-ingestion-contract_model/1.0";
pub fn worldgen_throughput_multimodal_ingestion_contract_model_manifest()->serde_json::Value{ingestion_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MultimodalIngestionRequest1@1","prospective high-throughput","A1")}
pub fn negotiate_worldgen_throughput_multimodal_ingestion(request:&MultimodalIngestionRequest)->Result<MultimodalIngestionReceipt,ingestion_support::MultimodalIngestionError>{ingestion_support::ingest(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false)}
pub use ingestion_support::{MultimodalIngestionError,MultimodalIngestionReceipt as WorldgenthroughputMultimodalIngestionReceipt,MultimodalIngestionRequest as WorldgenthroughputMultimodalIngestionRequest};
