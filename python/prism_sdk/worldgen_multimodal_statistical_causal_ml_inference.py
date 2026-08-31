from .worldgen_statistical_causal_ml_support import *
FEATURE_ID="AFA-worldgen-P13-F02"; CONTRACT_VERSION="worldgen-multimodal-statistical-causal-ml/1.0"
def worldgen_multimodal_statistical_causal_ml_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def qualify_worldgen_multimodal_statistical_causal_ml_analysis(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)

