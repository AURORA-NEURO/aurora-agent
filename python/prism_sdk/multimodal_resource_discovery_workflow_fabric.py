"""Worldgen P05 AFA-worldgen-P05-F14 product wrapper."""
from .worldgen_resource_workflow_support import ResourceWorkflowRequest, ResourceWorkflowReceipt, manifest as _manifest, schedule as _run
FEATURE_ID="AFA-worldgen-P05-F14"
CONTRACT_VERSION="worldgen-multimodal-resource-workflow/1.0"
def worldgen_multimodal_resource_discovery_workflow_fabric_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="ResourceWorkflowRequest1@1", scale="multimodal multi-study", autonomy_tier="A1")
def schedule_worldgen_multimodal_resource_discovery_workflow(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="multimodal multi-study", require_approval=False, require_federation=False)
__all__=["ResourceWorkflowRequest","ResourceWorkflowReceipt","worldgen_multimodal_resource_discovery_workflow_fabric_manifest","schedule_worldgen_multimodal_resource_discovery_workflow"]
