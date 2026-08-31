from .worldgen_publication_research_object_contract_support import *
FEATURE_ID="AFA-worldgen-P16-F08"; CONTRACT_VERSION="worldgen-federated_continual-publication-research-object-contract/1.0"
def worldgen_federated_continual_publication_research_object_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous")
def negotiate_worldgen_federated_continual_publication_research_object_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",federation=True)

