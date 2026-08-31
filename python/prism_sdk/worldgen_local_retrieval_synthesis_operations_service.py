"""Local retrieval-synthesis operations service (AFA-worldgen-P02-F29)."""
from .worldgen_retrieval_operations_support import *
FEATURE_ID="AFA-worldgen-P02-F29"; CONTRACT_VERSION="worldgen-local-retrieval-synthesis-operations/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery1@1"
def worldgen_local_retrieval_synthesis_operations_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="local single-study",autonomy_tier="A1")
def operate_worldgen_local_retrieval_synthesis_operations(request): return operate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_federation=False)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","RetrievalOperationsRequest","RetrievalOperationsReceipt","worldgen_local_retrieval_synthesis_operations_manifest","operate_worldgen_local_retrieval_synthesis_operations"]
