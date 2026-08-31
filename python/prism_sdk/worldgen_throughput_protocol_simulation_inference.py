from .worldgen_protocol_simulation_support import ProtocolDraft, ProtocolSimulationReport, simulate, manifest
FEATURE_ID="AFA-worldgen-P10-F03"; CONTRACT_VERSION="worldgen-throughput-protocol_simulation/1.0"
def worldgen_throughput_protocol_simulation_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ProtocolDraft1@1",scale="prospective high-throughput",autonomy_tier="A1")
def simulate_worldgen_throughput_protocol_simulations(request:ProtocolDraft)->ProtocolSimulationReport: return simulate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_federation=False)
