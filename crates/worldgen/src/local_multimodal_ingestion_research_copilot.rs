//! Worldgen P06 AFA-worldgen-P06-F09 local research_copilot.
use super::ingestion_support::{self,MultimodalIngestionRequest,MultimodalIngestionReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P06-F09"; pub const CONTRACT_VERSION:&str="worldgen-local-multimodal-ingestion-research_copilot/1.0";
pub fn worldgen_local_multimodal_ingestion_research_copilot_manifest()->serde_json::Value{ingestion_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MultimodalIngestionRequest1@1","local single-study","A1")}
pub fn run_worldgen_local_multimodal_ingestion(request:&MultimodalIngestionRequest)->Result<MultimodalIngestionReceipt,ingestion_support::MultimodalIngestionError>{ingestion_support::ingest(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false)}
pub use ingestion_support::{MultimodalIngestionError,MultimodalIngestionReceipt as WorldgenlocalMultimodalIngestionReceipt,MultimodalIngestionRequest as WorldgenlocalMultimodalIngestionRequest};
