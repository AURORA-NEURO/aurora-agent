from .worldgen_mechanism_exploration_support import MechanismQuestion, MechanismPortfolio, explore, manifest
FEATURE_ID="AFA-worldgen-P08-F04"; CONTRACT_VERSION="worldgen-federated_continual-mechanism-exploration/1.0"
def worldgen_federated_continual_mechanism_exploration_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismQuestion1@1",scale="federated continual autonomous",autonomy_tier="A1")
def explore_worldgen_federated_continual_mechanisms(request:MechanismQuestion)->MechanismPortfolio: return explore(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_federation=true)
