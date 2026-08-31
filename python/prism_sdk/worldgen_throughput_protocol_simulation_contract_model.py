from .worldgen_protocol_simulation_contract_support import ProtocolContractRequest, ProtocolContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P10-F07"; CONTRACT_VERSION="worldgen-throughput-protocol_simulation-contract/1.0"
def worldgen_throughput_protocol_simulation_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ProtocolContractRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def negotiate_worldgen_throughput_protocol_simulation_contract(request:ProtocolContractRequest)->ProtocolContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_federation=False)
