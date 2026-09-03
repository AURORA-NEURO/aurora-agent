from .worldgen_provenance_signing_workflow_support import *
FEATURE_ID="AFA-worldgen-P18-F15"; CONTRACT_VERSION="worldgen-throughput-provenance-signing-workflow/1.0"
def worldgen_throughput_provenance_signing_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def schedule_worldgen_throughput_provenance_signing_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=True,federation=True)

