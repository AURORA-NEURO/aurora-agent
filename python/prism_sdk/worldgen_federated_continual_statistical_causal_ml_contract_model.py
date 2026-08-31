from .worldgen_statistical_causal_ml_contract_support import *
FEATURE_ID="AFA-worldgen-P13-F08"; CONTRACT_VERSION="worldgen-federated_continual-statistical-causal-ml-contract/1.0"
def worldgen_federated_continual_statistical_causal_ml_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous")
def negotiate_worldgen_federated_continual_statistical_causal_ml_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",federation=True)

