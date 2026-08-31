"""Worldgen P06 AFA-worldgen-P06-F08 product wrapper."""
from .worldgen_ingestion_support import MultimodalIngestionRequest, MultimodalIngestionReceipt, manifest as _manifest, ingest as _run
FEATURE_ID="AFA-worldgen-P06-F08"
CONTRACT_VERSION="worldgen-federated_continual-multimodal-ingestion-contract_model/1.0"
def worldgen_federated_continual_multimodal_ingestion_contract_model_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="MultimodalIngestionRequest1@1", scale="federated continual autonomous", autonomy_tier="A1")
def negotiate_worldgen_federated_continual_multimodal_ingestion(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="federated continual autonomous", require_federation=True)
__all__=["MultimodalIngestionRequest","MultimodalIngestionReceipt","worldgen_federated_continual_multimodal_ingestion_contract_model_manifest","negotiate_worldgen_federated_continual_multimodal_ingestion"]
