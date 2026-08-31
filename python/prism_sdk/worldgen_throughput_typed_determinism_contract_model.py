from .worldgen_typed_determinism_contract_support import *
FEATURE_ID="AFA-worldgen-P17-F07"; CONTRACT_VERSION="worldgen-throughput-typed-determinism-contract/1.0"
def worldgen_throughput_typed_determinism_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def negotiate_worldgen_throughput_typed_determinism_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",federation=False)

