from .worldgen_interpretation_visualization_workflow_support import *
FEATURE_ID="AFA-worldgen-P14-F14"; CONTRACT_VERSION="worldgen-multimodal-interpretation-visualization-workflow/1.0"
def worldgen_multimodal_interpretation_visualization_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def schedule_worldgen_multimodal_interpretation_visualization_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=False,federation=False)

