from .worldgen_mechanism_workflow_support import MechanismWorkflowRequest, MechanismWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P08-F15"; CONTRACT_VERSION="worldgen-throughput-mechanism-workflow/1.0"
def worldgen_throughput_mechanism_exploration_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismWorkflowRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def schedule_worldgen_throughput_mechanism_exploration_workflow(request:MechanismWorkflowRequest)->MechanismWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=false,require_federation=false)
