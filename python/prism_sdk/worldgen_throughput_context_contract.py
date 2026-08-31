"""Prospective high-throughput context contract model (AFA-worldgen-P03-F07)."""
from .worldgen_context_contract_support import *
FEATURE_ID="AFA-worldgen-P03-F07";CONTRACT_VERSION="worldgen-throughput-context-contract/1.0";INPUT_SCHEMA="ContextContractRequest3@1"
def worldgen_throughput_context_contract_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="prospective high-throughput",autonomy_tier="A2")
def compile_worldgen_throughput_context_contract(request):return compile(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_federation=True)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","ContextContractRequest","ContextContractReceipt","worldgen_throughput_context_contract_manifest","compile_worldgen_throughput_context_contract"]
