from .worldgen_publication_research_object_workflow_support import *
FEATURE_ID="AFA-worldgen-P16-F16"; CONTRACT_VERSION="worldgen-federated_continual-publication-research-object-workflow/1.0"
def worldgen_federated_continual_publication_research_object_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous")
def schedule_worldgen_federated_continual_publication_research_object_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_approval=True,federation=True)

