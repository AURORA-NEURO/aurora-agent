from .worldgen_publication_research_object_workflow_support import *
FEATURE_ID="AFA-worldgen-P16-F13"; CONTRACT_VERSION="worldgen-local-publication-research-object-workflow/1.0"
def worldgen_local_publication_research_object_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def schedule_worldgen_local_publication_research_object_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=False,federation=False)

