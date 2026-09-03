from .worldgen_security_federation_workflow_support import *
FEATURE_ID="AFA-worldgen-P20-F14"; CONTRACT_VERSION="worldgen-multimodal-security-federation-workflow/1.0"
def worldgen_multimodal_security_federation_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def schedule_worldgen_multimodal_security_federation_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")

