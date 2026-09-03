from .worldgen_publication_research_object_workflow_support import *
FEATURE_ID="AFA-worldgen-P16-F14"; CONTRACT_VERSION="worldgen-multimodal-publication-research-object-workflow/1.0"
def worldgen_multimodal_publication_research_object_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def schedule_worldgen_multimodal_publication_research_object_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=False,federation=False)

