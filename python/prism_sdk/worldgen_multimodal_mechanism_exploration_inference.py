from .worldgen_mechanism_exploration_support import MechanismQuestion, MechanismPortfolio, explore, manifest
FEATURE_ID="AFA-worldgen-P08-F02"; CONTRACT_VERSION="worldgen-multimodal-mechanism-exploration/1.0"
def worldgen_multimodal_mechanism_exploration_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismQuestion1@1",scale="multimodal multi-study",autonomy_tier="A1")
def explore_worldgen_multimodal_mechanisms(request:MechanismQuestion)->MechanismPortfolio: return explore(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_federation=false)
