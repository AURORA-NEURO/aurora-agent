from .worldgen_replication_negative_results_support import *
FEATURE_ID="AFA-worldgen-P15-F03"; CONTRACT_VERSION="worldgen-throughput-replication-negative-results/1.0"
def worldgen_throughput_replication_negative_results_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def qualify_worldgen_throughput_replication_negative_results_replication(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)

