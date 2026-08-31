from .worldgen_provenance_signing_workflow_support import *
FEATURE_ID="AFA-worldgen-P18-F14"; CONTRACT_VERSION="worldgen-multimodal-provenance-signing-workflow/1.0"
def worldgen_multimodal_provenance_signing_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def schedule_worldgen_multimodal_provenance_signing_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=False,federation=False)

