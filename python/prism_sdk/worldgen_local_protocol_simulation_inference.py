from .worldgen_protocol_simulation_support import ProtocolDraft, ProtocolSimulationReport, simulate, manifest
FEATURE_ID="AFA-worldgen-P10-F01"; CONTRACT_VERSION="worldgen-local-protocol_simulation/1.0"
def worldgen_local_protocol_simulation_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ProtocolDraft1@1",scale="local single-study",autonomy_tier="A0")
def simulate_worldgen_local_protocol_simulations(request:ProtocolDraft)->ProtocolSimulationReport: return simulate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_federation=False)
