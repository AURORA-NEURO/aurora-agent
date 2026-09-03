from .worldgen_mechanism_workflow_support import MechanismWorkflowRequest, MechanismWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P08-F14"; CONTRACT_VERSION="worldgen-multimodal-mechanism-workflow/1.0"
def worldgen_multimodal_mechanism_exploration_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismWorkflowRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def schedule_worldgen_multimodal_mechanism_exploration_workflow(request:MechanismWorkflowRequest)->MechanismWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=false,require_federation=false)
