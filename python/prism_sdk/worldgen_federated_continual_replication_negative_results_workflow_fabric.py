from .worldgen_replication_negative_results_workflow_support import *
FEATURE_ID="AFA-worldgen-P15-F16"; CONTRACT_VERSION="worldgen-federated_continual-replication-negative-results-workflow/1.0"
def worldgen_federated_continual_replication_negative_results_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous")
def schedule_worldgen_federated_continual_replication_negative_results_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_approval=True,federation=True)

