"""Worldgen P05 AFA-worldgen-P05-F03 product wrapper."""
from .worldgen_resource_discovery_support import ResourceDiscoveryRequest, ResourceDiscoveryReceipt, manifest as _manifest, discover as _run
FEATURE_ID="AFA-worldgen-P05-F03"
CONTRACT_VERSION="worldgen-throughput-resource-discovery/1.0"
def worldgen_throughput_resource_discovery_inference_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="ResourceDiscoveryRequest1@1", scale="prospective high-throughput", autonomy_tier="A1")
def discover_worldgen_throughput_resources(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="prospective high-throughput", require_federation=False)
__all__=["ResourceDiscoveryRequest","ResourceDiscoveryReceipt","worldgen_throughput_resource_discovery_inference_manifest","discover_worldgen_throughput_resources"]
