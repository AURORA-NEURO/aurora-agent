"""Worldgen P05 AFA-worldgen-P05-F15 product wrapper."""
from .worldgen_resource_workflow_support import ResourceWorkflowRequest, ResourceWorkflowReceipt, manifest as _manifest, schedule as _run
FEATURE_ID="AFA-worldgen-P05-F15"
CONTRACT_VERSION="worldgen-throughput-resource-workflow/1.0"
def worldgen_throughput_resource_discovery_workflow_fabric_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="ResourceWorkflowRequest1@1", scale="prospective high-throughput", autonomy_tier="A2")
def schedule_worldgen_throughput_resource_discovery_workflow(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="prospective high-throughput", require_approval=True, require_federation=False)
__all__=["ResourceWorkflowRequest","ResourceWorkflowReceipt","worldgen_throughput_resource_discovery_workflow_fabric_manifest","schedule_worldgen_throughput_resource_discovery_workflow"]
