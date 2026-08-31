from .worldgen_mechanism_exploration_support import MechanismQuestion, MechanismPortfolio, explore, manifest
FEATURE_ID="AFA-worldgen-P08-F03"; CONTRACT_VERSION="worldgen-throughput-mechanism-exploration/1.0"
def worldgen_throughput_mechanism_exploration_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismQuestion1@1",scale="prospective high-throughput",autonomy_tier="A1")
def explore_worldgen_throughput_mechanisms(request:MechanismQuestion)->MechanismPortfolio: return explore(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_federation=false)
