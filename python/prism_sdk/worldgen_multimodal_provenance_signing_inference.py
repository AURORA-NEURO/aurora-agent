from .worldgen_provenance_signing_support import *
FEATURE_ID="AFA-worldgen-P18-F02"; CONTRACT_VERSION="worldgen-multimodal-provenance-signing/1.0"
def worldgen_multimodal_provenance_signing_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def qualify_worldgen_multimodal_provenance_signing_provenance(request):return qualify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION)

