from .worldgen_mechanism_workflow_support import MechanismWorkflowRequest, MechanismWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P08-F16"; CONTRACT_VERSION="worldgen-federated_continual-mechanism-workflow/1.0"
def worldgen_federated_continual_mechanism_exploration_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismWorkflowRequest1@1",scale="federated continual autonomous",autonomy_tier="A1")
def schedule_worldgen_federated_continual_mechanism_exploration_workflow(request:MechanismWorkflowRequest)->MechanismWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_approval=false,require_federation=true)
