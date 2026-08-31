from .worldgen_mechanism_contract_support import MechanismContractRequest, MechanismContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P08-F05"; CONTRACT_VERSION="worldgen-local-mechanism-contract/1.0"
def worldgen_local_mechanism_exploration_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismContractRequest1@1",scale="local single-study",autonomy_tier="A0")
def negotiate_worldgen_local_mechanism_contract(request:MechanismContractRequest)->MechanismContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_federation=false)
