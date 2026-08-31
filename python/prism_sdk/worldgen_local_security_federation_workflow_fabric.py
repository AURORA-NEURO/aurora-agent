from .worldgen_security_federation_workflow_support import *
FEATURE_ID="AFA-worldgen-P20-F13"; CONTRACT_VERSION="worldgen-local-security-federation-workflow/1.0"
def worldgen_local_security_federation_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def schedule_worldgen_local_security_federation_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")

