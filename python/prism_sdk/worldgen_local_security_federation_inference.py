from .worldgen_security_federation_support import *
FEATURE_ID="AFA-worldgen-P20-F01"; CONTRACT_VERSION="worldgen-local-security-federation/1.0"
def worldgen_local_security_federation_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def qualify_worldgen_local_security_federation_security(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)

