from .worldgen_provenance_signing_contract_support import *
FEATURE_ID="AFA-worldgen-P18-F06"; CONTRACT_VERSION="worldgen-multimodal-provenance-signing-contract/1.0"
def worldgen_multimodal_provenance_signing_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def negotiate_worldgen_multimodal_provenance_signing_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",federation=False)

