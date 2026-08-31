from .worldgen_provenance_signing_copilot_support import *
FEATURE_ID="AFA-worldgen-P18-F12"; CONTRACT_VERSION="worldgen-federated_continual-provenance-signing-copilot/1.0"
def worldgen_federated_continual_provenance_signing_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous")
def run_worldgen_federated_continual_provenance_signing_research_copilot(request):return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_approval=True,federation=True)

