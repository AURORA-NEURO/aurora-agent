"""Worldgen P06 AFA-worldgen-P06-F15 product wrapper."""
from .worldgen_ingestion_support import MultimodalIngestionRequest, MultimodalIngestionReceipt, manifest as _manifest, ingest as _run
FEATURE_ID="AFA-worldgen-P06-F15"
CONTRACT_VERSION="worldgen-throughput-multimodal-ingestion-workflow_fabric/1.0"
def worldgen_throughput_multimodal_ingestion_workflow_fabric_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="MultimodalIngestionRequest1@1", scale="prospective high-throughput", autonomy_tier="A2")
def schedule_worldgen_throughput_multimodal_ingestion(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="prospective high-throughput", require_federation=False)
__all__=["MultimodalIngestionRequest","MultimodalIngestionReceipt","worldgen_throughput_multimodal_ingestion_workflow_fabric_manifest","schedule_worldgen_throughput_multimodal_ingestion"]
