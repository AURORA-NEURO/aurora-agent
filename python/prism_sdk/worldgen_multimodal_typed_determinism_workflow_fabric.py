from .worldgen_typed_determinism_workflow_support import *
FEATURE_ID="AFA-worldgen-P17-F14"; CONTRACT_VERSION="worldgen-multimodal-typed-determinism-workflow/1.0"
def worldgen_multimodal_typed_determinism_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def schedule_worldgen_multimodal_typed_determinism_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=False,federation=False)

