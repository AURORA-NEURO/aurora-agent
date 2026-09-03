"""Worldgen P05 AFA-worldgen-P05-F01 product wrapper."""
from .worldgen_resource_discovery_support import ResourceDiscoveryRequest, ResourceDiscoveryReceipt, manifest as _manifest, discover as _run
FEATURE_ID="AFA-worldgen-P05-F01"
CONTRACT_VERSION="worldgen-local-resource-discovery/1.0"
def worldgen_local_resource_discovery_inference_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="ResourceDiscoveryRequest1@1", scale="local single-study", autonomy_tier="A0")
def discover_worldgen_local_resources(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", require_federation=False)
__all__=["ResourceDiscoveryRequest","ResourceDiscoveryReceipt","worldgen_local_resource_discovery_inference_manifest","discover_worldgen_local_resources"]
