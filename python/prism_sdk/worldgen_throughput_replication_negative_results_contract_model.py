from .worldgen_replication_negative_results_contract_support import *
FEATURE_ID="AFA-worldgen-P15-F07"; CONTRACT_VERSION="worldgen-throughput-replication-negative-results-contract/1.0"
def worldgen_throughput_replication_negative_results_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def negotiate_worldgen_throughput_replication_negative_results_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",federation=False)

