from .worldgen_statistical_causal_ml_workflow_support import *
FEATURE_ID="AFA-worldgen-P13-F16"; CONTRACT_VERSION="worldgen-federated_continual-statistical-causal-ml-workflow/1.0"
def worldgen_federated_continual_statistical_causal_ml_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous")
def schedule_worldgen_federated_continual_statistical_causal_ml_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_approval=True,federation=True)

