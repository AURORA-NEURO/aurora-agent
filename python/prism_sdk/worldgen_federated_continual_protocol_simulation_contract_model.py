from .worldgen_protocol_simulation_contract_support import ProtocolContractRequest, ProtocolContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P10-F08"; CONTRACT_VERSION="worldgen-federated_continual-protocol_simulation-contract/1.0"
def worldgen_federated_continual_protocol_simulation_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ProtocolContractRequest1@1",scale="federated continual autonomous",autonomy_tier="A1")
def negotiate_worldgen_federated_continual_protocol_simulation_contract(request:ProtocolContractRequest)->ProtocolContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_federation=True)
