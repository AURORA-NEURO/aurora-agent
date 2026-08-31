from .worldgen_publication_research_object_support import *
FEATURE_ID="AFA-worldgen-P16-F01"; CONTRACT_VERSION="worldgen-local-publication-research-object/1.0"
def worldgen_local_publication_research_object_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def qualify_worldgen_local_publication_research_object_release(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)

