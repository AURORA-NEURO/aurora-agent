from .worldgen_protocol_simulation_support import ProtocolDraft, ProtocolSimulationReport, simulate, manifest
FEATURE_ID="AFA-worldgen-P10-F02"; CONTRACT_VERSION="worldgen-multimodal-protocol_simulation/1.0"
def worldgen_multimodal_protocol_simulation_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ProtocolDraft1@1",scale="multimodal multi-study",autonomy_tier="A1")
def simulate_worldgen_multimodal_protocol_simulations(request:ProtocolDraft)->ProtocolSimulationReport: return simulate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_federation=False)
