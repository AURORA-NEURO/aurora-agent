"""Multimodal context contract model (AFA-worldgen-P03-F06)."""
from .worldgen_context_contract_support import *
FEATURE_ID="AFA-worldgen-P03-F06";CONTRACT_VERSION="worldgen-multimodal-context-contract/1.0";INPUT_SCHEMA="ContextContractRequest2@1"
def worldgen_multimodal_context_contract_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="multimodal multi-study",autonomy_tier="A1")
def compile_worldgen_multimodal_context_contract(request):return compile(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_federation=False)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","ContextContractRequest","ContextContractReceipt","worldgen_multimodal_context_contract_manifest","compile_worldgen_multimodal_context_contract"]
