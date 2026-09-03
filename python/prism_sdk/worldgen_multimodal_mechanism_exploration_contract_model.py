from .worldgen_mechanism_contract_support import MechanismContractRequest, MechanismContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P08-F06"; CONTRACT_VERSION="worldgen-multimodal-mechanism-contract/1.0"
def worldgen_multimodal_mechanism_exploration_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismContractRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def negotiate_worldgen_multimodal_mechanism_contract(request:MechanismContractRequest)->MechanismContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_federation=false)
