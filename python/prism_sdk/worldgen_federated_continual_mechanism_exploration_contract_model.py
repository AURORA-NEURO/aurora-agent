from .worldgen_mechanism_contract_support import MechanismContractRequest, MechanismContractReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P08-F08"; CONTRACT_VERSION="worldgen-federated_continual-mechanism-contract/1.0"
def worldgen_federated_continual_mechanism_exploration_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismContractRequest1@1",scale="federated continual autonomous",autonomy_tier="A1")
def negotiate_worldgen_federated_continual_mechanism_contract(request:MechanismContractRequest)->MechanismContractReceipt: return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_federation=true)
