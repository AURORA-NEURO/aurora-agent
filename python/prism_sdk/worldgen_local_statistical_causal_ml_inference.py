from .worldgen_statistical_causal_ml_support import *
FEATURE_ID="AFA-worldgen-P13-F01"; CONTRACT_VERSION="worldgen-local-statistical-causal-ml/1.0"
def worldgen_local_statistical_causal_ml_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def qualify_worldgen_local_statistical_causal_ml_analysis(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)

