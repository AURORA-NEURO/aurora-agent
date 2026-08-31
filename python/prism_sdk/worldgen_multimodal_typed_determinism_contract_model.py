from .worldgen_typed_determinism_contract_support import *
FEATURE_ID="AFA-worldgen-P17-F06"; CONTRACT_VERSION="worldgen-multimodal-typed-determinism-contract/1.0"
def worldgen_multimodal_typed_determinism_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def negotiate_worldgen_multimodal_typed_determinism_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",federation=False)

