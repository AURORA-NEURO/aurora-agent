"""Worldgen P05 AFA-worldgen-P05-F02 product wrapper."""
from .worldgen_resource_discovery_support import ResourceDiscoveryRequest, ResourceDiscoveryReceipt, manifest as _manifest, discover as _run
FEATURE_ID="AFA-worldgen-P05-F02"
CONTRACT_VERSION="worldgen-multimodal-resource-discovery/1.0"
def worldgen_multimodal_resource_discovery_inference_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="ResourceDiscoveryRequest1@1", scale="multimodal multi-study", autonomy_tier="A1")
def discover_worldgen_multimodal_resources(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="multimodal multi-study", require_federation=False)
__all__=["ResourceDiscoveryRequest","ResourceDiscoveryReceipt","worldgen_multimodal_resource_discovery_inference_manifest","discover_worldgen_multimodal_resources"]
