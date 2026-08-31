"""Multimodal retrieval-synthesis operations service (AFA-worldgen-P02-F30)."""
from .worldgen_retrieval_operations_support import *
FEATURE_ID="AFA-worldgen-P02-F30"; CONTRACT_VERSION="worldgen-multimodal-retrieval-synthesis-operations/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery2@1"
def worldgen_multimodal_retrieval_synthesis_operations_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="multimodal multi-study",autonomy_tier="A1")
def operate_worldgen_multimodal_retrieval_synthesis_operations(request): return operate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_federation=False)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","RetrievalOperationsRequest","RetrievalOperationsReceipt","worldgen_multimodal_retrieval_synthesis_operations_manifest","operate_worldgen_multimodal_retrieval_synthesis_operations"]
