from .worldgen_interpretation_visualization_contract_support import *
FEATURE_ID="AFA-worldgen-P14-F05"; CONTRACT_VERSION="worldgen-local-interpretation-visualization-contract/1.0"
def worldgen_local_interpretation_visualization_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def negotiate_worldgen_local_interpretation_visualization_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",federation=False)

