"""Local context contract model (AFA-worldgen-P03-F05)."""
from .worldgen_context_contract_support import *
FEATURE_ID="AFA-worldgen-P03-F05";CONTRACT_VERSION="worldgen-local-context-contract/1.0";INPUT_SCHEMA="ContextContractRequest1@1"
def worldgen_local_context_contract_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="local single-study",autonomy_tier="A0")
def compile_worldgen_local_context_contract(request):return compile(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_federation=False)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","ContextContractRequest","ContextContractReceipt","worldgen_local_context_contract_manifest","compile_worldgen_local_context_contract"]
