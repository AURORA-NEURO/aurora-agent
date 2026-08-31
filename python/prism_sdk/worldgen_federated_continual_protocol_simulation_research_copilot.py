from .worldgen_protocol_simulation_copilot_support import ProtocolCopilotRequest, ProtocolCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P10-F12"; CONTRACT_VERSION="worldgen-federated_continual-protocol_simulation-copilot/1.0"
def worldgen_federated_continual_protocol_simulation_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ProtocolCopilotRequest1@1",scale="federated continual autonomous",autonomy_tier="A1")
def run_worldgen_federated_continual_protocol_simulation_research_copilot(request:ProtocolCopilotRequest)->ProtocolCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_approval=False,require_federation=True)
