from .worldgen_provenance_signing_workflow_support import *
FEATURE_ID="AFA-worldgen-P18-F13"; CONTRACT_VERSION="worldgen-local-provenance-signing-workflow/1.0"
def worldgen_local_provenance_signing_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def schedule_worldgen_local_provenance_signing_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=False,federation=False)

