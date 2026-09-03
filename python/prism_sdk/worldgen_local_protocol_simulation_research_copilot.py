from .worldgen_protocol_simulation_copilot_support import ProtocolCopilotRequest, ProtocolCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P10-F09"; CONTRACT_VERSION="worldgen-local-protocol_simulation-copilot/1.0"
def worldgen_local_protocol_simulation_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ProtocolCopilotRequest1@1",scale="local single-study",autonomy_tier="A0")
def run_worldgen_local_protocol_simulation_research_copilot(request:ProtocolCopilotRequest)->ProtocolCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=True,require_federation=False)
