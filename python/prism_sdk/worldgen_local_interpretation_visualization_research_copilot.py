from .worldgen_interpretation_visualization_copilot_support import *
FEATURE_ID="AFA-worldgen-P14-F09"; CONTRACT_VERSION="worldgen-local-interpretation-visualization-copilot/1.0"
def worldgen_local_interpretation_visualization_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def run_worldgen_local_interpretation_visualization_research_copilot(request):return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=False,federation=False)

