from .worldgen_mechanism_contract_support import MechanismContractRequest, MechanismContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P08-F07"; CONTRACT_VERSION="worldgen-throughput-mechanism-contract/1.0"
def worldgen_throughput_mechanism_exploration_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismContractRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def negotiate_worldgen_throughput_mechanism_contract(request:MechanismContractRequest)->MechanismContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_federation=false)
