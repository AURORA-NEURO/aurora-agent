from .worldgen_quality_workflow_support import QualityWorkflowRequest, QualityWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P07-F16"; CONTRACT_VERSION="worldgen-federated_continual-quality-workflow/1.0"
def worldgen_federated_continual_quality_control_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityWorkflowRequest1@1",scale="federated continual autonomous",autonomy_tier="A1")
def schedule_worldgen_federated_continual_quality_control_workflow(request:QualityWorkflowRequest)->QualityWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_approval=false,require_federation=true)
