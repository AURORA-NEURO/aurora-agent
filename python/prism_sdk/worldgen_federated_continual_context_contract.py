"""Federated continual context contract model (AFA-worldgen-P03-F08)."""
from .worldgen_context_contract_support import *
FEATURE_ID="AFA-worldgen-P03-F08";CONTRACT_VERSION="worldgen-federated-continual-context-contract/1.0";INPUT_SCHEMA="ContextContractRequest4@1"
def worldgen_federated_continual_context_contract_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="federated continual/autonomous",autonomy_tier="A2")
def compile_worldgen_federated_continual_context_contract(request):return compile(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_federation=True)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","ContextContractRequest","ContextContractReceipt","worldgen_federated_continual_context_contract_manifest","compile_worldgen_federated_continual_context_contract"]
