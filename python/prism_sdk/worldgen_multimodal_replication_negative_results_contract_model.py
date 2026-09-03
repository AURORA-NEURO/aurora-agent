from .worldgen_replication_negative_results_contract_support import *
FEATURE_ID="AFA-worldgen-P15-F06"; CONTRACT_VERSION="worldgen-multimodal-replication-negative-results-contract/1.0"
def worldgen_multimodal_replication_negative_results_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def negotiate_worldgen_multimodal_replication_negative_results_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",federation=False)

