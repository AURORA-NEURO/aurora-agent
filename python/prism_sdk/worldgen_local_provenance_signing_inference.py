from .worldgen_provenance_signing_support import *
FEATURE_ID="AFA-worldgen-P18-F01"; CONTRACT_VERSION="worldgen-local-provenance-signing/1.0"
def worldgen_local_provenance_signing_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def qualify_worldgen_local_provenance_signing_provenance(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)

