"""Worldgen P05 AFA-worldgen-P05-F12 product wrapper."""
from .worldgen_resource_copilot_support import ResourceCopilotRequest, ResourceCopilotReceipt, manifest as _manifest, run as _run
FEATURE_ID="AFA-worldgen-P05-F12"
CONTRACT_VERSION="worldgen-federated_continual-resource-copilot/1.0"
def worldgen_federated_continual_resource_discovery_research_copilot_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="ResourceCopilotRequest1@1", scale="federated continual autonomous", autonomy_tier="A2")
def run_worldgen_federated_continual_resource_discovery_research_copilot(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="federated continual autonomous", require_approval=True, require_federation=True)
__all__=["ResourceCopilotRequest","ResourceCopilotReceipt","worldgen_federated_continual_resource_discovery_research_copilot_manifest","run_worldgen_federated_continual_resource_discovery_research_copilot"]
