"""Federated continual retrieval-synthesis operations service (AFA-worldgen-P02-F32)."""
from .worldgen_retrieval_operations_support import *
FEATURE_ID="AFA-worldgen-P02-F32"; CONTRACT_VERSION="worldgen-federated-continual-retrieval-synthesis-operations/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery4@1"
def worldgen_federated_continual_retrieval_synthesis_operations_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="federated continual/autonomous",autonomy_tier="A2")
def operate_worldgen_federated_continual_retrieval_synthesis_operations(request): return operate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_federation=True)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","RetrievalOperationsRequest","RetrievalOperationsReceipt","worldgen_federated_continual_retrieval_synthesis_operations_manifest","operate_worldgen_federated_continual_retrieval_synthesis_operations"]
