from .worldgen_statistical_causal_ml_contract_support import *
FEATURE_ID="AFA-worldgen-P13-F07"; CONTRACT_VERSION="worldgen-throughput-statistical-causal-ml-contract/1.0"
def worldgen_throughput_statistical_causal_ml_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def negotiate_worldgen_throughput_statistical_causal_ml_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",federation=False)

