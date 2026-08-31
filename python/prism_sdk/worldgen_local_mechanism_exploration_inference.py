from .worldgen_mechanism_exploration_support import MechanismQuestion, MechanismPortfolio, explore, manifest
FEATURE_ID="AFA-worldgen-P08-F01"; CONTRACT_VERSION="worldgen-local-mechanism-exploration/1.0"
def worldgen_local_mechanism_exploration_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismQuestion1@1",scale="local single-study",autonomy_tier="A0")
def explore_worldgen_local_mechanisms(request:MechanismQuestion)->MechanismPortfolio: return explore(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_federation=false)
