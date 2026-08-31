"""Worldgen P05 AFA-worldgen-P05-F05 product wrapper."""
from .worldgen_resource_contract_support import ResourceContractRequest, ResourceContractReceipt, manifest as _manifest, negotiate as _run
FEATURE_ID="AFA-worldgen-P05-F05"
CONTRACT_VERSION="worldgen-local-resource-contract/1.0"
def worldgen_local_resource_discovery_contract_model_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="ResourceContractRequest1@1", scale="local single-study", autonomy_tier="A0")
def negotiate_worldgen_local_resource_contract(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", require_federation=False)
__all__=["ResourceContractRequest","ResourceContractReceipt","worldgen_local_resource_discovery_contract_model_manifest","negotiate_worldgen_local_resource_contract"]
