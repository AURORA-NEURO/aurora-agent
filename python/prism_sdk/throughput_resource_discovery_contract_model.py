"""Worldgen P05 AFA-worldgen-P05-F07 product wrapper."""
from .worldgen_resource_contract_support import ResourceContractRequest, ResourceContractReceipt, manifest as _manifest, negotiate as _run
FEATURE_ID="AFA-worldgen-P05-F07"
CONTRACT_VERSION="worldgen-throughput-resource-contract/1.0"
def worldgen_throughput_resource_discovery_contract_model_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="ResourceContractRequest1@1", scale="prospective high-throughput", autonomy_tier="A0")
def negotiate_worldgen_throughput_resource_contract(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="prospective high-throughput", require_federation=False)
__all__=["ResourceContractRequest","ResourceContractReceipt","worldgen_throughput_resource_discovery_contract_model_manifest","negotiate_worldgen_throughput_resource_contract"]
