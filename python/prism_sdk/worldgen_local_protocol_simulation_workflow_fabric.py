from .worldgen_protocol_simulation_workflow_support import ProtocolWorkflowRequest, ProtocolWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P10-F13"; CONTRACT_VERSION="worldgen-local-protocol_simulation-workflow/1.0"
def worldgen_local_protocol_simulation_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ProtocolWorkflowRequest1@1",scale="local single-study",autonomy_tier="A0")
def schedule_worldgen_local_protocol_simulation_workflow(request:ProtocolWorkflowRequest)->ProtocolWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=True,require_federation=False)
