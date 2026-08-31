from .worldgen_mechanism_workflow_support import MechanismWorkflowRequest, MechanismWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P08-F13"; CONTRACT_VERSION="worldgen-local-mechanism-workflow/1.0"
def worldgen_local_mechanism_exploration_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismWorkflowRequest1@1",scale="local single-study",autonomy_tier="A0")
def schedule_worldgen_local_mechanism_exploration_workflow(request:MechanismWorkflowRequest)->MechanismWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=true,require_federation=false)
