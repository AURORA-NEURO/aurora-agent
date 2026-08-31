"""Worldgen P05 AFA-worldgen-P05-F09 product wrapper."""
from .worldgen_resource_copilot_support import ResourceCopilotRequest, ResourceCopilotReceipt, manifest as _manifest, run as _run
FEATURE_ID="AFA-worldgen-P05-F09"
CONTRACT_VERSION="worldgen-local-resource-copilot/1.0"
def worldgen_local_resource_discovery_research_copilot_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="ResourceCopilotRequest1@1", scale="local single-study", autonomy_tier="A1")
def run_worldgen_local_resource_discovery_research_copilot(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", require_approval=False, require_federation=False)
__all__=["ResourceCopilotRequest","ResourceCopilotReceipt","worldgen_local_resource_discovery_research_copilot_manifest","run_worldgen_local_resource_discovery_research_copilot"]
