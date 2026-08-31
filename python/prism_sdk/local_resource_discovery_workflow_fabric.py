"""Worldgen P05 AFA-worldgen-P05-F13 product wrapper."""
from .worldgen_resource_workflow_support import ResourceWorkflowRequest, ResourceWorkflowReceipt, manifest as _manifest, schedule as _run
FEATURE_ID="AFA-worldgen-P05-F13"
CONTRACT_VERSION="worldgen-local-resource-workflow/1.0"
def worldgen_local_resource_discovery_workflow_fabric_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="ResourceWorkflowRequest1@1", scale="local single-study", autonomy_tier="A1")
def schedule_worldgen_local_resource_discovery_workflow(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", require_approval=False, require_federation=False)
__all__=["ResourceWorkflowRequest","ResourceWorkflowReceipt","worldgen_local_resource_discovery_workflow_fabric_manifest","schedule_worldgen_local_resource_discovery_workflow"]
