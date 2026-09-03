from .worldgen_protocol_simulation_workflow_support import ProtocolWorkflowRequest, ProtocolWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P10-F15"; CONTRACT_VERSION="worldgen-throughput-protocol_simulation-workflow/1.0"
def worldgen_throughput_protocol_simulation_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ProtocolWorkflowRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def schedule_worldgen_throughput_protocol_simulation_workflow(request:ProtocolWorkflowRequest)->ProtocolWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=False,require_federation=False)
