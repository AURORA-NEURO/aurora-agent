from .worldgen_protocol_simulation_contract_support import ProtocolContractRequest, ProtocolContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P10-F05"; CONTRACT_VERSION="worldgen-local-protocol_simulation-contract/1.0"
def worldgen_local_protocol_simulation_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ProtocolContractRequest1@1",scale="local single-study",autonomy_tier="A0")
def negotiate_worldgen_local_protocol_simulation_contract(request:ProtocolContractRequest)->ProtocolContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_federation=False)
