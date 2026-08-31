from .worldgen_publication_research_object_copilot_support import *
FEATURE_ID="AFA-worldgen-P16-F10"; CONTRACT_VERSION="worldgen-multimodal-publication-research-object-copilot/1.0"
def worldgen_multimodal_publication_research_object_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def run_worldgen_multimodal_publication_research_object_research_copilot(request):return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=False,federation=False)

