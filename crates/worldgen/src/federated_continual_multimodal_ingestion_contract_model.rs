//! Worldgen P06 AFA-worldgen-P06-F08 federated_continual contract_model.
use super::ingestion_support::{self,MultimodalIngestionRequest,MultimodalIngestionReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P06-F08"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-multimodal-ingestion-contract_model/1.0";
pub fn worldgen_federated_continual_multimodal_ingestion_contract_model_manifest()->serde_json::Value{ingestion_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MultimodalIngestionRequest1@1","federated continual autonomous","A1")}
pub fn negotiate_worldgen_federated_continual_multimodal_ingestion(request:&MultimodalIngestionRequest)->Result<MultimodalIngestionReceipt,ingestion_support::MultimodalIngestionError>{ingestion_support::ingest(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true)}
pub use ingestion_support::{MultimodalIngestionError,MultimodalIngestionReceipt as Worldgenfederated_continualMultimodalIngestionReceipt,MultimodalIngestionRequest as Worldgenfederated_continualMultimodalIngestionRequest};
