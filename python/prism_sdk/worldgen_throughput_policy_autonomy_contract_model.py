from .worldgen_policy_autonomy_contract_support import *
FEATURE_ID="AFA-worldgen-P19-F07"; CONTRACT_VERSION="worldgen-throughput-policy_autonomy-signing-contract/1.0"
def worldgen_throughput_policy_autonomy_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def negotiate_worldgen_throughput_policy_autonomy_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",federation=False)

