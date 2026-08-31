from .worldgen_typed_determinism_workflow_support import *
FEATURE_ID="AFA-worldgen-P17-F16"; CONTRACT_VERSION="worldgen-federated_continual-typed-determinism-workflow/1.0"
def worldgen_federated_continual_typed_determinism_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous")
def schedule_worldgen_federated_continual_typed_determinism_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_approval=True,federation=True)

