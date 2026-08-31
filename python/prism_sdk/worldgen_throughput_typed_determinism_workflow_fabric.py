from .worldgen_typed_determinism_workflow_support import *
FEATURE_ID="AFA-worldgen-P17-F15"; CONTRACT_VERSION="worldgen-throughput-typed-determinism-workflow/1.0"
def worldgen_throughput_typed_determinism_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def schedule_worldgen_throughput_typed_determinism_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=True,federation=True)

