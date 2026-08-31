"""Worldgen P06 AFA-worldgen-P06-F07 product wrapper."""
from .worldgen_ingestion_support import MultimodalIngestionRequest, MultimodalIngestionReceipt, manifest as _manifest, ingest as _run
FEATURE_ID="AFA-worldgen-P06-F07"
CONTRACT_VERSION="worldgen-throughput-multimodal-ingestion-contract_model/1.0"
def worldgen_throughput_multimodal_ingestion_contract_model_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="MultimodalIngestionRequest1@1", scale="prospective high-throughput", autonomy_tier="A1")
def negotiate_worldgen_throughput_multimodal_ingestion(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="prospective high-throughput", require_federation=False)
__all__=["MultimodalIngestionRequest","MultimodalIngestionReceipt","worldgen_throughput_multimodal_ingestion_contract_model_manifest","negotiate_worldgen_throughput_multimodal_ingestion"]
