from .worldgen_provenance_signing_contract_support import *
FEATURE_ID="AFA-worldgen-P18-F08"; CONTRACT_VERSION="worldgen-federated_continual-provenance-signing-contract/1.0"
def worldgen_federated_continual_provenance_signing_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous")
def negotiate_worldgen_federated_continual_provenance_signing_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",federation=True)

