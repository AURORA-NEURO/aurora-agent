"""Worldgen P05 AFA-worldgen-P05-F16 product wrapper."""
from .worldgen_resource_workflow_support import ResourceWorkflowRequest, ResourceWorkflowReceipt, manifest as _manifest, schedule as _run
FEATURE_ID="AFA-worldgen-P05-F16"
CONTRACT_VERSION="worldgen-federated_continual-resource-workflow/1.0"
def worldgen_federated_continual_resource_discovery_workflow_fabric_manifest(): return _manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, input_schema="ResourceWorkflowRequest1@1", scale="federated continual autonomous", autonomy_tier="A2")
def schedule_worldgen_federated_continual_resource_discovery_workflow(request): return _run(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="federated continual autonomous", require_approval=True, require_federation=True)
__all__=["ResourceWorkflowRequest","ResourceWorkflowReceipt","worldgen_federated_continual_resource_discovery_workflow_fabric_manifest","schedule_worldgen_federated_continual_resource_discovery_workflow"]
