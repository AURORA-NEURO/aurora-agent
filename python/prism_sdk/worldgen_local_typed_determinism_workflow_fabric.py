from .worldgen_typed_determinism_workflow_support import *
FEATURE_ID="AFA-worldgen-P17-F13"; CONTRACT_VERSION="worldgen-local-typed-determinism-workflow/1.0"
def worldgen_local_typed_determinism_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def schedule_worldgen_local_typed_determinism_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=False,federation=False)

