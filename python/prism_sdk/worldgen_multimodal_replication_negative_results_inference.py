from .worldgen_replication_negative_results_support import *
FEATURE_ID="AFA-worldgen-P15-F02"; CONTRACT_VERSION="worldgen-multimodal-replication-negative-results/1.0"
def worldgen_multimodal_replication_negative_results_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def qualify_worldgen_multimodal_replication_negative_results_replication(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)

