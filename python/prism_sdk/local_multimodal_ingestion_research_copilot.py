"""Worldgen P06 AFA-worldgen-P06-F09 product wrapper."""
from .worldgen_ingestion_support import MultimodalIngestionRequest, MultimodalIngestionReceipt, manifest as _manifest, ingest as _run
FEATURE_ID="AFA-worldgen-P06-F09"
CONTRACT_VERSION="worldgen-local-multimodal-ingestion-research_copilot/1.0"
def worldgen_local_multimodal_ingestion_research_copilot_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="MultimodalIngestionRequest1@1", scale="local single-study", autonomy_tier="A1")
def run_worldgen_local_multimodal_ingestion(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", require_federation=False)
__all__=["MultimodalIngestionRequest","MultimodalIngestionReceipt","worldgen_local_multimodal_ingestion_research_copilot_manifest","run_worldgen_local_multimodal_ingestion"]
