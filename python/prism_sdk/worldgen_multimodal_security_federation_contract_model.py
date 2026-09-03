from .worldgen_security_federation_contract_support import *
FEATURE_ID="AFA-worldgen-P20-F06"; CONTRACT_VERSION="worldgen-multimodal-security-federation-contract/1.0"
def worldgen_multimodal_security_federation_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def negotiate_worldgen_multimodal_security_federation_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")

