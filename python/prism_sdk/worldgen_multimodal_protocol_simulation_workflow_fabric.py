from .worldgen_protocol_simulation_workflow_support import ProtocolWorkflowRequest, ProtocolWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P10-F14"; CONTRACT_VERSION="worldgen-multimodal-protocol_simulation-workflow/1.0"
def worldgen_multimodal_protocol_simulation_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ProtocolWorkflowRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def schedule_worldgen_multimodal_protocol_simulation_workflow(request:ProtocolWorkflowRequest)->ProtocolWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=False,require_federation=False)
