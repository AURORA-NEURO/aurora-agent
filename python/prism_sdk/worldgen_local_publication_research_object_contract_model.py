from .worldgen_publication_research_object_contract_support import *
FEATURE_ID="AFA-worldgen-P16-F05"; CONTRACT_VERSION="worldgen-local-publication-research-object-contract/1.0"
def worldgen_local_publication_research_object_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def negotiate_worldgen_local_publication_research_object_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",federation=False)

