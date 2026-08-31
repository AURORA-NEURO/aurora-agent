from .worldgen_interpretation_visualization_copilot_support import *
FEATURE_ID="AFA-worldgen-P14-F10"; CONTRACT_VERSION="worldgen-multimodal-interpretation-visualization-copilot/1.0"
def worldgen_multimodal_interpretation_visualization_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def run_worldgen_multimodal_interpretation_visualization_research_copilot(request):return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=False,federation=False)

