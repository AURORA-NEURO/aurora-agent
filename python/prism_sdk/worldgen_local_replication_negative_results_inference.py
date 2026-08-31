from .worldgen_replication_negative_results_support import *
FEATURE_ID="AFA-worldgen-P15-F01"; CONTRACT_VERSION="worldgen-local-replication-negative-results/1.0"
def worldgen_local_replication_negative_results_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def qualify_worldgen_local_replication_negative_results_replication(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)

