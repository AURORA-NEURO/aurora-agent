from .worldgen_security_federation_workflow_support import *
FEATURE_ID="AFA-worldgen-P20-F15"; CONTRACT_VERSION="worldgen-throughput-security-federation-workflow/1.0"
def worldgen_throughput_security_federation_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def schedule_worldgen_throughput_security_federation_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")

