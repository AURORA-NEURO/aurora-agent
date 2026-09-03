"""Worldgen P05 AFA-worldgen-P05-F08 product wrapper."""
from .worldgen_resource_contract_support import ResourceContractRequest, ResourceContractReceipt, manifest as _manifest, negotiate as _run
FEATURE_ID="AFA-worldgen-P05-F08"
CONTRACT_VERSION="worldgen-federated_continual-resource-contract/1.0"
def worldgen_federated_continual_resource_discovery_contract_model_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="ResourceContractRequest1@1", scale="federated continual autonomous", autonomy_tier="A2")
def negotiate_worldgen_federated_continual_resource_contract(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="federated continual autonomous", require_federation=True)
__all__=["ResourceContractRequest","ResourceContractReceipt","worldgen_federated_continual_resource_discovery_contract_model_manifest","negotiate_worldgen_federated_continual_resource_contract"]
