"""Worldgen P05 AFA-worldgen-P05-F04 product wrapper."""
from .worldgen_resource_discovery_support import ResourceDiscoveryRequest, ResourceDiscoveryReceipt, manifest as _manifest, discover as _run
FEATURE_ID="AFA-worldgen-P05-F04"
CONTRACT_VERSION="worldgen-federated_continual-resource-discovery/1.0"
def worldgen_federated_continual_resource_discovery_inference_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="ResourceDiscoveryRequest1@1", scale="federated continual autonomous", autonomy_tier="A1")
def discover_worldgen_federated_continual_resources(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="federated continual autonomous", require_federation=True)
__all__=["ResourceDiscoveryRequest","ResourceDiscoveryReceipt","worldgen_federated_continual_resource_discovery_inference_manifest","discover_worldgen_federated_continual_resources"]
