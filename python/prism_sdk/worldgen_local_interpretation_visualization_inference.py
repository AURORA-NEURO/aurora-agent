from .worldgen_interpretation_visualization_support import *
FEATURE_ID="AFA-worldgen-P14-F01"; CONTRACT_VERSION="worldgen-local-interpretation-visualization/1.0"
def worldgen_local_interpretation_visualization_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def qualify_worldgen_local_interpretation_visualization_interpretation(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)

