from .worldgen_policy_autonomy_workflow_support import *
FEATURE_ID="AFA-worldgen-P19-F15"; CONTRACT_VERSION="worldgen-throughput-policy_autonomy-signing-workflow/1.0"
def worldgen_throughput_policy_autonomy_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def schedule_worldgen_throughput_policy_autonomy_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=True,federation=True)

