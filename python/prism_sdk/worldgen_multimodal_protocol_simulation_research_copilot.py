from .worldgen_protocol_simulation_copilot_support import ProtocolCopilotRequest, ProtocolCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P10-F10"; CONTRACT_VERSION="worldgen-multimodal-protocol_simulation-copilot/1.0"
def worldgen_multimodal_protocol_simulation_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ProtocolCopilotRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def run_worldgen_multimodal_protocol_simulation_research_copilot(request:ProtocolCopilotRequest)->ProtocolCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=False,require_federation=False)
