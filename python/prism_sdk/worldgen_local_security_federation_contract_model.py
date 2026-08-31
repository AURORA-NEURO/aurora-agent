from .worldgen_security_federation_contract_support import *
FEATURE_ID="AFA-worldgen-P20-F05"; CONTRACT_VERSION="worldgen-local-security-federation-contract/1.0"
def worldgen_local_security_federation_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def negotiate_worldgen_local_security_federation_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")

