from .worldgen_policy_autonomy_copilot_support import *
FEATURE_ID="AFA-worldgen-P19-F09"; CONTRACT_VERSION="worldgen-local-policy_autonomy-signing-copilot/1.0"
def worldgen_local_policy_autonomy_research_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def run_worldgen_local_policy_autonomy_research_copilot(request):return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=False,federation=False)

