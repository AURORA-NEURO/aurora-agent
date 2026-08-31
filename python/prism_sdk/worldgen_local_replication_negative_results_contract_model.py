from .worldgen_replication_negative_results_contract_support import *
FEATURE_ID="AFA-worldgen-P15-F05"; CONTRACT_VERSION="worldgen-local-replication-negative-results-contract/1.0"
def worldgen_local_replication_negative_results_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def negotiate_worldgen_local_replication_negative_results_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",federation=False)

