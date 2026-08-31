"""Prospective high-throughput retrieval-synthesis operations service (AFA-worldgen-P02-F31)."""
from .worldgen_retrieval_operations_support import *
FEATURE_ID="AFA-worldgen-P02-F31"; CONTRACT_VERSION="worldgen-throughput-retrieval-synthesis-operations/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery3@1"
def worldgen_throughput_retrieval_synthesis_operations_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="prospective high-throughput",autonomy_tier="A2")
def operate_worldgen_throughput_retrieval_synthesis_operations(request): return operate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_federation=True)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","RetrievalOperationsRequest","RetrievalOperationsReceipt","worldgen_throughput_retrieval_synthesis_operations_manifest","operate_worldgen_throughput_retrieval_synthesis_operations"]
