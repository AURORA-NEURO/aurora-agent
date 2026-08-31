from .worldgen_protocol_simulation_support import ProtocolDraft, ProtocolSimulationReport, simulate, manifest
FEATURE_ID="AFA-worldgen-P10-F04"; CONTRACT_VERSION="worldgen-federated_continual-protocol_simulation/1.0"
def worldgen_federated_continual_protocol_simulation_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ProtocolDraft1@1",scale="federated continual autonomous",autonomy_tier="A1")
def simulate_worldgen_federated_continual_protocol_simulations(request:ProtocolDraft)->ProtocolSimulationReport: return simulate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_federation=True)
