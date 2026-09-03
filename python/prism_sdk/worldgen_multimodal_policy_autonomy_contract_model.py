from .worldgen_policy_autonomy_contract_support import *
FEATURE_ID="AFA-worldgen-P19-F06"; CONTRACT_VERSION="worldgen-multimodal-policy_autonomy-signing-contract/1.0"
def worldgen_multimodal_policy_autonomy_contract_model_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def negotiate_worldgen_multimodal_policy_autonomy_contract(request):return negotiate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",federation=False)

