from .worldgen_publication_research_object_copilot_support import *
FEATURE_ID="AFA-worldgen-P16-F12"; CONTRACT_VERSION="worldgen-federated_continual-publication-research-object-copilot/1.0"
def worldgen_federated_continual_publication_research_object_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous")
def run_worldgen_federated_continual_publication_research_object_research_copilot(request):return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_approval=True,federation=True)

