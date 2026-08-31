from .worldgen_replication_negative_results_workflow_support import *
FEATURE_ID="AFA-worldgen-P15-F13"; CONTRACT_VERSION="worldgen-local-replication-negative-results-workflow/1.0"
def worldgen_local_replication_negative_results_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def schedule_worldgen_local_replication_negative_results_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=False,federation=False)

