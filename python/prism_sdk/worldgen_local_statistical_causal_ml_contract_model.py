from .worldgen_statistical_causal_ml_contract_support import *
FEATURE_ID="AFA-worldgen-P13-F05"; CONTRACT_VERSION="worldgen-local-statistical-causal-ml-contract/1.0"
def worldgen_local_statistical_causal_ml_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def negotiate_worldgen_local_statistical_causal_ml_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",federation=False)

