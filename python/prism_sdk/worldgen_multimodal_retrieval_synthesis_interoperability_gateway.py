"""Multimodal retrieval-synthesis interoperability gateway (AFA-worldgen-P02-F22)."""
from typing import Any, Mapping
from .worldgen_retrieval_interoperability_support import RetrievalInteroperabilityReceipt, negotiate, manifest
FEATURE_ID="AFA-worldgen-P02-F22"; CONTRACT_VERSION="worldgen-multimodal-retrieval-synthesis-interoperability-gateway/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery2@1"; OUTPUT_SCHEMA="EvidenceSynthesis5@1"
def worldgen_multimodal_retrieval_synthesis_interoperability_gateway_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="multimodal multi-study",autonomy_tier="A1")
def negotiate_worldgen_multimodal_retrieval_synthesis_interoperability(protocol:Mapping[str,Any], workbench_request)->RetrievalInteroperabilityReceipt: return negotiate(protocol,workbench_request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,output_schema=OUTPUT_SCHEMA,require_approval=True,require_federation=False,semantic_loss_budget=int(protocol.get("semantic_loss_budget",4)))
__all__=["RetrievalInteroperabilityReceipt","negotiate_worldgen_multimodal_retrieval_synthesis_interoperability","worldgen_multimodal_retrieval_synthesis_interoperability_gateway_manifest"]
